// Run against a bridge serving configured client-owned source and Text modules.
import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Client, unhex } from '../../target/browser/sdk/typescript/resource.js';
import * as text from '../../target/browser/modules/text/api/typescript.js';

const origin = new URL(process.argv[2]).origin;
const nativeFetch = globalThis.fetch;
let loseEdit = false;
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  const reply = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (loseEdit && fields[1] === 'call' && fields[5] === 'text.replace') {
    loseEdit = false;
    await reply.text();
    throw new Error('injected lost accepted edit reply');
  }
  return reply;
};
const state = { retiring: [] };
let saved;
const editor = new Editing(state, () => { saved = structuredClone(state); }, `${origin}/rpc`);
const original = unhex('2320ed959ceab88020f09fa6800d0aff007461696c');
const scalar = value => [...value].map(scalar => ({ scalar }));
const before = [...scalar('# 한글 🦀\r\n'), { opaque: 255 }, ...scalar('\0tail')];
const after = [...scalar('# é漢 🦀\r\n'), { opaque: 255 }, ...scalar('\0tail')];
await editor.create(original);
assert.deepEqual(state.window.units, before);
loseEdit = true;
await assert.rejects(editor.replace(2n, 4n, 'é漢'), /Outcome unavailable/);
assert.ok(saved.receipt);
const resumed = new Editing(saved, () => {}, `${origin}/rpc`);
await resumed.recover();
await resumed.observe();
assert.equal(saved.source.revision, '1');
assert.deepEqual(saved.window.units, after);
await resumed.undo();
assert.deepEqual(saved.window.units, before);

// A separate caller of this retained scope changes state behind the old view.
const client = new Client(saved.token, `${origin}/rpc`);
const receipt = await client.reserve(text.replace(saved.observed, 2n, 4n, 'peer'));
assert.match(await client.send(receipt), /^ok\t/);
await client.release(receipt.id);
await assert.rejects(resumed.replace(2n, 4n, 'stale'), /error\tfailed\t7\t/);
await resumed.observe();
assert.deepEqual(saved.window.units,
  [...scalar('# peer 🦀\r\n'), { opaque: 255 }, ...scalar('\0tail')]);
await resumed.undo();
assert.deepEqual(saved.window.units, before);
await resumed.close();
assert.equal(saved.source, undefined);
assert.equal(saved.access, undefined);
assert.equal(saved.observed, undefined);
const deadline = performance.now() + 5000;
for (;;) {
  const response = await client.wire([client.token, 'disconnect']);
  if (response === 'ok') break;
  assert.equal(response, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline, 'Client cleanup did not settle');
  await setTimeout(10);
}
console.log('Browser transport editing, lost reply, stale rejection, Undo and cleanup passed');
