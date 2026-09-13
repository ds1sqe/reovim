import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { setTimeout } from 'node:timers/promises';
import { Client, message } from '../../target/browser/sdk/typescript/resource.js';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';
import * as context from '../../target/browser/modules/context/api/typescript.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
let state = { retiring: [] }, focusState = { retiring: [] };
let lostOpen = true, lostFocus = false, opened, openReceipt;
const openingRequests = new Set();
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (fields[1] === 'call' && fields[5] === 'command.open') {
    openingRequests.add(fields[2]);
    if (lostOpen) {
      opened = message(await response.text()); openReceipt = fields[2];
      lostOpen = false; throw new Error('lost accepted file opening');
    }
  }
  if (lostFocus && fields[1] === 'call' && fields[5] === 'editing.context.update') {
    lostFocus = false; await response.text(); throw new Error('lost focus update');
  }
  return response;
};
let editing = new Editing(state, () => {}, `${origin}/rpc`);
await assert.rejects(editing.openFile('draft.md'), /Outcome unavailable/);
assert.equal(state.receipt.id, openReceipt);
assert.equal(state.source, undefined);
await writeFile(process.argv[3], '# replaced on disk\n');
state = JSON.parse(JSON.stringify(state));
editing = new Editing(state, () => {}, `${origin}/rpc`);
await editing.recover();
assert.equal(state.source.buffer, opened.resources[0]);
assert.equal(state.source.shared, undefined);
assert.equal(openingRequests.size, 1, 'recovery cannot submit another opening');
await editing.observe();
const original = [...'# file\n'].map(scalar => ({ scalar }));
original.push({ opaque: 255 });
assert.deepEqual(state.window.units, original);

const client = new Client(state.token, `${origin}/rpc`);
let focus = new Focus(client, focusState, () => {});
await focus.create({ known: [] });
async function rpc(fields) {
  const receipt = await client.reserve(fields), line = await client.send(receipt);
  assert.ok(line.startsWith('ok'), line); await client.release(receipt.id);
  return line;
}
// Another update wins after this Client's retained context observation.
const competing = context.update(focusState.current, { known: [] });
competing[1] = focusState.participation;
const other = context.observation(await rpc(competing));
await assert.rejects(focus.set({ known: [state.source.buffer], active: state.source.buffer }),
  /error\tfailed\t7/);
assert.equal(state.source.buffer, opened.resources[0]);
await editing.observe(); assert.deepEqual(state.window.units, original);
await focus.refresh();
await rpc(['write', other.snapshot, 'kernel.session.release', '1']);
lostFocus = true;
await assert.rejects(focus.set({ known: [state.source.buffer], active: state.source.buffer }),
  /Outcome unavailable/);
focusState = JSON.parse(JSON.stringify(focusState));
focus = new Focus(client, focusState, () => {}); await focus.recover();
assert.equal(focusState.current.active, state.source.buffer);
assert.equal(state.source.revision, '0', 'context updates do not edit Source');
await editing.replace(2n, 6n, 'changed'); await editing.undo();
assert.deepEqual(state.window.units, original);
await focus.set({ known: [] }); await focus.close(); await editing.close();
assert.equal(state.source, undefined);
const deadline = performance.now() + 5000;
for (;;) {
  const line = await client.wire([client.token, 'disconnect']);
  if (line === 'ok') break;
  assert.equal(line, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline); await setTimeout(10);
}
console.log(JSON.stringify({ openReceipt, buffer: opened.resources[0],
  recoveredOriginalBytes: true, staleFocusPreservedBuffer: true,
  recoveredFocus: true, editUndo: true, cleanup: true }));
