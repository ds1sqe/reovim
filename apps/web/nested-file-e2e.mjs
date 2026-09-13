import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { setTimeout } from 'node:timers/promises';
import { Client, message, outcome, resource } from '../../target/browser/sdk/typescript/resource.js';
import { Editing } from '../../target/browser/apps/web/editing.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
let state = { retiring: [] }, loseResolve = true, loseRelease = true;
let originalReceipt, openings = 0, resumes = 0;
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  if (fields[1] === 'call' && fields[5] === 'command.open') openings++;
  if (fields[1] === 'call' && fields[5] === 'command.open.resume') resumes++;
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (originalReceipt && fields[1] === 'call' && fields[3] === 'resolve' && loseResolve) {
    loseResolve = false; await response.text(); throw new Error('lost recovery lookup');
  }
  if (fields[1] === 'release' && fields[2] === originalReceipt && loseRelease) {
    loseRelease = false; await response.text(); throw new Error('lost original receipt retirement');
  }
  return response;
};
let editing = new Editing(state, () => {}, `${origin}/rpc`);
await assert.rejects(editing.openFile('draft.md'), /Unknown outcome/);
const unknown = outcome(state.receipt.response);
assert.equal(unknown.kind, 'unknown');
const client = new Client(state.token, `${origin}/rpc`);
async function rpc(fields) {
  const receipt = await client.reserve(fields), line = await client.send(receipt);
  assert.ok(line.startsWith('ok'), line); await client.release(receipt.id); return line;
}
let work = resource((await rpc(['resolve', unknown.recovery])).split('\t')[1]);
for (let depth = 0;; depth++) {
  assert.ok(depth < 16);
  const reply = message(await rpc(['read', work, 'kernel.request.result', '1']));
  if (reply.data[0] === 0) break;
  assert.equal(reply.data[0], 2);
  const locator = new TextDecoder().decode(reply.data.slice(9));
  work = resource((await rpc(['resolve', locator])).split('\t')[1]);
}
await writeFile(process.argv[3], '# replaced after capture\n');
originalReceipt = state.receipt.id;

function reload() {
  state = JSON.parse(JSON.stringify(state));
  editing = new Editing(state, () => {}, `${origin}/rpc`);
}
await assert.rejects(editing.recover(), /Outcome unavailable/);
assert.equal(state.resuming.original.id, originalReceipt); reload();
await assert.rejects(editing.recover(), /lost original receipt retirement/);
assert.ok(state.resuming.work); reload();
await assert.rejects(editing.recover(), /Unknown outcome/);
assert.equal(state.receipt.fields[2], 'command.open.resume'); reload();
await editing.recover();
assert.equal(state.receipt, undefined); assert.equal(state.resuming, undefined);
assert.equal(openings, 1); assert.equal(resumes, 2);
assert.ok(state.source && !state.source.shared);
await editing.observe();
const expected = [...'# nested\n'].map(scalar => ({ scalar }));
assert.deepEqual(state.window.units, expected);
await editing.replace(2n, 8n, 'resumed'); await editing.undo();
assert.deepEqual(state.window.units, expected);
const buffer = state.source.buffer;
await editing.close();
const deadline = performance.now() + 5000;
for (;;) {
  const line = await client.wire([client.token, 'disconnect']);
  if (line === 'ok') break;
  assert.equal(line, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline); await setTimeout(10);
}
console.log(JSON.stringify({ openings, resumes, buffer,
  captureAndInitializationResumed: true, interruptedResumeRetained: true,
  originalBytes: true, editUndo: true, cleanup: true }));
