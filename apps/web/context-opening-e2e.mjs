import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';
import { Switching } from '../../target/browser/clients/gui/runtime/switching.js';
import { Client, opening } from '../../target/browser/sdk/typescript/resource.js';
import * as context from '../../target/browser/modules/context/api/typescript.js';

const origin = new URL(process.argv[2]).origin, endpoint = `${origin}/rpc`;
const kind = process.argv[3] ?? 'create';
const contract = { create: 'source.initial', file: 'command.open', join: 'kernel.buffer.describe' }[kind];
assert.ok(contract);
const nativeFetch = globalThis.fetch, preparations = new Set(), updates = new Set();
let loseOpening = true, loseFocus = false, loseView = false;
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (fields[1] === 'call' && fields[5] === contract) {
    preparations.add(fields[2]);
    if (loseOpening) { loseOpening = false; await response.text(); throw new Error('lost opening'); }
  }
  if (fields[1] === 'call' && fields[5] === 'editing.context.update') {
    updates.add(fields[2]);
    if (loseFocus) { loseFocus = false; await response.text(); throw new Error('lost focus'); }
  }
  if (loseView && fields[1] === 'call' && fields[5] === 'text.observe') {
    loseView = false; await response.text(); throw new Error('lost view');
  }
  return response;
};
const client = await Client.connect(opening(), () => {}, endpoint);
let state = { token: client.token, retiring: [] }, focused = { retiring: [] }, transition = {};
let savedState, savedFocus, savedTransition, editor, focus, switching;
function connect() {
  editor = new Editing(state, () => { savedState = structuredClone(state); }, endpoint);
  focus = new Focus(client, focused, () => { savedFocus = structuredClone(focused); });
  switching = new Switching(focus, {
    current: () => state.source?.buffer,
    known: () => [...(state.inactive ?? []).map(source => source.buffer),
      ...(state.source ? [state.source.buffer] : [])],
    pending: () => !!(state.receipt || state.resuming || state.openingCleanup || state.retiring.length),
    show: target => target ? editor.switchTo(target) : editor.park(),
    recover: () => editor.recover(),
    close: async target => {
      if (state.source?.buffer === target) await editor.close();
      else assert.ok(!state.inactive?.some(source => source.buffer === target));
    }
  }, transition, () => { savedTransition = structuredClone(transition); });
}
function reload() {
  state = structuredClone(savedState); focused = structuredClone(savedFocus);
  transition = structuredClone(savedTransition); connect();
}
connect();
await focus.create({ known: [] });
const prepare = () => kind === 'file'
  ? editor.openFile('draft.md', 'open', 'modules/files', 'file_source', false)
  : kind === 'join' ? editor.join(false)
    : editor.create(new TextEncoder().encode('opened'), false);
await assert.rejects(switching.open(prepare));
assert.ok(transition.opening);
assert.equal(state.source, undefined);
const original = state.receipt.id;
reload(); loseFocus = true;
await assert.rejects(switching.recover());
assert.ok(state.source);
assert.equal(state.access, undefined);
assert.equal(state.window, undefined);
assert.equal(transition.opening, undefined);
assert.equal(transition.pending.phase, 'focus');
const buffer = state.source.buffer, update = transition.pending.request.id;
reload(); loseView = true;
await assert.rejects(switching.recover());
assert.equal(focused.current.active, buffer);
assert.equal(state.source.buffer, buffer);
assert.equal(transition.pending.phase, 'view');
reload(); await switching.recover();
assert.deepEqual([...preparations], [original]);
assert.deepEqual([...updates], [update]);
assert.deepEqual(focused.current.known, [buffer]);
assert.equal(state.window.units.map(unit => unit.scalar).join(''),
  kind === 'join' ? 'shared' : kind === 'file' ? 'opened\n' : 'opened');
await switching.close(buffer);
assert.deepEqual(focused.current.known, []);
if (kind === 'file') {
  await assert.rejects(switching.open(() => editor.openFile('missing.md',
    'open', 'modules/files', 'file_source', false)));
  assert.equal(transition.opening, undefined);
  assert.equal(state.source, undefined);
}
await assert.rejects(switching.open(async () => {
  await prepare();
  const request = await client.reserve(context.update(focused.current,
    { known: [state.source.buffer] }));
  const peer = context.observation(await client.send(request));
  await client.release(request.id);
  const release = await client.reserve(['write', peer.snapshot, 'kernel.session.release', '1']);
  assert.match(await client.send(release), /^ok\t/); await client.release(release.id);
}), /error/);
assert.equal(transition.opening, undefined);
assert.equal(transition.pending, undefined);
assert.ok(state.source);
assert.equal(state.access, undefined);
assert.equal(state.window, undefined);
await focus.refresh();
await switching.select(state.source.buffer);
await switching.close(state.source.buffer);
await focus.close();
const deadline = performance.now() + 5000;
for (;;) {
  const reply = await client.wire([client.token, 'disconnect']);
  if (reply === 'ok') break;
  assert.equal(reply, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline); await setTimeout(10);
}
console.log(`${kind}: opening, focus and view delivery recover without repeating preparation.`);
