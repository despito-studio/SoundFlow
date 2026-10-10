// Explicit network integration check. Runs the packaged worker with PATH empty.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const temp=await fs.mkdtemp(path.join(os.tmpdir(),'soundflow-live-'));
const child=spawn(path.join(root,'resources/runtime/soundflow-node.exe'),[path.join(root,'media-provider/dist/worker.cjs'),'--data-dir',path.join(temp,'data'),'--download-dir',path.join(temp,'downloads')],{windowsHide:true,env:{...process.env,PATH:''},stdio:['pipe','pipe','pipe']});
const exit=new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);});
let serial=0;let pending='';const waiting=new Map();let stderr='';
child.stderr.on('data',chunk=>stderr+=chunk.toString());
child.stdout.on('data',chunk=>{pending+=chunk.toString();let newline;while((newline=pending.indexOf('\n'))>=0){const frame=JSON.parse(pending.slice(0,newline));pending=pending.slice(newline+1);waiting.get(frame.id)?.(frame);waiting.delete(frame.id);}});
async function call(method,...args) {
  const id=++serial;const frame=await new Promise((resolve,reject)=>{const timeout=setTimeout(()=>reject(new Error(`${method} timed out`)),120000);waiting.set(id,frame=>{clearTimeout(timeout);resolve(frame);});child.stdin.write(JSON.stringify({id,method,args})+'\n');});
  if(frame.error)throw new Error(`${method}: ${frame.error.code}: ${frame.error.message}`);return frame.result;
}
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function download(track) {
  const enqueued=await call('download_track',track);let last=-1;let progressSeen=false;
  for(let i=0;i<180;i++){
    const job=await call('get_download_job',enqueued.job.id);
    if(job.progress!==last){last=job.progress;if(job.progress>0&&job.progress<100)progressSeen=true;}
    if(job.status==='completed'){
      const stat=await fs.stat(job.file_path);assert.equal(stat.size,job.downloaded_bytes);assert.ok(stat.size>10000);assert.ok(['m4a','mp3'].includes(job.format));
      return {status:job.status,bytes:stat.size,format:job.format,progress_observed:progressSeen};
    }
    assert.ok(!['failed','cancelled'].includes(job.status),job.error);await pause(100);
  }
  throw new Error('Download did not complete.');
}
const report={checked_at:new Date().toISOString(),runtime:null,checks:{}};
try {
  report.runtime=await call('ping');assert.equal(report.runtime.engine,'youtubei.js');assert.equal(report.runtime.runtime_bundled,true);
  const yt=await call('resolve_direct_url','https://www.youtube.com/watch?v=K4DyBUG242c');assert.equal(yt.source,'youtube');assert.ok(yt.title.includes('On & On'));
  const ytSearch=await call('search','NCS On And On','youtube',5);assert.ok(ytSearch.length>=2);report.checks.youtube_search={count:ytSearch.length};
  const ytURL=await call('get_stream',yt);const yr=await fetch(ytURL,{headers:{range:'bytes=0-1023'}});const yb=await yr.arrayBuffer();assert.ok(yr.ok);assert.ok(yb.byteLength>=1024);assert.match(yr.headers.get('content-type'),/audio\/mp4/);
  report.checks.youtube_stream={http_status:yr.status,probe_bytes:yb.byteLength,mime:yr.headers.get('content-type')};
  report.checks.youtube_download=await download(yt);
  const playback=await call('get_playback_file',yt);assert.ok(playback.includes('provider-audio'));assert.equal((await fs.stat(playback)).size,report.checks.youtube_download.bytes);
  const repeatPlayback=await call('get_playback_file',yt);assert.equal(playback,repeatPlayback);report.checks.youtube_playback_cache={bytes:(await fs.stat(playback)).size,reused:true};
  const scSearch=await call('search','NCS On And On','soundcloud',4);assert.ok(scSearch.length>=2);const sc=scSearch.find(t=>t.url.includes('nocopyrightsounds'))||scSearch[0];
  report.checks.soundcloud_search={count:scSearch.length};
  const scURL=await call('get_stream',sc);const sr=await fetch(scURL,{headers:{range:'bytes=0-1023'}});const sb=await sr.arrayBuffer();assert.ok(sr.ok);assert.ok(sb.byteLength>=1024);assert.match(sr.headers.get('content-type'),/audio\/mpeg/);
  report.checks.soundcloud_stream={http_status:sr.status,probe_bytes:sb.byteLength,mime:sr.headers.get('content-type')};
  report.checks.soundcloud_download=await download(sc);
  const scPlayback=await call('get_playback_file',sc);assert.ok(scPlayback.endsWith('.mp3'));assert.equal((await fs.stat(scPlayback)).size,report.checks.soundcloud_download.bytes);report.checks.soundcloud_playback_cache={bytes:(await fs.stat(scPlayback)).size};
  const spotify=await call('resolve_direct_url','https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC');assert.equal(spotify.source,'spotify');assert.equal(spotify.downloadable,false);
  const matches=await call('match_spotify_track',spotify,'all',4);assert.equal(matches.length,4);assert.ok(matches.every(t=>t.matched_from.source==='spotify'&&t.match_confirmed===false));report.checks.spotify_metadata_match={title:spotify.title,candidates:matches.length};
  const similar=await call('get_similar_tracks',yt,5,[yt.id]);assert.ok(similar.length>0);assert.ok(similar.every(t=>t.id!==yt.id));report.checks.similar={count:similar.length};
  const discover=await call('get_discover_tracks','lofi');assert.ok(discover.length>0);report.checks.discover={count:discover.length};
  const profile=await call('import_external_profile','soundcloud','https://soundcloud.com/nocopyrightsounds');assert.ok(profile.length>0);report.checks.soundcloud_public_import={count:profile.length};
  const channel=await call('import_external_profile','youtube','https://www.youtube.com/@NoCopyrightSounds');assert.ok(channel.length>0);report.checks.youtube_public_import={count:channel.length};
  const long=await call('search','lofi hip hop 1 hour mix','youtube',8);const cancelTrack=long.find(t=>t.duration>1800 && t.duration<18000);assert.ok(cancelTrack);
  const queued=await call('download_track',cancelTrack);let cancelled;
  for(let i=0;i<180;i++){
    const job=await call('get_download_job',queued.job.id);
    if(job.downloaded_bytes>0 && job.status==='downloading') {await call('cancel_download',job.id);cancelled=job;break;}
    assert.ok(!['failed','completed'].includes(job.status),job.error);await pause(30);
  }
  assert.ok(cancelled,'Cancellation check never observed active bytes.');
  let final;for(let i=0;i<100;i++){final=await call('get_download_job',queued.job.id);if(final.status==='cancelled')break;await pause(30);}
  assert.equal(final.status,'cancelled');assert.equal(final.file_path,null);const files=await fs.readdir(path.join(temp,'downloads'));assert.ok(!files.some(f=>f.includes(queued.job.id)));
  report.checks.active_cancel={status:final.status,bytes_before_cancel:cancelled.downloaded_bytes,partial_file_removed:true};
  const jobs=await call('get_downloads');assert.equal(jobs.length,3);report.checks.download_history={jobs:jobs.length};
} finally {
  child.stdin.end();const code=await exit;assert.equal(code,0,stderr);await fs.rm(temp,{recursive:true,force:true});
  await fs.mkdir(path.join(root,'output/provider-js'),{recursive:true});await fs.writeFile(path.join(root,'output/provider-js/live-report.json'),JSON.stringify(report,null,2)+'\n');
}
console.log(JSON.stringify(report,null,2));
