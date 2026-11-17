import test from 'node:test';
import assert from 'node:assert/strict';
import { feedQueries, diverseTracks } from './feed-selection.mjs';
test('refresh rotates discovery queries and seed artists',()=>{
 const seeds=[{artist:'A'},{artist:'B'},{artist:'C'}];
 assert.notEqual(feedQueries(seeds,0).youtube,feedQueries(seeds,1).youtube);
 assert.notEqual(feedQueries(seeds,0).discovery,feedQueries(seeds,1).discovery);
 assert.ok(feedQueries([],0).youtube);
});
test('deduplicates by source and ID, prefers fresh tracks and spreads artists',()=>{
 const t=(id,artist,source='youtube')=>({id,artist,source});
 const result=diverseTracks([t('old','A'),t('1','A'),t('1','A'),t('2','A'),t('3','A'),t('4','B'),t('1','C','soundcloud')],[t('old','A')]);
 assert.equal(result.length,6);
 assert.deepEqual(result.slice(0,4).map(t=>t.id),['1','2','4','1']);
 assert.equal(result.at(-1).id,'old');
});
