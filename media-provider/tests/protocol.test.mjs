import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { canonicalURL, safeFilename, MediaProvider, handleRequest, MAX_REQUEST_BYTES } from '../worker.mjs';
const project=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
test('canonical provider URLs reject network/file escape and normalize supported tracks',()=>{
  assert.equal(canonicalURL('https://youtu.be/dQw4w9WgXcQ?si=abc').url,'https://www.youtube.com/watch?v=dQw4w9WgXcQ');
  assert.equal(canonicalURL('spotify:track:4uLU6hMCjMI75M1A2tKUQC').source,'spotify');
  assert.equal(canonicalURL('https://open.spotify.com/intl-uk/embed/track/4uLU6hMCjMI75M1A2tKUQC').id,'4uLU6hMCjMI75M1A2tKUQC');
  for(const value of ['file:///C:/private','https://127.0.0.1/private','https://youtube.com.evil.invalid/watch?v=dQw4w9WgXcQ','https://user:secret@youtube.com/watch?v=dQw4w9WgXcQ','https://soundcloud.com/a/%2e%2e','https://soundcloud.com/a/likes','https://youtube.com:8080/watch?v=dQw4w9WgXcQ']) assert.throws(()=>canonicalURL(value));
});
test('protocol validation rejects malformed identifiers, decode errors and unknown methods',async()=>{
  const provider=new MediaProvider(path.join(os.tmpdir(),'soundflow-protocol-unwritten'));
  for(const frame of ['{','[]','{"id":false,"method":"ping"}','{"id":-1,"method":"ping"}','{"id":1,"method":"ping","args":{}}',Buffer.from([0xc3,0x28])])assert.equal((await handleRequest(provider,frame)).error.code,'invalid_request');
  assert.equal((await handleRequest(provider,'{"id":2,"method":"eval","args":[]}')).error.code,'unsupported_method');
  assert.equal((await handleRequest(provider,'{"id":2,"method":"constructor","args":[]}')).error.code,'unsupported_method');
  assert.equal((await handleRequest(provider,JSON.stringify({id:3,method:'get_downloads',args:[]}))).result.length,0);
  assert.equal((await handleRequest(provider,Buffer.alloc(MAX_REQUEST_BYTES+1))).error.code,'invalid_request');
  await provider.close();
});
test('Spotify requires explicit selection; queued cancellation never creates an output file',async()=>{
  const dir=await fs.mkdtemp(path.join(os.tmpdir(),'soundflow-queue-'));const provider=new MediaProvider(dir);
  assert.throws(()=>provider.downloads.enqueue({url:'https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC'}),e=>e.code==='match_required');
  assert.throws(()=>provider.downloads.enqueue({url:'https://youtu.be/dQw4w9WgXcQ',matched_from:{source:'spotify'}}),e=>e.code==='match_required');
  const queued=provider.downloads.enqueue({url:'https://youtu.be/dQw4w9WgXcQ',title:'test',source:'youtube'});
  provider.downloads.cancel(queued.job.id);await provider.close();
  assert.equal(provider.downloads.get(queued.job.id).status,'cancelled');assert.deepEqual(await fs.readdir(dir),[]);await fs.rmdir(dir);
  assert.equal(safeFilename('CON'),'_CON');assert.equal(safeFilename('a/b\\c:*?%'),'a b c');
});
test('playback cache prunes only its own files and bounds retained files',async()=>{
  const dir=await fs.mkdtemp(path.join(os.tmpdir(),'soundflow-cache-'));const audio=path.join(dir,'provider-audio');await fs.mkdir(audio);
  const provider=new MediaProvider(path.join(dir,'downloads'),audio);
  for(let i=0;i<10;i++)await fs.writeFile(path.join(audio,`sf-audio-00000000-0000-0000-0000-${String(i).padStart(12,'0')}.m4a`),Buffer.alloc(1024));
  await fs.writeFile(path.join(audio,'keep-user-file.txt'),'preserve');await fs.writeFile(path.join(audio,'sf-audio-00000000-0000-0000-0000-000000000099.mp3.part'),'unfinished');
  await provider.audioCache.prune();const files=await fs.readdir(audio);assert.equal(files.filter(f=>f.endsWith('.m4a')).length,8);assert.ok(files.includes('keep-user-file.txt'));assert.ok(!files.some(f=>f.endsWith('.part')));
  await provider.close();await fs.rm(dir,{recursive:true});
});
test('Spotify confirmation is required even when counterpart audio is cached or preparing',async()=>{
  const dir=await fs.mkdtemp(path.join(os.tmpdir(),'soundflow-match-cache-'));const audio=path.join(dir,'provider-audio');await fs.mkdir(audio);
  const provider=new MediaProvider(path.join(dir,'downloads'),audio);const file=path.join(audio,'sf-audio-00000000-0000-0000-0000-000000000001.m4a');await fs.writeFile(file,'cached counterpart');
  const track={url:'https://youtu.be/dQw4w9WgXcQ',matched_from:{id:'spotify:4uLU6hMCjMI75M1A2tKUQC',source:'spotify'}};
  const url=canonicalURL(track.url).url;provider.audioCache.paths.set(url,file);
  try {
    for(const match_confirmed of [undefined,false,'true']) {
      const reply=await handleRequest(provider,JSON.stringify({id:1,method:'get_playback_file',args:[{...track,match_confirmed}]}));
      assert.equal(reply.error?.code,'match_required');assert.equal(reply.result,null);
    }
    const confirmed=await handleRequest(provider,JSON.stringify({id:2,method:'get_playback_file',args:[{...track,match_confirmed:true}]}));assert.equal(confirmed.result,file);
    provider.audioCache.paths.delete(url);provider.audioCache.pending.set(url,Promise.resolve(file));
    const preparing=await handleRequest(provider,JSON.stringify({id:3,method:'get_playback_file',args:[{...track,match_confirmed:false}]}));assert.equal(preparing.error?.code,'match_required');
    assert.equal(provider.audioCache.queue.list().length,0);assert.equal(await fs.readFile(file,'utf8'),'cached counterpart');
  } finally {await provider.close();await fs.rm(dir,{recursive:true});}
});
test('bundled worker uses no system Python/Node, stdout is JSONL, framing recovers and EOF exits',async()=>{
  const dir=await fs.mkdtemp(path.join(os.tmpdir(),'soundflow-worker-'));
  const child=spawn(path.join(project,'resources/runtime/soundflow-node.exe'),[path.join(project,'media-provider/dist/worker.cjs'),'--data-dir',dir,'--download-dir',path.join(dir,'downloads')],{windowsHide:true,env:{...process.env,PATH:''},stdio:['pipe','pipe','pipe']});
  const stdout=[];const stderr=[];child.stdout.on('data',v=>stdout.push(v));child.stderr.on('data',v=>stderr.push(v));
  const done=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',resolve);});
  child.stdin.write('{"id":1,"method":"ping","args":[]}\n');child.stdin.write('{"id":2,"method":"get_downloads","args":[]}\n');
  child.stdin.write(Buffer.alloc(MAX_REQUEST_BYTES+10,0x78));child.stdin.write('\n');child.stdin.write(Buffer.from([0xc3,0x28,10]));
  child.stdin.write('{"id":3,"method":"invalid","args":[]}\n');child.stdin.write('{"id":4,"method":"ping","args":[]}\n');child.stdin.end('{"id":5');
  const exit=await done;assert.equal(exit,0,Buffer.concat(stderr).toString());
  const lines=Buffer.concat(stdout).toString().trim().split('\n').map(JSON.parse);assert.equal(lines.length,7);
  assert.equal(lines[0].result.engine,'youtubei.js');assert.equal(lines[0].result.runtime_bundled,true);assert.equal(lines[0].result.runtime_version,'v24.21.0');
  assert.deepEqual(lines[1].result,[]);assert.equal(lines[2].error.code,'invalid_request');assert.equal(lines[3].error.code,'invalid_request');assert.equal(lines[4].error.code,'unsupported_method');assert.equal(lines[5].id,4);assert.match(lines[6].error.message,/Truncated/);
  await fs.rm(dir,{recursive:true});
});
