import fs from 'node:fs';

const origPath = 'C:\\SoundFlow\\media-provider\\dist\\worker.cjs';
const targetDir = 'c:\\SounFlowproject\\media-provider\\dist';
const targetPath = targetDir + '\\worker.cjs';

if (!fs.existsSync(targetDir)) {
  fs.mkdirSync(targetDir, { recursive: true });
}

let content = fs.readFileSync(origPath, 'utf8');

// 1. Add methods to arity map
const oldArity = 'let n={ping:0,search:3,get_discover_tracks:1,resolve_direct_url:1,get_stream:2,get_playback_file:1,get_similar_tracks:3,match_spotify_track:3,import_external_profile:2,download_track:1,get_downloads:0,get_download_job:1,cancel_download:1};';
const newArity = 'let n={ping:0,search:3,get_discover_tracks:1,resolve_direct_url:1,get_stream:2,get_playback_file:1,get_similar_tracks:3,match_spotify_track:3,import_external_profile:2,download_track:1,get_downloads:0,get_download_job:1,cancel_download:1,detect_connected_accounts:2,get_infinite_recommendations:2};';

if (!content.includes(oldArity)) {
  console.error("Arity string not found");
  process.exit(1);
}
content = content.replace(oldArity, newArity);

// 2. Add cases to switch
const oldSwitch = 'case"ping":return{';
const newSwitch = 'case"detect_connected_accounts":return this.detectConnectedAccounts(i[0],i[1]);case"ping":return{';

if (!content.includes(oldSwitch)) {
  console.error("Switch case not found");
  process.exit(1);
}
content = content.replace(oldSwitch, newSwitch);

// 3. Append detectConnectedAccounts implementation to oh.prototype
const extraMethods = `
oh.prototype.detectConnectedAccounts = async function(email = '', name = '') {
  const result = {
    youtube: { found: false, name: '', identifier: '' },
    soundcloud: { found: false, name: '', identifier: '' },
    spotify: { found: false, name: '', identifier: '' }
  };
  const cleanEmail = (typeof email === 'string' ? email.trim() : '').toLowerCase();
  const username = cleanEmail.includes('@') ? cleanEmail.split('@')[0] : cleanEmail;
  const cleanHandle = username.replace(/[^a-zA-Z0-9._-]/g, '');

  // 1. YouTube check
  if (cleanHandle) {
    try {
      const yt = await this.youtube();
      const endpoint = await yt.actions.execute('/navigation/resolve_url', { url: 'https://www.youtube.com/@' + cleanHandle });
      if (endpoint && endpoint.payload && endpoint.payload.browseId) {
        const channel = await yt.getChannel(endpoint.payload.browseId);
        const channelTitle = channel?.metadata?.title || name || ('@' + cleanHandle);
        result.youtube = {
          found: true,
          name: channelTitle,
          identifier: '@' + cleanHandle
        };
      }
    } catch {}
  }

  // 2. SoundCloud check
  if (username) {
    try {
      const sc = await this.scAPI('resolve', { url: 'https://soundcloud.com/' + username });
      if (sc && sc.kind === 'user' && sc.username) {
        result.soundcloud = {
          found: true,
          name: sc.username,
          identifier: sc.permalink_url || ('https://soundcloud.com/' + username)
        };
      }
    } catch {}

    if (!result.soundcloud.found && cleanHandle && cleanHandle !== username) {
      try {
        const sc2 = await this.scAPI('resolve', { url: 'https://soundcloud.com/' + cleanHandle });
        if (sc2 && sc2.kind === 'user' && sc2.username) {
          result.soundcloud = {
            found: true,
            name: sc2.username,
            identifier: sc2.permalink_url || ('https://soundcloud.com/' + cleanHandle)
          };
        }
      } catch {}
    }

    if (!result.soundcloud.found) {
      try {
        const scSearch = await this.scAPI('search/users', { q: username, limit: 5 });
        const matches = scSearch?.collection || [];
        for (const u of matches) {
          if (u?.username && (u.permalink?.toLowerCase() === username || u.username.toLowerCase() === username)) {
            result.soundcloud = {
              found: true,
              name: u.username,
              identifier: u.permalink_url || ('https://soundcloud.com/' + u.permalink)
            };
            break;
          }
        }
      } catch {}
    }
  }

  // 3. Spotify check
  if (username) {
    const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36';
    try {
      const profileUrl = 'https://open.spotify.com/user/' + username;
      const res = await fetch(profileUrl, {
        headers: { 'user-agent': UA },
        signal: AbortSignal.timeout(5000)
      });
      if (res.ok) {
        const html = await res.text();
        if (!html.includes('Page not found') && !html.includes("Couldn't find that user")) {
          let spName = name || username;
          const match = html.match(/<title>([^<]+)<\\/title>/) || html.match(/<meta property="og:title" content="([^"]+)"/);
          if (match && !match[1].toLowerCase().includes('spotify')) {
            spName = match[1].replace(/ \\| Spotify$/, '').trim();
          }
          result.spotify = {
            found: true,
            name: spName,
            identifier: profileUrl
          };
        }
      }
    } catch {}

    if (!result.spotify.found) {
      try {
        const searchUrl = 'https://open.spotify.com/search/' + encodeURIComponent(username);
        const searchRes = await fetch(searchUrl, {
          headers: { 'user-agent': UA },
          signal: AbortSignal.timeout(5000)
        });
        if (searchRes.ok) {
          const html = await searchRes.text();
          const playlistMatch = html.match(/href="\\/playlist\\/([A-Za-z0-9]{22})"/);
          if (playlistMatch) {
            result.spotify = {
              found: true,
              name: (name || username) + ' (Spotify)',
              identifier: 'https://open.spotify.com/playlist/' + playlistMatch[1]
            };
          }
        }
      } catch {}
    }
  }

  return result;
};
`;

content = content + '\n' + extraMethods;

fs.writeFileSync(targetPath, content, 'utf8');
fs.writeFileSync(origPath, content, 'utf8');
console.log('Successfully patched worker.cjs in both project and C:\\SoundFlow!');
