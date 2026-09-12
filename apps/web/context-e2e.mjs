import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Client } from '../../target/browser/sdk/typescript/resource.js';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
let loseFocus = false;
globalThis.fetch = async (url, options) => {
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  const fields = JSON.parse(options.body);
  if (loseFocus && fields[1] === 'call' && fields[5] === 'editing.context.update') {
    loseFocus = false; await response.text(); throw new Error('lost focus reply');
  }
  return response;
};
const left = { retiring: [] }, right = { retiring: [] };
const first = new Editing(left, () => {}, `${origin}/rpc`);
const second = new Editing(right, () => {}, `${origin}/rpc`);
await first.join(); await second.join();
assert.ok(left.token !== right.token, 'Clients must have independent identities');
const beforeFocus = left.source.revision;
const a = { retiring: [] }, b = { retiring: [] };
const ca = new Client(left.token, `${origin}/rpc`), cb = new Client(right.token, `${origin}/rpc`);
const own = new Focus(ca, a, () => {}), peer = new Focus(cb, b, () => {});
await own.create({ known: [] });
assert.equal(a.current.active, undefined);
await own.set({ known: [left.source.buffer], active: left.source.buffer });
await peer.create({ known: [right.source.buffer], active: right.source.buffer });
loseFocus = true;
const changing = own.set({ known: a.current.known });
await assert.rejects(own.refresh(), /pending/);
await assert.rejects(changing, /Outcome unavailable/);
await own.recover();
assert.equal(a.current.active, undefined);
await peer.refresh();
assert.equal(b.current.active, right.source.buffer);
await first.observe();
assert.equal(left.source.revision, beforeFocus);
await second.replace(2n, 8n, 'edited');
await first.follow();
assert.deepEqual(left.window.units, [...'# edited'].map(scalar => ({ scalar })));
await own.close();
await peer.refresh();
assert.equal(b.current.active, right.source.buffer);
await second.undo();
await first.follow();
assert.deepEqual(left.window.units, [...'# shared'].map(scalar => ({ scalar })));
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
console.log('Independent retained focus, lost-update recovery, shared content/Undo and context cleanup passed');
