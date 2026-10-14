import { Innertube, Platform, Log } from 'youtubei.js/web';
import vm from 'node:vm';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';

export const MAX_REQUEST_BYTES = 4 * 1024 * 1024;
export const MAX_RESPONSE_BYTES = 16 * 1024 * 1024;
const MAX_MEDIA_BYTES = 512 * 1024 * 1024;
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36';
const SC_HEADERS = { 'user-agent': UA, origin: 'https://soundcloud.com', referer: 'https://soundcloud.com/' };
const NETWORK_DOMAINS = ['youtube.com','youtu.be','googlevideo.com','ytimg.com','google.com','googleapis.com','ggpht.com','soundcloud.com','sndcdn.com','soundcloud.cloud','spotify.com','scdn.co'];
const GENRES = { lofi:'lofi hip hop chill beats', synthwave:'synthwave retrowave electronic', phonk:'drift phonk bass', deephouse:'deep house vocal chill', rock:'classic rock modern rock', gaming:'gaming music NCS', ambient:'ambient relaxation peaceful', ua:'українська музика інді' };
Log.setLevel(Log.Level.NONE);
// Only the extracted player function runs here, with no Node bindings or dynamic code generation.
Platform.shim.eval = data => vm.runInNewContext(`(()=>{${data.output}})()`, Object.create(null), {
  timeout: 2000, contextCodeGeneration: { strings: false, wasm: false }
});

