import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Client } from '../../target/browser/sdk/typescript/resource.js';
import { Editing } from '../../target/browser/apps/web/editing.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
globalThis.fetch = (url, options) => nativeFetch(url, { ...options,
  headers: { ...options.headers, Origin: origin } });
let state = { retiring: [] }, editing = new Editing(state, () => {}, `${origin}/rpc`);
await assert.rejects(editing.openFile('fail.md'), /Unknown outcome/);
await assert.rejects(editing.recover(), /Unknown outcome/);
state = JSON.parse(JSON.stringify(state));
editing = new Editing(state, () => {}, `${origin}/rpc`);
await assert.rejects(editing.recover(), /Opening failed \(code 3\); cleanup completed/);
assert.equal(state.source, undefined); assert.equal(state.resuming, undefined);
assert.equal(state.receipt, undefined); assert.equal(state.openingCleanup, undefined);
assert.equal(state.openingWork, undefined); assert.equal(state.openingFailed, undefined);
assert.deepEqual(state.retiring, []);
const client = new Client(state.token, `${origin}/rpc`);
const lookup = await client.reserve(['resolve', 'buffers']);
assert.ok((await client.send(lookup)).startsWith('error\trejected\t2\t'));
await client.release(lookup.id);
await editing.openFile('draft.md');
assert.ok(state.source && state.window);
assert.ok(state.source.backing);
await editing.close();
const deadline = performance.now() + 5000;
for (;;) {
  const line = await client.wire([client.token, 'disconnect']);
  if (line === 'ok') break;
  assert.equal(line, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline); await setTimeout(10);
}
console.log(JSON.stringify({ terminalFailure: true, noBufferInstalled: true,
  cleanup: true, laterOpening: true }));
