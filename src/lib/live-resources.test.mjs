import assert from 'node:assert/strict';
import { test } from 'node:test';
import { LiveResourceStore, panePercent } from './live-resources.ts';

const pod = (name, uid = name, rv = 'v1') => ({ name, namespace: 'team', uid, resourceVersion: rv, status: 'Running' });
test('add, status change and delete are applied without a list request', () => {
  const store = new LiveResourceStore([pod('a')]);
  store.apply([{action:'upsert',object:pod('b')},{action:'upsert',object:{...pod('a','a','v2'),status:'Pending'}}]);
  assert.equal(store.get(pod('a')).status, 'Pending');
  store.apply([{action:'delete',object:pod('b')}]);
  assert.deepEqual(store.rows().map(x=>x.name), ['a']);
});
test('old deletion cannot delete a same-name replacement', () => {
  const store = new LiveResourceStore([pod('a','old')]);
  store.apply([{action:'upsert',object:pod('a','new')},{action:'delete',object:pod('a','old')}]);
  assert.equal(store.get(pod('a')).uid,'new');
});
test('resource versions are opaque; duplicate updates preserve row identity and metrics', () => {
  const original={...pod('a','a','z'),cpuUsage:'0.2 cores'};
  const store=new LiveResourceStore([original]);
  assert.equal(store.apply([{action:'upsert',object:{...original}}]), false);
  assert.equal(store.get(original),original);
  store.apply([{action:'upsert',object:pod('a','a','a')}]);
  assert.equal(store.get(original).resourceVersion,'a');
  assert.equal(store.get(original).cpuUsage,'0.2 cores');
});
test('relist is atomic and clears missing objects only at completion', () => {
  const store=new LiveResourceStore([pod('old')]);
  store.beginReset();store.stage([pod('new')]);
  assert.deepEqual(store.rows().map(x=>x.name),['old']);
  store.abortReset();assert.deepEqual(store.rows().map(x=>x.name),['old']);
  store.beginReset();store.stage([pod('new')]);store.finishReset();
  assert.deepEqual(store.rows().map(x=>x.name),['new']);
  store.beginReset();store.finishReset();assert.equal(store.rows().length,0);
});
test('large bursts converge to the same result as individual updates', () => {
  const rows=Array.from({length:20_000},(_,i)=>pod(`pod-${String(i).padStart(6,'0')}`));
  const store=new LiveResourceStore(rows);
  const changes=rows.slice(0,5000).map(object=>({action:'delete',object}));
  changes.push(...rows.slice(5000,10000).map(object=>({action:'upsert',object:{...object,resourceVersion:'next',status:'Pending'}})));
  changes.push(...Array.from({length:2000},(_,i)=>({action:'upsert',object:pod(`new-${i}`)})));
  store.apply(changes);
  assert.equal(store.rows().length,17_000);
  assert.equal(store.rows().filter(x=>x.status==='Pending').length,5000);
  assert.equal(store.get(rows[15_000]),rows[15_000]);
});
test('metrics are independent of object versions and namespace-safe',()=>{
  const store=new LiveResourceStore([pod('a'),{...pod('a'),namespace:'other'}]);
  store.applyMetrics([{name:'a',namespace:'team',cpuUsage:'0.3 cores'}]);
  assert.equal(store.get(pod('a')).cpuUsage,'0.3 cores');
  assert.equal(store.get({...pod('a'),namespace:'other'}).cpuUsage,undefined);
});
test('splitter remains in usable limits',()=>{
  assert.equal(panePercent(-1),25);assert.equal(panePercent(100),70);assert.equal(panePercent(45),45);
});
