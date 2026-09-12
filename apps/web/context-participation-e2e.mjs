import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Client, message } from '../../target/browser/sdk/typescript/resource.js';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
const left = { retiring: [] }, right = { retiring: [] };
let a = { retiring: [] }, b = { retiring: [] }, fault, lostAttach, calls = 0;
globalThis.fetch = async (url, options) => {
  calls++;
  const fields = JSON.parse(options.body), name = fields[5];
  if (fields[1] === 'call' && fields[0] === left.token) {
    assert.notEqual(name, 'kernel.session.close', 'shared context must not be closed');
    if (name === 'editing.context.update') assert.equal(fields[4], a.participation);
  }
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  const selected = (fault === 'attach' && name === 'kernel.session.attach')
    || (fault === 'seek' && name === 'kernel.session.seek')
    || (fault === 'detach' && name === 'kernel.session.detach')
    || (fault === 'checkpoint' && fields[1] === 'release' && fields[2] === a.checkpointRequest);
  if (selected) {
    const line = await response.text();
    if (fault === 'attach') lostAttach = message(line).resources[0];
    fault = undefined; throw new Error('lost context delivery');
  }
  return response;
};
const first = new Editing(left, () => {}, `${origin}/rpc`);
const second = new Editing(right, () => {}, `${origin}/rpc`);
await first.join(); await second.join();
const before = left.source.revision;
const ca = new Client(left.token, `${origin}/rpc`), cb = new Client(right.token, `${origin}/rpc`);
let own = new Focus(ca, a, () => {});
const peer = new Focus(cb, b, () => {});
function reload() {
  const count = calls;
  a = JSON.parse(JSON.stringify(a)); own = new Focus(ca, a, () => {});
  assert.equal(calls, count, 'restoring the journal cannot replay work');
}
fault = 'attach';
await assert.rejects(own.join(), /Outcome unavailable/);
reload(); await own.recover();
assert.equal(a.participation, lostAttach);
assert.equal(a.ownership, 'shared');
await peer.join(); await own.follow(); await peer.follow();
await own.set({ known: [left.source.buffer], active: left.source.buffer });
await peer.follow();
assert.equal(b.current.active, right.source.buffer);
const previous = a.current.snapshot;
fault = 'checkpoint';
await assert.rejects(own.remember(), /lost context delivery/);
const checkpoint = a.checkpoint.snapshot;
reload(); await own.recover();
assert.equal(a.checkpoint.snapshot, checkpoint);
assert.equal(a.current.snapshot, previous);
assert.ok(!a.retiring.includes(checkpoint));
await peer.set({ known: [right.source.buffer] });
await own.refresh(); assert.equal(a.current.active, undefined);
fault = 'seek';
await assert.rejects(own.seek(), /Outcome unavailable/);
reload(); await own.recover();
assert.equal(a.historical, true);
assert.equal(a.current.active, left.source.buffer);
await peer.refresh(); assert.equal(b.current.active, undefined);
await peer.set({ known: [right.source.buffer], active: right.source.buffer });
reload(); await own.refresh();
assert.equal(a.historical, true);
await assert.rejects(own.set({ known: [left.source.buffer] }), /Follow live/);
await first.observe(); assert.equal(left.source.revision, before);
await second.replace(2n, 8n, 'edited'); await first.follow();
assert.deepEqual(left.window.units, [...'# edited'].map(scalar => ({ scalar })));
assert.equal(a.historical, true);
await own.follow(); assert.equal(a.historical, false);
assert.equal(a.current.active, left.source.buffer);
await second.undo(); await first.follow();
assert.deepEqual(left.window.units, [...'# shared'].map(scalar => ({ scalar })));
fault = 'detach';
await assert.rejects(own.close(), /Outcome unavailable/);
reload(); await own.recover();
assert.equal(a.current, undefined); assert.equal(a.participation, undefined);
assert.equal(a.checkpoint, undefined); assert.equal(a.closing, undefined);
assert.deepEqual(a.retiring, []); assert.deepEqual(a.retiringWatches, []);
await peer.refresh(); assert.equal(b.current.active, right.source.buffer);
await peer.close(); await first.close(); await second.close();
for (const client of [ca, cb]) {
  const deadline = performance.now() + 5000;
  for (;;) {
    const result = await client.wire([client.token, 'disconnect']);
    if (result === 'ok') break;
    assert.equal(result, 'error\tclient cleanup: Resource error 8');
    assert.ok(performance.now() < deadline); await setTimeout(10);
  }
}
console.log('Shared browser participation, checkpoint/seek recovery, independent views, content/Undo and detach passed');
