import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Client, hex, u64 } from '../../target/browser/sdk/typescript/resource.js';

const origin = new URL(process.argv[2]).origin;
const nativeFetch = globalThis.fetch;
globalThis.fetch = (url, options) => nativeFetch(url, { ...options,
  headers: { ...options.headers, Origin: origin } });
const left = { retiring: [] }, right = { retiring: [] };
const first = new Editing(left, () => {}, `${origin}/rpc`);
const second = new Editing(right, () => {}, `${origin}/rpc`);
const scalar = value => [...value].map(scalar => ({ scalar }));
await first.join();
await second.join();
assert.notEqual(left.token, right.token);
assert.deepEqual(left.window.units, scalar('# shared'));
assert.deepEqual(right.window.units, scalar('# shared'));
await first.replace(2n, 8n, 'first');
assert.deepEqual(left.window.units, scalar('# first'));
assert.deepEqual(right.window.units, scalar('# shared'));
await assert.rejects(second.replace(2n, 8n, 'stale'), /error\tfailed\t7\t/);
await second.follow();
assert.deepEqual(right.window.units, scalar('# first'));
const client = new Client(right.token, `${origin}/rpc`);
const request = await client.reserve(['write', right.source.buffer,
  'kernel.buffer.close', '1', hex(u64(BigInt(right.source.revision))) + '01', right.source.buffer]);
assert.match(await client.send(request), /^error\tfailed\t1\t/);
await client.release(request.id);
await first.close();
await second.replace(2n, 7n, 'second');
assert.deepEqual(right.window.units, scalar('# second'));
await first.join();
assert.deepEqual(left.window.units, scalar('# second'));
await first.undo();
assert.deepEqual(left.window.units, scalar('# first'));
await second.follow();
assert.deepEqual(right.window.units, scalar('# first'));
await first.close();
await second.close();
for (const state of [left, right]) {
  const owner = new Client(state.token, `${origin}/rpc`);
  const deadline = performance.now() + 5000;
  for (;;) {
    const response = await owner.wire([owner.token, 'disconnect']);
    if (response === 'ok') break;
    assert.equal(response, 'error\tclient cleanup: Resource error 8');
    assert.ok(performance.now() < deadline, 'Client teardown did not settle');
    await setTimeout(10);
  }
}
console.log('Independent Clients share edits/Undo, retain separate views, deny shared close and detach independently');
