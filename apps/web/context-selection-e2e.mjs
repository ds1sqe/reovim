import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';
import { Switching } from '../../target/browser/clients/gui/runtime/switching.js';
import { Client } from '../../target/browser/sdk/typescript/resource.js';

const origin = new URL(process.argv[2]).origin, endpoint = `${origin}/rpc`, nativeFetch = globalThis.fetch;
let lost, seeks = new Set(), updates = new Set();
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (fields[1] === 'call' && fields[5] === 'editing.context.update') updates.add(fields[2]);
  if (fields[1] === 'call' && fields[5] === 'kernel.session.seek') seeks.add(fields[2]);
  if (lost && fields[1] === 'call' && fields[5] === lost) {
    lost = undefined; await response.text(); throw new Error('lost selection');
  }
  return response;
};
let state = { retiring: [] }, focused = { retiring: [] }, transition = {};
let savedState, savedFocus, savedTransition, editor, focus, switching;
function connect() {
  editor = new Editing(state, () => { savedState = structuredClone(state); }, endpoint);
  focus = new Focus(new Client(state.token, endpoint), focused,
    () => { savedFocus = structuredClone(focused); });
  switching = new Switching(focus, {
    current: () => state.source?.buffer,
    known: () => [...(state.inactive ?? []).map(source => source.buffer), ...(state.source ? [state.source.buffer] : [])],
    pending: () => !!(state.receipt || state.retiring.length),
    show: target => target ? editor.showContext(target) : editor.park(),
    recover: () => editor.recover(),
    close: async target => { if (state.source?.buffer === target) await editor.close(); }
  }, transition, () => { savedTransition = structuredClone(transition); });
}
function reload() {
  state = structuredClone(savedState); focused = structuredClone(savedFocus);
  transition = structuredClone(savedTransition); connect();
}
editor = new Editing(state, () => { savedState = structuredClone(state); }, endpoint);
await editor.create(new TextEncoder().encode('first'));
const first = state.source.buffer;
await editor.park(); await editor.create(new TextEncoder().encode('second'));
const second = state.source.buffer;
connect(); await focus.join();
await focus.set({ known: [first, second], active: first });
await editor.switchTo(first); await focus.remember();
await editor.replace(0n, 5n, 'later first');
await switching.select(second);
const writes = [...updates];
lost = 'kernel.session.seek';
await assert.rejects(switching.observe('seek'));
const original = focused.receipt.id;
assert.equal(state.source.buffer, second);
reload(); lost = 'text.observe';
await assert.rejects(switching.recover());
assert.equal(focused.historical, true);
reload(); await switching.recover();
assert.deepEqual([...seeks], [original]);
assert.deepEqual([...updates], writes);
assert.equal(state.source.buffer, first);
assert.equal(state.window.units.map(unit => unit.scalar).join(''), 'later first');
await switching.observe('follow');
assert.equal(focused.historical, false);
assert.equal(state.source.buffer, second);

const peerState = { retiring: [] };
const peerEditor = new Editing(peerState, () => {}, endpoint);
await peerEditor.create(new TextEncoder().encode('peer'), false);
const peerClient = new Client(peerState.token, endpoint);
const peer = new Focus(peerClient, { retiring: [] }, () => {});
await peer.join();
const peerKnown = [...peer.journal.current.known];
await peer.set({ known: [...peerKnown, peerState.source.buffer], active: peerState.source.buffer });
lost = 'kernel.session.next';
await assert.rejects(switching.observe('follow'));
await switching.recover();
assert.equal(state.source.readOnly, true);
assert.equal(state.window.units.map(unit => unit.scalar).join(''), 'peer');
assert.equal(transition.observing, undefined);
const client = new Client(state.token, endpoint);
await assert.rejects(editor.replace(0n, 0n, '!'), /read-only/);
const readonly = state.source.buffer;
const probe = await client.reserve(['write', readonly, 'kernel.buffer.close', '1', '000000000000000001', readonly]);
assert.match(await client.send(probe), /^error\t(?:rejected|failed)\t1\t/); await client.release(probe.id);
const editProbe = await client.reserve(['write', readonly, 'source.splice', '1',
  '00000000000000000000000000000000000000000000000021', readonly]);
assert.match(await client.send(editProbe), /^error\t(?:rejected|failed)\t1\t/);
await client.release(editProbe.id);
await peer.set({ known: peerKnown, active: peerKnown[1] });
await switching.observe('follow');
assert.equal(state.source.buffer, second);
await editor.switchTo(readonly); await editor.close(); await editor.switchTo(second);
await peer.set({ known: [...peerKnown, peerState.source.buffer], active: peerState.source.buffer });
await peerEditor.close();
await assert.rejects(switching.observe('follow'));
assert.ok(state.source?.buffer === second || state.inactive.some(source => source.buffer === second));
await peer.set({ known: peerKnown, active: peerKnown[1] });
await switching.observe('follow');
assert.equal(state.source.buffer, second);
await peer.close();
await focus.set({ known: [] }); await focus.close();
await editor.close(); await editor.switchTo(first); await editor.close();
for (const owned of [client, peerClient]) {
  const deadline = performance.now() + 5000;
  for (;;) {
    const reply = await owned.wire([owned.token, 'disconnect']);
    if (reply === 'ok') break;
    assert.equal(reply, 'error\tclient cleanup: Resource error 8');
    assert.ok(performance.now() < deadline); await setTimeout(10);
  }
}
console.log('Seek/follow preserve current Buffer data; peer context grants only read-only views.');
