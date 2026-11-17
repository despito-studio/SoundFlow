// Pure feed selection helpers: refresh rotates queries without shuffling playback.
export function feedQueries(seeds, round) {
  const artists = [...new Set(seeds.map(t => String(t.artist || '').trim()).filter(Boolean))].slice(0, 12);
  const discovery = ['new music official audio', 'indie music official audio', 'electronic music official audio', 'alternative music official audio', 'українська музика новинки'];
  return {
    artists,
    youtube: artists.length ? artists[round % artists.length] : discovery[round % discovery.length],
    soundcloud: artists.length ? artists[(round + 1) % artists.length] : discovery[(round + 1) % discovery.length],
    discovery: discovery[round % discovery.length],
  };
}

export function diverseTracks(tracks, previous = []) {
  const seen = new Set();
  const old = new Set(previous.map(t => `${t.source}:${t.id}`));
  const unique = tracks.filter(t => {
    const key = `${t.source}:${t.id}`;
    if (!t.id || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  const ordered = [...unique.filter(t => !old.has(`${t.source}:${t.id}`)), ...unique.filter(t => old.has(`${t.source}:${t.id}`))];
  const counts = new Map(), first = [], rest = [];
  for (const track of ordered) {
    const artist = String(track.artist || track.uploader || track.id).trim().toLowerCase();
    const count = counts.get(artist) || 0;
    counts.set(artist, count + 1);
    (count < 2 ? first : rest).push(track);
  }
  return [...first, ...rest];
}