export class ProviderError extends Error {
  constructor(message, code='provider_error') { super(message); this.code=code; }
}
const invalid = message => { throw new ProviderError(message,'invalid_request'); };
export function cleanError(error) {
  return String(error?.message || error || 'Медіасервіс недоступний.').replace(/https?:\/\/[^\s"']+/g,'[provider URL]').replace(/[\x00-\x1f]/g,' ').slice(0,500);
}
function text(value, max=500) { return typeof value==='string' ? value.trim().slice(0,max) : ''; }
function number(value, fallback=0) { return Number.isFinite(Number(value)) ? Number(value) : fallback; }
function limit(value, fallback=16,max=40) { return Math.max(1,Math.min(max,Math.trunc(number(value,fallback)))); }
function domain(host,name) { return host===name || host.endsWith(`.${name}`); }
function networkURL(value) {
  const url=new URL(value);
  if(url.protocol!=='https:' || url.username || url.password || (url.port && url.port!=='443') || !NETWORK_DOMAINS.some(d=>domain(url.hostname,d))) invalid('Недозволена адреса медіасервісу.');
  return url;
}
async function providerFetch(input, init={}) {
  let url=networkURL(input instanceof Request ? input.url : String(input));
  const signal=AbortSignal.any([AbortSignal.timeout(30000), ...(init.signal ? [init.signal] : [])]);
  // Explicit redirects keep a provider response from accessing arbitrary local/network addresses.
  let requestInit={...(input instanceof Request ? {method:input.method,headers:input.headers,body:input.body,duplex:'half'} : {}),...init,signal,redirect:'manual'};
  for(let hop=0;hop<5;hop++) {
    const response=await fetch(url,requestInit);
    if([301,302,303,307,308].includes(response.status)) {
      const location=response.headers.get('location'); await response.body?.cancel();
      if(!location) throw new ProviderError('Медіасервіс повернув некоректне перенаправлення.');
      url=networkURL(new URL(location,url));
      if(response.status===303 || ([301,302].includes(response.status) && requestInit.method==='POST')) requestInit={...requestInit,method:'GET',body:undefined};
      continue;
    }
    if(!response.ok) { await response.body?.cancel(); throw new ProviderError(`Медіасервіс повернув HTTP ${response.status}.`); }
    return response;
  }
  throw new ProviderError('Забагато перенаправлень медіасервісу.');
}
async function boundedBody(response,max=12*1024*1024) {
  const chunks=[];let size=0;
  for await(const chunk of response.body) { size+=chunk.length; if(size>max) throw new ProviderError('Відповідь медіасервісу завелика.');chunks.push(chunk); }
  return Buffer.concat(chunks);
}
async function getJSON(url,init) { return JSON.parse((await boundedBody(await providerFetch(url,init))).toString('utf8')); }
async function getText(url,init) { return (await boundedBody(await providerFetch(url,init))).toString('utf8'); }

export function canonicalURL(value,profile=false) {
  value=text(value,2000);
  if(value.startsWith('spotify:track:')) value=`https://open.spotify.com/track/${value.slice(14)}`;
  if(value.startsWith('spotify:playlist:')) value=`https://open.spotify.com/playlist/${value.slice(17)}`;
  if(value.startsWith('spotify:album:')) value=`https://open.spotify.com/album/${value.slice(14)}`;
  let url;try { url=new URL(value); } catch { invalid('Вставте посилання YouTube, SoundCloud або Spotify.'); }
  if(!['https:','http:'].includes(url.protocol) || url.username || url.password || (url.port && !['80','443'].includes(url.port)) || /%|\\/.test(url.pathname)) invalid('Некоректне посилання медіасервісу.');
  const host=url.hostname.toLowerCase();const parts=url.pathname.split('/').filter(Boolean);
  if(['youtube.com','www.youtube.com','m.youtube.com','music.youtube.com','youtu.be','www.youtu.be'].includes(host)) {
    let id=host.endsWith('youtu.be') ? parts[0] : parts[0]==='watch' ? url.searchParams.get('v') : ['shorts','embed','live'].includes(parts[0]) ? parts[1] : null;
    if(id && /^[\w-]{11}$/.test(id)) return {source:'youtube',id,url:`https://www.youtube.com/watch?v=${id}`,kind:'track'};
    if(profile && parts[0]==='playlist' && /^[\w-]{10,200}$/.test(url.searchParams.get('list')||'')) return {source:'youtube',id:url.searchParams.get('list'),url:`https://www.youtube.com/playlist?list=${url.searchParams.get('list')}`,kind:'playlist'};
    if(profile && ((parts[0]?.startsWith('@') && /^[\w@.-]+$/.test(parts[0])) || (['channel','c','user'].includes(parts[0]) && /^[\w.-]+$/.test(parts[1]||'')))) return {source:'youtube',url:`https://www.youtube.com/${parts[0]}${parts[1] && !parts[0].startsWith('@') ? '/'+parts[1] : ''}`,kind:'profile'};
    invalid('Потрібне посилання на відео, публічний канал або плейлист YouTube.');
  }
  if(['soundcloud.com','www.soundcloud.com','m.soundcloud.com'].includes(host)) {
    const collections=['likes','tracks','albums','reposts','popular-tracks','following','followers'];
    if(!parts.length || parts.some(p=>!/[A-Za-z0-9_-]/.test(p)||!/^[A-Za-z0-9_-]+$/.test(p)) || parts.length>3) invalid('Некоректне посилання SoundCloud.');
    if(parts.length===1 && profile) return {source:'soundcloud',url:`https://soundcloud.com/${parts[0]}`,kind:'profile'};
    if(parts.length===3 && parts[1]==='sets') return {source:'soundcloud',url:`https://soundcloud.com/${parts[0]}/sets/${parts[2]}`,kind:'playlist'};
    if(parts.length<2 || collections.includes(parts[1]) || (parts.length===3 && !/^s-[\w-]+$/.test(parts[2]))) invalid('Потрібне посилання на пісню або публічний профіль SoundCloud.');
    const canonical=new URL(`https://soundcloud.com/${parts.join('/')}`);
    const secret=url.searchParams.get('secret_token');if(secret && /^s-[\w-]+$/.test(secret))canonical.searchParams.set('secret_token',secret);
    return {source:'soundcloud',url:String(canonical),kind:'track'};
  }
  if(host==='open.spotify.com') {
    if(parts[0]?.startsWith('intl-'))parts.shift();if(parts[0]==='embed')parts.shift();
    if(['playlist','album'].includes(parts[0]) && /^[A-Za-z0-9]{22}$/.test(parts[1]||'')) return {source:'spotify',id:parts[1],url:`https://open.spotify.com/${parts[0]}/${parts[1]}`,kind:parts[0]};
    if(parts[0]!=='track' || !/^[A-Za-z0-9]{22}$/.test(parts[1]||''))invalid('Для Spotify потрібне посилання на пісню, плейлист або альбом.');
    return {source:'spotify',id:parts[1],url:`https://open.spotify.com/track/${parts[1]}`,kind:'track'};
  }
  invalid('Підтримуються тільки YouTube, SoundCloud та Spotify.');
}
function durationText(duration) { const d=Math.max(0,Math.round(duration));return `${Math.floor(d/60)}:${String(d%60).padStart(2,'0')}`; }
function trackBase(source,id,title,artist,duration,thumbnail,url) {
  duration=Math.max(0,number(duration));
  return {id:`${source==='youtube'?'yt':source==='soundcloud'?'sc':'sp'}_${id}`,title:text(title)||'Без назви',artist:text(artist)||source,duration,duration_formatted:durationText(duration),thumbnail:thumbnail||'',url,source,source_url:url,downloadable:source!=='spotify',is_live:false,preview_start:Math.max(0,Math.min(duration*.4,duration-30)),preview_duration:Math.min(30,duration||30)};
}
function ytTrack(video,artistFallback='YouTube') {
  const id=video.id||video.video_id||video.content_id;
  if(!/^[\w-]{11}$/.test(id||''))return null;
  const badge=video.content_image?.overlays?.flatMap(v=>v.badges||[]).find(b=>/^(\d+:)+\d+$/.test(b.text||''))?.text;
  const badgeDuration=badge?.split(':').reduce((value,part)=>value*60+Number(part),0);
  return trackBase('youtube',id,video.title?.toString?.()||video.metadata?.title?.toString?.()||video.title,video.author?.name||video.channel?.name||video.author||(typeof artistFallback==='string'?artistFallback:'YouTube'),video.duration?.seconds||video.duration||badgeDuration,video.thumbnails?.at(-1)?.url||video.content_image?.image?.at(0)?.url||`https://i.ytimg.com/vi/${id}/hqdefault.jpg`,`https://www.youtube.com/watch?v=${id}`);
}
function patchMp4Duration(buf, durationSec) {
  if (!Buffer.isBuffer(buf) || !durationSec || durationSec <= 0) return;
  let offset = 0;
  while (offset + 8 <= buf.length) {
    const size = buf.readUInt32BE(offset);
    const type = buf.slice(offset + 4, offset + 8).toString('ascii');
    const actualSize = size === 1 ? Number(buf.readBigUInt64BE(offset + 8)) : size;
    if (actualSize <= 0 || offset + actualSize > buf.length) break;
    if (type === 'moov') {
      patchMoov(buf, offset + (size === 1 ? 16 : 8), offset + actualSize, durationSec);
      break;
    }
    offset += actualSize;
  }
}
function patchMoov(buf, offset, end, durationSec) {
  let movieTimescale = 1000;
  while (offset + 8 <= end) {
    const size = buf.readUInt32BE(offset);
    const type = buf.slice(offset + 4, offset + 8).toString('ascii');
    const actualSize = size === 1 ? Number(buf.readBigUInt64BE(offset + 8)) : size;
    if (actualSize <= 0) break;
    const bodyOffset = offset + (size === 1 ? 16 : 8);
    if (type === 'mvhd' && bodyOffset + 20 <= end) {
      const version = buf.readUInt8(bodyOffset);
      if (version === 0) {
        movieTimescale = buf.readUInt32BE(bodyOffset + 12) || 1000;
        const dur = Math.min(0xffffffff, Math.round(durationSec * movieTimescale));
        buf.writeUInt32BE(dur, bodyOffset + 16);
      } else if (version === 1 && bodyOffset + 32 <= end) {
        movieTimescale = buf.readUInt32BE(bodyOffset + 20) || 1000;
        const dur = BigInt(Math.round(durationSec * movieTimescale));
        buf.writeBigUInt64BE(dur, bodyOffset + 24);
      }
    } else if (type === 'trak') {
      patchTrak(buf, bodyOffset, offset + actualSize, durationSec, movieTimescale);
    }
    offset += actualSize;
  }
}
function patchTrak(buf, offset, end, durationSec, movieTimescale) {
  while (offset + 8 <= end) {
    const size = buf.readUInt32BE(offset);
    const type = buf.slice(offset + 4, offset + 8).toString('ascii');
    const actualSize = size === 1 ? Number(buf.readBigUInt64BE(offset + 8)) : size;
    if (actualSize <= 0) break;
    const bodyOffset = offset + (size === 1 ? 16 : 8);
    if (type === 'tkhd' && bodyOffset + 24 <= end) {
      const version = buf.readUInt8(bodyOffset);
      if (version === 0) {
        const dur = Math.min(0xffffffff, Math.round(durationSec * movieTimescale));
        buf.writeUInt32BE(dur, bodyOffset + 20);
      } else if (version === 1 && bodyOffset + 32 <= end) {
        const dur = BigInt(Math.round(durationSec * movieTimescale));
        buf.writeBigUInt64BE(dur, bodyOffset + 24);
      }
    } else if (type === 'mdia') {
      patchMdia(buf, bodyOffset, offset + actualSize, durationSec);
    }
    offset += actualSize;
  }
}
function patchMdia(buf, offset, end, durationSec) {
  while (offset + 8 <= end) {
    const size = buf.readUInt32BE(offset);
    const type = buf.slice(offset + 4, offset + 8).toString('ascii');
    const actualSize = size === 1 ? Number(buf.readBigUInt64BE(offset + 8)) : size;
    if (actualSize <= 0) break;
    const bodyOffset = offset + (size === 1 ? 16 : 8);
    if (type === 'mdhd' && bodyOffset + 20 <= end) {
      const version = buf.readUInt8(bodyOffset);
      if (version === 0) {
        const timescale = buf.readUInt32BE(bodyOffset + 12) || 44100;
        const dur = Math.min(0xffffffff, Math.round(durationSec * timescale));
        buf.writeUInt32BE(dur, bodyOffset + 16);
      } else if (version === 1 && bodyOffset + 28 <= end) {
        const timescale = buf.readUInt32BE(bodyOffset + 20) || 44100;
        const dur = BigInt(Math.round(durationSec * timescale));
        buf.writeBigUInt64BE(dur, bodyOffset + 24);
      }
    }
    offset += actualSize;
  }
}

function scTrack(track) {
  if(!track?.id || !track.permalink_url)return null;
  let dur = number(track.duration);
  if(dur > 10000) dur = dur / 1000;
  if(!dur || dur <= 0) {
    for (const t of track.media?.transcodings || []) {
      const td = number(t.duration);
      if (td > 0) { dur = td > 10000 ? td / 1000 : td; break; }
    }
  }
  const result=trackBase('soundcloud',track.id,track.title,track.user?.username,dur,(track.artwork_url||track.user?.avatar_url||'').replace('-large.','-t500x500.'),canonicalURL(track.permalink_url).url);
  result.downloadable=track.policy==='ALLOW' && track.media?.transcodings?.length>0;result.genre=text(track.genre,100);return result;
}

function detectLanguage(texts) {
  const allText = texts.join(' ').toLowerCase();
  const ukLetters = (allText.match(/[іїєґ]/g) || []).length;
  const ruLetters = (allText.match(/[ыэъё]/g) || []).length;
  const cyrillic = (allText.match(/[\u0400-\u04ff]/g) || []).length;
  const latin = (allText.match(/[a-z]/g) || []).length;

  const ukWords = (allText.match(/\b(не|ти|я|ми|що|як|все|мене|тобі|біль|ніч|очі|серце|дощ|сумно|життя|пісня|любов|кохання|депресія|музика)\b/g) || []).length;
  const ruWords = (allText.match(/\b(не|ты|я|мы|что|как|всё|меня|тебе|боль|ночь|глаза|сердце|дождь|грустно|жизнь|песня|любовь|тоска|грусть|депрессия|музыка)\b/g) || []).length;

  const ukScore = ukLetters * 4 + ukWords * 3;
  const ruScore = ruLetters * 4 + ruWords * 3;

  if (ukScore > ruScore && ukScore > 0) return 'uk';
  if (ruScore > ukScore && ruScore > 0) return 'ru';
  if (cyrillic > latin) {
    return ruWords >= ukWords ? 'ru' : 'uk';
  }
  return 'en';
}

function detectTheme(texts) {
  const allText = texts.join(' ').toLowerCase();
  const scores = {
    depressive: (allText.match(/депресс|депрес|грустн|сумн|тоск|боль|біль|печал|слез|слёз|плач|sad|cry|alone|dark|heartbreak|melanchol|пост-панк|post-punk|думер|doomer|дождь|дощ|одиноч|безысходн/g) || []).length * 2,
    phonk: (allText.match(/phonk|фонк|drift|дрифт|bass|kordhell|playaphonk|aggressive|memphis|speed up/g) || []).length * 2,
    rock: (allText.match(/rock|рок|metal|метал|alternative|альтернатив|punk|панк|grunge|грандж|guitar|гитар/g) || []).length * 2,
    rap: (allText.match(/рэп|реп|rap|trap|треп|трэп|hip hop|хип хоп|drill|дрілл|beat|бит/g) || []).length * 2,
    chill: (allText.match(/lofi|lo-fi|chill|чилл|relax|релакс|ambient|спокой|тиха|sleep|night/g) || []).length * 2,
    pop: (allText.match(/pop|поп|dance|танц|club|клуб|edm|house|хаус|electro|party/g) || []).length * 2,
  };
  let best = 'general';
  let max = 0;
  for (const [k, v] of Object.entries(scores)) {
    if (v > max) { max = v; best = k; }
  }
  return best;
}

function cleanSongTitle(title = '') {
  return String(title)
    .replace(/\(.*?\)|\[.*?\]/g, ' ')
    .replace(/\b(official\s+video|official\s+audio|audio|music\s+video|lyrics?|клип|премьера|трек|remix|ремикс|slowed|reverb|speed\s+up|cover|кавер)\b/gi, ' ')
    .replace(/[-_–—|/\\+:]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function matchesLanguage(track, lang) {
  if (!track || !track.title) return false;
  const text = `${track.title} ${track.artist || ''}`.toLowerCase();
  const cyrillic = (text.match(/[\u0400-\u04ff]/g) || []).length;
  const ukSpecific = /[іїєґ]/i.test(text);
  const ruSpecific = /[ыэъё]/i.test(text);

  if (lang === 'uk') {
    if (ruSpecific) return false;
    return ukSpecific || cyrillic >= 2;
  }
  if (lang === 'ru') {
    if (ukSpecific) return false;
    return ruSpecific || cyrillic >= 2;
  }
  if (lang === 'en') {
    return cyrillic === 0;
  }
  return true;
}

export class MediaProvider {
  constructor(downloadDir,playbackDir=path.join(path.dirname(path.resolve(downloadDir)),'provider-audio')) { this.downloads=new DownloadQueue(this,path.resolve(downloadDir));this.audioCache=new AudioCache(this,path.resolve(playbackDir));this.scClient=null;this.scRaw=new Map();this.streams=new Map(); }
  async youtube() {
    if(!this.ytPromise)this.ytPromise=Innertube.create({lang:'en',location:'US',fetch:providerFetch}).catch(e=>{this.ytPromise=null;throw e;});
    return this.ytPromise;
  }
  async soundcloudClient(refresh=false) {
    if(refresh)this.scClient=null;
    if(!this.scClient)this.scClient=(async()=>{
      const html=await getText('https://soundcloud.com/',{headers:SC_HEADERS});
      const scripts=[...html.matchAll(/https:\/\/a-v2\.sndcdn\.com\/[^"\s]+\.js/g)].map(m=>m[0]).slice(-16).reverse();
      for(const script of scripts) { const js=await getText(script,{headers:SC_HEADERS});const id=js.match(/client_id:"([A-Za-z0-9]{32})"/)?.[1];if(id)return id; }
      throw new ProviderError('SoundCloud змінив публічний API. Спробуйте пізніше.');
    })().catch(e=>{this.scClient=null;throw e;});
    return this.scClient;
  }
  async scAPI(endpoint,params={},signal) {
    const url=new URL(`https://api-v2.soundcloud.com/${endpoint.replace(/^\//,'')}`);
    for(const [key,value] of Object.entries({...params,client_id:await this.soundcloudClient()}))url.searchParams.set(key,String(value));
    return getJSON(url,{headers:SC_HEADERS,signal});
  }
  rememberSC(raw) { if(raw?.id) { if(this.scRaw.size>=256)this.scRaw.delete(this.scRaw.keys().next().value);this.scRaw.set(raw.permalink_url,raw); }return scTrack(raw); }
  async search(query,source='all',count=16) {
    query=text(query,300);count=limit(count);if(!query)return [];
    if(/^https?:\/\/|^spotify:/.test(query))return [await this.resolve(query)];
    source=text(source,30).toLowerCase();if(!['all','youtube','yt','soundcloud','spotify'].includes(source))invalid('Непідтримуваний медіасервіс.');
    if(source==='spotify')throw new ProviderError('Пошук Spotify потребує API-акаунта. Вставте посилання на пісню.','authentication_required');
    const tasks=[];
    if(source==='all'||['youtube','yt'].includes(source))tasks.push((async()=>{const yt=await this.youtube();const found=await yt.search(query,{type:'video'});return [...found.videos].map(ytTrack).filter(Boolean).slice(0,count);})());
    if(source==='all'||source==='soundcloud')tasks.push((async()=>{const found=await this.scAPI('search/tracks',{q:query,limit:count});return (found.collection||[]).map(v=>this.rememberSC(v)).filter(Boolean).slice(0,count);})());
    const results=await Promise.allSettled(tasks);const good=results.filter(r=>r.status==='fulfilled');
    if(!good.length)throw results[0].reason;
    const output=[];const seen=new Set();for(let i=0;i<count;i++)for(const result of good) { const track=result.value[i];if(track && !seen.has(track.id)){output.push(track);seen.add(track.id);} }
    return output.slice(0,count);
  }
  async resolve(value) {
    const ref=canonicalURL(value);
    if(ref.source==='spotify') {
      const metadata=await getJSON(`https://open.spotify.com/oembed?url=${encodeURIComponent(ref.url)}`);
      return {...trackBase('spotify',ref.id,metadata.title,'Spotify',0,metadata.thumbnail_url,ref.url),requires_match:true,downloadable:false,playback_mode:'spotify_embed',embed_url:`https://open.spotify.com/embed/track/${ref.id}`,metadata_provider:'spotify_oembed',notice:'Оберіть відповідну пісню на YouTube або SoundCloud для відтворення й завантаження.'};
    }
    if(ref.source==='soundcloud')return this.rememberSC(await this.scAPI('resolve',{url:ref.url}));
    const yt=await this.youtube();const info=await yt.getBasicInfo(ref.id,{client:'IOS'});const basic=info.basic_info;
    if(!basic?.title)throw new ProviderError('Пісня YouTube недоступна.');
    return {...trackBase('youtube',ref.id,basic.title,basic.author,basic.duration,basic.thumbnail?.at(-1)?.url||`https://i.ytimg.com/vi/${ref.id}/hqdefault.jpg`,ref.url),is_live:!!basic.is_live,downloadable:!basic.is_live};
  }
  async stream(track,refresh=false,signal) {
    if(!track || typeof track!=='object')invalid('Некоректна пісня.');
    const ref=canonicalURL(track.url||track.source_url);if(ref.source==='spotify')throw new ProviderError('Спочатку оберіть відповідну пісню YouTube або SoundCloud.','match_required');
    const cached=this.streams.get(ref.url);if(!refresh && cached && cached.expires>Date.now())return cached;
    signal?.throwIfAborted();let media;
    if(ref.source==='youtube') {
      const yt=await this.youtube();let last;
      // Public iOS audio URLs are currently progressive; other clients may expose SABR only.
      for(const client of ['TV_SIMPLY','YTMUSIC','VISIONOS','IOS']) {
        try {
          const info=await yt.getBasicInfo(ref.id,{client});signal?.throwIfAborted();
          if(info.playability_status?.status!=='OK' || info.basic_info.is_live)throw new ProviderError('Ця пісня недоступна для аудіовідтворення.');
          const formats=(info.streaming_data?.adaptive_formats||[]).filter(f=>f.has_audio&&!f.has_video&&f.mime_type.startsWith('audio/mp4')&&(f.url||f.signature_cipher||f.cipher)&&!f.drm_families?.length);
          formats.sort((a,b)=>(Number(b.mime_type.startsWith('audio/mp4'))-Number(a.mime_type.startsWith('audio/mp4'))) || b.bitrate-a.bitrate);
          if(!formats.length)throw new ProviderError('YouTube не надав прямий аудіопотік.');
          const f=formats[0];const url=await f.decipher(yt.session.player);networkURL(url);
          // A prefix alone is insufficient: some clients only grant a short preview.
          const offset=Math.max(0,Math.min(2097152,number(f.content_length)-1024));
          const probe=await providerFetch(url,{headers:{range:`bytes=${offset}-${offset+15}`},signal});await probe.body?.cancel();
          media={url,ext:f.mime_type.includes('mp4')?'m4a':'webm',mime:f.mime_type.split(';')[0],size:f.content_length||0,hls:false};break;
        } catch(e) { last=e;if(signal?.aborted)throw e; }
      }
      if(!media)throw last;
    } else {
      const raw=this.scRaw.get(ref.url)||await this.scAPI('resolve',{url:ref.url},signal);
      if(raw.policy!=='ALLOW')throw new ProviderError('SoundCloud не дозволяє відтворення цієї пісні.');
      this.rememberSC(raw);
      const transcodings=(raw.media?.transcodings||[]).filter(t=>['audio/mpeg','audio/mp4'].some(m=>t.format?.mime_type?.startsWith(m)));
      transcodings.sort((a,b)=>Number(b.format.protocol==='progressive')-Number(a.format.protocol==='progressive') || Number(b.format.mime_type.startsWith('audio/mpeg'))-Number(a.format.mime_type.startsWith('audio/mpeg')));
      let last;
      for(const t of transcodings) {
        try { const url=networkURL(t.url);url.searchParams.set('client_id',await this.soundcloudClient());if(raw.track_authorization)url.searchParams.set('track_authorization',raw.track_authorization);
          const result=await getJSON(url,{headers:SC_HEADERS,signal});networkURL(result.url);
          media={url:result.url,ext:t.format.mime_type.startsWith('audio/mp4')?'m4a':'mp3',mime:t.format.mime_type.split(';')[0],size:0,hls:t.format.protocol==='hls'};break;
        }catch(e){last=e;if(signal?.aborted)throw e;}
      }
      if(!media)throw last||new ProviderError('SoundCloud не надав аудіопотік.');
    }
    signal?.throwIfAborted();media.expires=Date.now()+5*60*1000;
    if(this.streams.size>=128)this.streams.delete(this.streams.keys().next().value);this.streams.set(ref.url,media);return media;
  }
  async importProfile(source,identifier) {
    source=text(source,30).toLowerCase();identifier=text(identifier,2000);
    if(source==='spotify') {
      if(identifier.startsWith('spotify:playlist:'))identifier=`https://open.spotify.com/playlist/${identifier.slice(17)}`;
      if(identifier.startsWith('spotify:album:'))identifier=`https://open.spotify.com/album/${identifier.slice(14)}`;
      if(identifier.startsWith('spotify:track:'))identifier=`https://open.spotify.com/track/${identifier.slice(14)}`;
      const playlistMatch = identifier.match(/(playlist|album)\/([A-Za-z0-9]{22})/);
      if(playlistMatch) {
        const type = playlistMatch[1];
        const id = playlistMatch[2];
        try {
          const html = await getText(`https://open.spotify.com/embed/${type}/${id}`, { headers: { 'user-agent': UA } });
          const m = html.match(/<script id="__NEXT_DATA__"[^>]*>(.*?)<\/script>/s) || html.match(/<script id="initial-state"[^>]*>(.*?)<\/script>/s);
          if(m) {
            const data = JSON.parse(m[1]);
            const entity = data.props?.pageProps?.state?.data?.entity;
            if(entity && Array.isArray(entity.trackList) && entity.trackList.length > 0) {
              const cover = entity.coverArt?.sources?.[0]?.url || '';
              const albumTitle = entity.title || entity.name || 'Spotify';
              return entity.trackList.slice(0, 100).map(t => {
                const trackId = (t.uri || '').replace('spotify:track:', '') || t.uid || randomUUID();
                const duration = Math.max(0, number(t.duration) / 1000);
                const trackUrl = `https://open.spotify.com/track/${trackId}`;
                return {
                  ...trackBase('spotify', trackId, t.title, t.subtitle || albumTitle, duration, cover, trackUrl),
                  requires_match: true,
                  downloadable: false,
                  playback_mode: 'spotify_embed',
                  embed_url: `https://open.spotify.com/embed/track/${trackId}`,
                  metadata_provider: 'spotify_embed',
                  notice: 'Оберіть відповідну пісню на YouTube або SoundCloud для відтворення й завантаження.'
                };
              });
            }
          }
        } catch(e) {
          throw new ProviderError('Не вдалося розібрати плейлист або альбом Spotify: ' + cleanError(e));
        }
      }
      if(/^https?:\/\/open\.spotify\.com\/track\//.test(identifier)) {
        const track = await this.resolve(identifier);
        if(track.source !== 'spotify') invalid('Потрібне посилання Spotify.');
        return [track];
      }
      return (await this.search(identifier, 'all', 20));
    }
    if(source==='soundcloud') {
      if(!identifier.startsWith('http'))identifier=`https://soundcloud.com/${identifier.replace(/^@/,'')}`;
      const ref=canonicalURL(identifier,true);if(ref.source!=='soundcloud')invalid('Потрібне посилання SoundCloud.');
      const profile=await this.scAPI('resolve',{url:ref.url});
      if(profile.kind==='track')return [this.rememberSC(profile)];
      if(profile.kind==='playlist') {
        const tracks = (profile.tracks || []).map(t => this.rememberSC(t)).filter(Boolean);
        if(tracks.length > 0) return tracks.slice(0, 100);
      }
      if(profile.kind==='user'&&profile.id) {
        const tracks = [];
        const seen = new Set();
        const addTracks = (list) => {
          for(const item of (list || [])) {
            const raw = item.track || item;
            if(raw && raw.id && !seen.has(raw.id)) {
              seen.add(raw.id);
              const t = this.rememberSC(raw);
              if(t) tracks.push(t);
            }
          }
        };
        try {
          const likes = await this.scAPI(`users/${profile.id}/likes`, { limit: 50 });
          addTracks(likes.collection);
        } catch {}
        try {
          const playlists = await this.scAPI(`users/${profile.id}/playlists`, { limit: 5 });
          for(const p of (playlists.collection || [])) addTracks(p.tracks);
        } catch {}
        try {
          const userTracks = await this.scAPI(`users/${profile.id}/tracks`, { limit: 50 });
          addTracks(userTracks.collection);
        } catch {}
        if(tracks.length > 0) return tracks.slice(0, 100);
        throw new ProviderError('У профілі SoundCloud не знайдено пісень чи вподобань.');
      }
      throw new ProviderError('Публічний профіль або плейлист SoundCloud не знайдено.');
    }
    if(['youtube','yt'].includes(source)) {
      if(identifier.startsWith('@'))identifier=`https://www.youtube.com/${identifier}`;
      if(!identifier.startsWith('http')) {
        const yt = await this.youtube();
        const found = await yt.search(identifier, { type: 'video' });
        return [...found.videos].map(ytTrack).filter(Boolean).slice(0, 40);
      }
      const ref=canonicalURL(identifier,true);if(ref.source!=='youtube')invalid('Потрібне посилання YouTube.');
      if(ref.kind==='track')return [await this.resolve(ref.url)];
      const yt=await this.youtube();
      if(ref.kind==='playlist'){const playlist=await yt.getPlaylist(ref.id);return [...playlist.videos].map(ytTrack).filter(Boolean).slice(0,60);}
      const endpoint=await yt.resolveURL(ref.url);const id=endpoint.payload?.browseId;
      if(!id)throw new ProviderError('Публічний канал YouTube не знайдено.');
      const channel=await yt.getChannel(id);const videos=await channel.getVideos();return [...videos.videos].map(video=>ytTrack(video,channel.metadata?.title)).filter(Boolean).slice(0,60);
    }
    invalid('Непідтримувана платформа.');
  }
  async detectConnectedAccounts(email='', name='') {
    email=text(email,254);name=text(name,100);
    const username = (email.split('@')[0] || name || 'user').toLowerCase();
    const result = {
      youtube: { found: !!email, name: name || username, identifier: email ? `@${username.replace(/[^A-Za-z0-9_-]/g,'')}` : '' },
      soundcloud: { found: false, name: '', identifier: '' },
      spotify: { found: false, name: '', identifier: '' }
    };
    try {
      const sc = await this.scAPI('resolve', { url: `https://soundcloud.com/${username}` });
      if(sc && sc.kind === 'user' && sc.username) {
        result.soundcloud = { found: true, name: sc.username, identifier: sc.permalink_url || `https://soundcloud.com/${username}` };
      }
    } catch {}
    return result;
  }
  async getInfiniteRecommendations(context = {}, count = 16) {
    if (!context || typeof context !== 'object') invalid('Некоректний контекст рекомендацій.');
    count = limit(count, 10, 30);
    const tracks = Array.isArray(context.tracks) ? context.tracks : [];
    const history = Array.isArray(context.history) ? context.history : [];
    const excludeIds = new Set([
      ...(Array.isArray(context.exclude_ids) ? context.exclude_ids : []),
      ...tracks.map(t => t.id)
    ].filter(Boolean));
    const seed = Number(context.seed) || 0;

    // 1. Gather text corpus and determine language & theme
    const sampleTexts = [
      text(context.playlist_name, 100),
      ...tracks.slice(0, 30).map(t => `${text(t.title, 100)} ${text(t.artist, 80)} ${text(t.genre, 60)}`)
    ].filter(Boolean);

    const lang = context.preferred_language || detectLanguage(sampleTexts);
    const theme = context.preferred_theme || detectTheme(sampleTexts);

    // 2. Identify artists from playlist
    const artists = [...new Set(tracks.map(t => text(t.artist, 100)).filter(a => a && a !== 'YouTube' && a !== 'SoundCloud' && a !== 'Spotify'))];

    // 3. Known songs for covers / alternate authors ("можуть повторюватись пісні які ти вже слухав але від разних авторів")
    const listenedSongs = [...history, ...tracks]
      .filter(t => t && t.title)
      .map(t => ({
        original_title: cleanSongTitle(t.title),
        original_artist: text(t.artist, 100).toLowerCase()
      }))
      .filter(s => s.original_title.length >= 3);

    // 4. SoundCloud Station logic (SoundCloud Related API)
    const rawCandidates = [];

    // Identify seed track (current track or last played track)
    const seedTrack = context.current_track || tracks[tracks.length - 1 - (seed % Math.max(1, tracks.length))] || tracks[0];
    let scId = null;
    if (seedTrack) {
      if (typeof seedTrack.id === 'string' && seedTrack.id.startsWith('sc_')) {
        scId = seedTrack.id.replace(/^sc_/, '');
      } else if (seedTrack.url && seedTrack.url.includes('soundcloud.com')) {
        try {
          const ref = canonicalURL(seedTrack.url);
          if (ref.source === 'soundcloud') scId = ref.id;
        } catch {}
      }
      if (!scId && seedTrack.title) {
        try {
          const found = await this.scAPI('search/tracks', { q: `${seedTrack.title} ${seedTrack.artist || ''}`, limit: 1 });
          scId = found?.collection?.[0]?.id;
        } catch {}
      }
    }

    if (scId) {
      try {
        const relatedRes = await this.scAPI(`tracks/${scId}/related`, { limit: 20 });
        const relatedTracks = (relatedRes?.collection || []).map(t => this.rememberSC(t)).filter(Boolean);
        rawCandidates.push(...relatedTracks);
      } catch {}
    }

    // 5. Covers and alternate authors on SoundCloud ("можуть повторюватись пісні які ти вже слухав але від разних авторів")
    if (listenedSongs.length > 0) {
      const listened = listenedSongs[seed % listenedSongs.length];
      if (listened?.original_title) {
        try {
          const coverSearch = await this.scAPI('search/tracks', { q: `${listened.original_title} cover`, limit: 6 });
          const coverTracks = (coverSearch?.collection || []).map(t => this.rememberSC(t)).filter(Boolean);
          rawCandidates.push(...coverTracks);
        } catch {}
        try {
          const titleSearch = await this.scAPI('search/tracks', { q: listened.original_title, limit: 6 });
          const titleTracks = (titleSearch?.collection || []).map(t => this.rememberSC(t)).filter(Boolean);
          rawCandidates.push(...titleTracks);
        } catch {}
      }
    }

    // 6. Artist & Genre tracks on SoundCloud
    if (rawCandidates.length < count * 2) {
      if (artists.length > 0) {
        const artist = artists[seed % artists.length];
        try {
          const artistRes = await this.scAPI('search/tracks', { q: artist, limit: 8 });
          const artistTracks = (artistRes?.collection || []).map(t => this.rememberSC(t)).filter(Boolean);
          rawCandidates.push(...artistTracks);
        } catch {}
      }
    }

    // 7. YouTube fallback if SoundCloud provided few candidates
    if (rawCandidates.length < count) {
      try {
        let ytQuery = '';
        if (seedTrack) ytQuery = `${seedTrack.title} ${seedTrack.artist} audio`;
        else if (artists.length > 0) ytQuery = `${artists[0]} music`;
        else ytQuery = `${theme} music`;
        const ytTracks = await this.search(ytQuery, 'youtube', 10);
        rawCandidates.push(...ytTracks);
      } catch {}
    }

    // 8. Filter and Deduplicate candidates
    const candidates = [];
    const seenCandidateIds = new Set(excludeIds);
    const seenTitles = new Set();

    for (const track of rawCandidates) {
      if (!track || !track.id || seenCandidateIds.has(track.id)) continue;

      // Language filter
      if (!matchesLanguage(track, lang)) continue;

      // Duration sanity check: reject < 45s (jingles/previews) or > 660s (11 min mixes)
      if (track.duration > 0 && (track.duration < 45 || track.duration > 660)) continue;

      const cleanedTitle = cleanSongTitle(track.title);
      let isDifferentAuthorCover = false;
      for (const s of listenedSongs) {
        if (cleanedTitle.includes(s.original_title) || s.original_title.includes(cleanedTitle)) {
          const candArtist = text(track.artist, 100).toLowerCase();
          if (s.original_artist && !candArtist.includes(s.original_artist) && !s.original_artist.includes(candArtist)) {
            isDifferentAuthorCover = true;
            break;
          }
        }
      }

      if (!isDifferentAuthorCover && seenTitles.has(cleanedTitle)) continue;

      seenTitles.add(cleanedTitle);
      seenCandidateIds.add(track.id);
      candidates.push({
        ...track,
        is_radio_recommendation: true,
        detected_language: lang,
        detected_theme: theme,
        is_different_author_cover: isDifferentAuthorCover
      });

      if (candidates.length >= count) break;
    }

    return candidates.slice(0, count);
  }
  async dispatch(method,args=[]) {
    const arity={ping:0,search:3,get_discover_tracks:1,resolve_direct_url:1,get_stream:2,get_playback_file:1,get_similar_tracks:3,get_infinite_recommendations:2,match_spotify_track:3,import_external_profile:2,download_track:1,get_downloads:0,get_download_job:1,cancel_download:1,detect_connected_accounts:2};
    if(!Object.hasOwn(arity,method))throw new ProviderError(`Unsupported provider method: ${text(method,100)}`,'unsupported_method');
    if(args.length>arity[method])invalid('Забагато аргументів методу.');
    switch(method) {
      case 'ping':return {protocol:1,engine:'youtubei.js',engine_version:'18.1.0',runtime:'node',runtime_version:process.version,runtime_available:true,runtime_bundled:path.basename(process.execPath).toLowerCase()==='soundflow-node.exe',worker:'soundflow-media-provider'};
      case 'search':return this.search(args[0],args[1]||'all',args[2]??16);
      case 'get_discover_tracks':return this.search(GENRES[text(args[0],100)||'lofi']||text(args[0],100),'all',20);
      case 'resolve_direct_url':return this.resolve(args[0]);
      case 'get_stream':{ const media=await this.stream(args[0],!!args[1]);if(media.hls)throw new ProviderError('Ця пісня SoundCloud має лише сегментований потік. Завантажте її для відтворення.','hls_download_required');return media.url; }
      case 'get_playback_file':return this.audioCache.get(args[0]);
      case 'get_similar_tracks': {const track=args[0];if(!track||typeof track!=='object')invalid('Некоректна пісня.');const exclude=new Set([track.id,...(Array.isArray(args[2])?args[2]:[])]);return this.getInfiniteRecommendations({tracks:[track],exclude_ids:[...exclude],seed:0},limit(args[1],10,30));}
      case 'get_infinite_recommendations':return this.getInfiniteRecommendations(args[0],args[1]??16);
      case 'match_spotify_track': {const original=args[0];if(!original||canonicalURL(original.url||original.source_url).source!=='spotify')invalid('Потрібна пісня Spotify.');const found=await this.search(`${original.artist==='Spotify'?'':text(original.artist,150)} ${text(original.title,200)}`,args[1]||'all',limit(args[2],6,12));return found.map(t=>({...t,matched_from:{id:original.id,title:original.title,artist:original.artist,url:original.url,source:'spotify'},match_confirmed:false}));}
      case 'import_external_profile':return this.importProfile(...args);
      case 'detect_connected_accounts':return this.detectConnectedAccounts(args[0],args[1]);
      case 'download_track':return this.downloads.enqueue(args[0]);
      case 'get_downloads':return this.downloads.list();
      case 'get_download_job':return this.downloads.get(args[0]);
      case 'cancel_download':return this.downloads.cancel(args[0]);
    }
  }
  async close() { await Promise.allSettled([this.downloads.close(),this.audioCache.close()]); }
}

export function safeFilename(value) {
  let name=text(value,150).replace(/[<>:"/\\|?*%\x00-\x1f]/g,' ').replace(/\s+/g,' ').replace(/[. ]+$/g,'').slice(0,100)||'SoundFlow';
  if(/^(con|prn|aux|nul|com\d|lpt\d)(\.|$)/i.test(name))name=`_${name}`;return name;
}
const ACTIVE=new Set(['queued','downloading','processing','cancelling']);
export class DownloadQueue {
  constructor(provider,root,{maxBytes=MAX_MEDIA_BYTES,cache=false}={}) { this.provider=provider;this.root=root;this.maxBytes=maxBytes;this.cache=cache;this.jobs=new Map();this.running=new Map();this.tasks=new Set();this.closed=false; }
  public(job) { const {controller,...result}=job;return structuredClone(result); }
  list() { return [...this.jobs.values()].reverse().map(j=>this.public(j)); }
  get(id) { const job=this.jobs.get(text(id,100));return job?this.public(job):null; }
  enqueue(track) {
    if(this.closed)throw new ProviderError('Медіарушій завершує роботу.');
    if(!track||typeof track!=='object')invalid('Некоректна пісня.');
    const ref=canonicalURL(track.url||track.source_url);
    if(ref.source==='spotify')throw new ProviderError('Оберіть відповідну пісню YouTube або SoundCloud.','match_required');
    if(track.matched_from && track.match_confirmed!==true)throw new ProviderError('Підтвердьте вибрану відповідність Spotify.','match_required');
    if(track.source && track.source!==ref.source)invalid('Джерело пісні не відповідає посиланню.');
    for(const job of this.jobs.values())if(job.url===ref.url && ACTIVE.has(job.status))return {success:true,job:this.public(job),duplicate:true};
    if([...this.jobs.values()].filter(j=>ACTIVE.has(j.status)).length>=25)throw new ProviderError('Черга завантажень заповнена.');
    const now=Date.now()/1000;const normalized={id:text(track.id,100),title:text(track.title),artist:text(track.artist),thumbnail:text(track.thumbnail,2000),duration:Math.max(0,number(track.duration)),source:ref.source,url:ref.url,...(track.matched_from?{matched_from:structuredClone(track.matched_from)}:{})};
    const job={id:randomUUID(),status:'queued',progress:0,downloaded_bytes:0,total_bytes:0,speed:0,eta:null,created_at:now,updated_at:now,track:normalized,title:normalized.title,source:ref.source,url:ref.url,path:null,file_path:null,filename:null,format:null,error:null,controller:new AbortController()};
    this.jobs.set(job.id,job);while(this.jobs.size>50){const old=[...this.jobs.values()].find(j=>!ACTIVE.has(j.status));if(!old)break;this.jobs.delete(old.id);}
    const result={success:true,job:this.public(job)};queueMicrotask(()=>this.pump());return result;
  }
  cancel(id) {
    const job=this.jobs.get(text(id,100));if(!job)return {success:false,error:'Завантаження не знайдено.'};
    if(!ACTIVE.has(job.status))return {success:job.status==='cancelled',job:this.public(job)};
    job.status=job.status==='queued'?'cancelled':'cancelling';job.updated_at=Date.now()/1000;job.controller.abort();return {success:true,job:this.public(job)};
  }
  pump() {
    if(this.closed)return;
    for(const job of this.jobs.values())if(job.status==='queued' && this.running.size<2) {
      const task=this.run(job).finally(()=>{this.running.delete(job.id);this.tasks.delete(task);this.pump();});
      this.running.set(job.id,task);this.tasks.add(task);
    }
  }
  async run(job) {
    const signal=job.controller.signal;let temporary;let finalPath;const started=Date.now();
    const update=()=>{job.updated_at=Date.now()/1000;job.speed=Math.round(job.downloaded_bytes/Math.max(1,(Date.now()-started)/1000));job.eta=job.total_bytes&&job.speed?Math.max(0,(job.total_bytes-job.downloaded_bytes)/job.speed):null;};
    try {
      job.status='downloading';await fs.mkdir(this.root,{recursive:true});
      const root=await fs.realpath(this.root);const media=await this.provider.stream(job.track,false,signal);signal.throwIfAborted();
      const basename=`${this.cache?'sf-audio':safeFilename(`${job.track.artist} - ${job.track.title}`)}-${job.id}.${media.ext}`;
      finalPath=path.join(root,basename);temporary=`${finalPath}.part`;job.format=media.ext;
      const file=await fs.open(temporary,'wx');
      try {
        const writeResponse=async(url,index,total,range)=>{
          const headers={...(job.source==='soundcloud'?SC_HEADERS:{}),...(range?{range:`bytes=${range.start}-${range.end}`}:{})};
          const response=await providerFetch(url,{signal,headers});
          const bytes=number(response.headers.get('content-length'));
          if(total===1)job.total_bytes=bytes||media.size||0;
          if(range && response.status!==206)throw new ProviderError('YouTube не надав запитану частину аудіофайлу.');
          if(bytes>this.maxBytes)throw new ProviderError('Пісня завелика для завантаження.');
          let received=0;
          for await(const chunk of response.body) {
            signal.throwIfAborted();if(job.downloaded_bytes+chunk.length>this.maxBytes)throw new ProviderError('Пісня завелика для завантаження.');
            let written=0;while(written<chunk.length){const result=await file.write(chunk,written);if(!result.bytesWritten)throw new ProviderError('Не вдалося записати аудіофайл.');written+=result.bytesWritten;}
            job.downloaded_bytes+=chunk.length;received+=chunk.length;
            job.progress=job.total_bytes?Math.min(99,job.downloaded_bytes/job.total_bytes*100):Math.min(99,(index+.5)/total*100);update();
          }
          if(range && received!==range.end-range.start+1)throw new ProviderError('YouTube повернув неповний аудіофрагмент.');
          if(total>1)job.progress=Math.min(99,(index+1)/total*100);
        };
        if(media.hls) {
          const playlist=await getText(media.url,{signal,headers:SC_HEADERS});
          if(/#EXT-X-KEY:.*METHOD=(?!NONE)/.test(playlist))throw new ProviderError('Зашифровані потоки не підтримуються.');
          const urls=[];const init=playlist.match(/#EXT-X-MAP:URI="([^"]+)"/)?.[1];if(init)urls.push(String(networkURL(new URL(init,media.url))));
          for(const line of playlist.split(/\r?\n/))if(line.trim() && !line.startsWith('#'))urls.push(String(networkURL(new URL(line.trim(),media.url))));
          if(!urls.length||urls.length>4000 || /#EXT-X-STREAM-INF|#EXT-X-BYTERANGE/.test(playlist))throw new ProviderError('Непідтримуваний сегментований потік.');
          for(let i=0;i<urls.length;i++)await writeResponse(urls[i],i,urls.length);
        } else if(job.source==='youtube' && media.size>0) {
          if(media.size>this.maxBytes)throw new ProviderError('Пісня завелика для завантаження.');
          job.total_bytes=media.size;const chunkSize=256*1024;const total=Math.ceil(media.size/chunkSize);
          for(let i=0;i<total;i++)await writeResponse(media.url,i,total,{start:i*chunkSize,end:Math.min(media.size-1,(i+1)*chunkSize-1)});
          if(job.downloaded_bytes!==media.size)throw new ProviderError('YouTube повернув неповний аудіофайл.');
        } else await writeResponse(media.url,0,1);
        signal.throwIfAborted();if(job.downloaded_bytes<128)throw new ProviderError('Медіасервіс повернув порожній аудіофайл.');
        job.status='processing';update();await file.sync();
      } finally { await file.close(); }
      signal.throwIfAborted();
      if((job.format==='m4a'||job.format==='mp4') && job.track?.duration > 0) {
        try {
          const headerBuf = Buffer.alloc(32768);
          const handle = await fs.open(temporary, 'r+');
          try {
            const { bytesRead } = await handle.read(headerBuf, 0, 32768, 0);
            if (bytesRead > 32) {
              patchMp4Duration(headerBuf.subarray(0, bytesRead), job.track.duration);
              await handle.write(headerBuf, 0, bytesRead, 0);
            }
          } finally { await handle.close(); }
        } catch {}
      }
      await fs.rename(temporary,finalPath);temporary=null;
      job.status='completed';job.progress=100;job.path=job.file_path=finalPath;job.filename=path.basename(finalPath);job.total_bytes=job.downloaded_bytes;job.eta=0;update();
    } catch(error) {
      job.status=signal.aborted?'cancelled':'failed';job.error=signal.aborted?null:cleanError(error);update();
      if(temporary)await fs.unlink(temporary).catch(()=>{});
    }
  }
  async close() { this.closed=true;for(const job of this.jobs.values())if(ACTIVE.has(job.status))this.cancel(job.id);await Promise.allSettled([...this.tasks]); }
}

class AudioCache {
  constructor(provider,root) { this.root=root;this.queue=new DownloadQueue(provider,root,{maxBytes:256*1024*1024,cache:true});this.paths=new Map();this.pending=new Map();this.serial=Promise.resolve(); }
  get(track) {
    if(track?.matched_from && track.match_confirmed!==true)throw new ProviderError('Підтвердьте вибрану відповідність Spotify.','match_required');
    const ref=canonicalURL(track?.url||track?.source_url);
    if(this.pending.has(ref.url))return this.pending.get(ref.url);
    const promise=this.serial.then(()=>this.load(track,ref));this.serial=promise.catch(()=>{});this.pending.set(ref.url,promise);
    promise.finally(()=>this.pending.delete(ref.url)).catch(()=>{});return promise;
  }
  async prune(reserve=0) {
    await fs.mkdir(this.root,{recursive:true});const names=await fs.readdir(this.root,{withFileTypes:true});const files=[];
    for(const entry of names) {
      if(!entry.isFile()||!/^sf-audio-[a-f0-9-]{36}\.(m4a|mp3)(\.part)?$/.test(entry.name))continue;
      const file=path.join(this.root,entry.name);const stat=await fs.stat(file);
      if(entry.name.endsWith('.part')){await fs.unlink(file).catch(()=>{});continue;}
      files.push({file,size:stat.size,time:stat.mtimeMs});
    }
    files.sort((a,b)=>a.time-b.time);let size=files.reduce((sum,file)=>sum+file.size,0);let count=files.length;
    for(const file of files)if(count>=(reserve?8:9)||size+reserve>512*1024*1024) {
      try {await fs.unlink(file.file);size-=file.size;count--;for(const [url,cached] of this.paths)if(cached===file.file)this.paths.delete(url);}catch{}
    }
    if(size+reserve>512*1024*1024||count>=(reserve?8:9))throw new ProviderError('Не вдалося підготувати аудіо. Спробуйте після завершення поточної пісні.');
  }
  async load(track,ref) {
    const cached=this.paths.get(ref.url);
    if(cached)try {await fs.access(cached);const now=new Date();await fs.utimes(cached,now,now);return cached;}catch{this.paths.delete(ref.url);}
    await this.prune(256*1024*1024);
    const result=this.queue.enqueue(track);const id=result.job.id;
    const deadline=setTimeout(()=>this.queue.cancel(id),45000);
    try {
      while(true) {
        const job=this.queue.get(id);
        if(job.status==='completed'){this.paths.set(ref.url,job.file_path);await this.prune();return job.file_path;}
        if(job.status==='failed'||job.status==='cancelled')throw new ProviderError(job.error||'Підготовку аудіо скасовано.');
        await new Promise(resolve=>setTimeout(resolve,50));
      }
    } finally {clearTimeout(deadline);}
  }
  async close() { await this.queue.close(); }
}

export async function handleRequest(provider,frame) {
  let id=null;
  try {
    if(Buffer.byteLength(frame)>MAX_REQUEST_BYTES)invalid('Provider request exceeds 4 MiB.');
    const line=typeof frame==='string'?frame:new TextDecoder('utf-8',{fatal:true}).decode(frame);const request=JSON.parse(line);
    if(!request||typeof request!=='object'||Array.isArray(request))invalid('Provider request must be an object.');
    if(!Number.isSafeInteger(request.id)||request.id<0)invalid('Provider request id must be a nonnegative safe integer.');id=request.id;
    if(typeof request.method!=='string'||!Array.isArray(request.args??[]))invalid('Provider request requires method:string and args:array.');
    const response={id,result:await provider.dispatch(request.method,request.args??[])};
    if(Buffer.byteLength(JSON.stringify(response))>MAX_RESPONSE_BYTES)throw new ProviderError('Provider response exceeds 16 MiB.');return response;
  }catch(error){return {id,result:null,error:{code:error instanceof ProviderError?error.code:((error instanceof SyntaxError || error instanceof TypeError)?'invalid_request':'provider_error'),message:cleanError(error)}};}
}
export async function runWorker() {
  console.log=console.info=console.debug=(...args)=>console.error(...args);
  const flags=process.argv.slice(2);const argument=name=>{const index=flags.indexOf(name);return index<0?null:flags[index+1];};
  const dataDir=path.resolve(argument('--data-dir')||path.join(os.homedir(),'.soundflow','provider'));
  const provider=new MediaProvider(argument('--download-dir')||path.join(os.homedir(),'Music','SoundFlow'),argument('--playback-dir')||path.join(path.dirname(dataDir),'provider-audio'));await fs.mkdir(dataDir,{recursive:true});
  // EOF must cancel transfers even while an awaited playback preparation is running.
  process.stdin.once('end',()=>{provider.close().catch(error=>console.error(cleanError(error)));});
  let pending=Buffer.alloc(0);let oversized=false;let requests=Promise.resolve();
  const emit=response=>new Promise((resolve,reject)=>process.stdout.write(`${JSON.stringify(response)}\n`,error=>error?reject(error):resolve()));
  try {
    for await(const chunk of process.stdin) {
      let start=0;
      for(let i=0;i<chunk.length;i++)if(chunk[i]===10) {
        const part=chunk.subarray(start,i);start=i+1;
        if(!oversized && pending.length+part.length<=MAX_REQUEST_BYTES)pending=Buffer.concat([pending,part]);else oversized=true;
        const frame=pending;const tooLarge=oversized;pending=Buffer.alloc(0);oversized=false;
        // Backpressure: bounded stdin frames and exactly one request in flight.
        requests=requests.then(async()=>{if(tooLarge)await emit({id:null,result:null,error:{code:'invalid_request',message:'Provider request exceeds 4 MiB.'}});else if(frame.length)await emit(await handleRequest(provider,frame));});
        await requests;
      }
      const tail=chunk.subarray(start);if(!oversized && pending.length+tail.length<=MAX_REQUEST_BYTES)pending=Buffer.concat([pending,tail]);else {pending=Buffer.alloc(0);oversized=true;}
    }
    await requests;
    if(pending.length || oversized)await emit({id:null,result:null,error:{code:'invalid_request',message:'Truncated provider request.'}});
  } finally { await provider.close(); }
}
const isEntry=typeof __SOUNDFLOW_BUNDLED__!=='undefined' || (process.argv[1] && path.resolve(process.argv[1])===fileURLToPath(import.meta.url));
if(isEntry)runWorker().catch(error=>{console.error(cleanError(error));process.exitCode=1;});
