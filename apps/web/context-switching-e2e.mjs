import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';
import { Switching } from '../../target/browser/clients/gui/runtime/switching.js';
import { Client, hex, u64 } from '../../target/browser/sdk/typescript/resource.js';
import * as context from '../../target/browser/modules/context/api/typescript.js';

const origin = new URL(process.argv[2]).origin, endpoint = `${origin}/rpc`;
const nativeFetch = globalThis.fetch, updates = new Set();
const closes = new Set();
let loseFocus = false, loseView = false, loseCleanup = false, rejectView = false;
let loseClose = false;
let raceClose;
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  const reply = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (fields[1] === 'call' && fields[5] === 'editing.context.update') {
    updates.add(fields[2]);
    if (raceClose) {
      const selected = raceClose; raceClose = undefined;
      const peer = new Client(fields[0], url);
      const edit = await peer.reserve(['write', selected.buffer, 'source.splice', '1',
        hex(u64(BigInt(selected.revision))) + hex(u64(0n)) + hex(u64(0n)) + '21', selected.buffer]);
      assert.match(await peer.send(edit), /^ok\t/); await peer.release(edit.id);
    }
    if (loseFocus) { loseFocus = false; await reply.text(); throw new Error('lost focus'); }
  }
  if (loseView && fields[1] === 'call' && fields[5] === 'text.observe') {
    loseView = false; await reply.text(); throw new Error('lost view');
  }
  if (loseCleanup && fields[1] === 'call' && fields[5] === 'kernel.session.release') {
    loseCleanup = false; await reply.text(); throw new Error('lost context cleanup');
  }
  if (fields[1] === 'call' && fields[5] === 'kernel.buffer.close') {
    closes.add(fields[2]);
    if (loseClose) { loseClose = false; await reply.text(); throw new Error('lost Buffer close'); }
  }
  return reply;
};
let state = { retiring: [] }, focused = { retiring: [] }, transition = {};
let savedState, savedFocus, savedTransition, editor, focus, switching;
function connect() {
  editor = new Editing(state, () => { savedState = structuredClone(state); }, endpoint);
  focus = new Focus(new Client(state.token, endpoint), focused,
    () => { savedFocus = structuredClone(focused); });
  switching = new Switching(focus, {
    current: () => state.source?.buffer,
    pending: () => !!(state.receipt || state.retiring.length),
    known: () => [...(state.inactive ?? []).map(source => source.buffer),
      ...(state.source ? [state.source.buffer] : [])],
    show: async target => {
      if (rejectView) { rejectView = false; throw new Error('view unavailable'); }
      if (target) await editor.switchTo(target); else await editor.park();
    },
    recover: () => editor.recover(),
    close: async target => {
      if (state.source?.buffer === target) await editor.close();
      else assert.ok(!state.inactive?.some(source => source.buffer === target));
    }
  }, transition, () => { savedTransition = structuredClone(transition); });
}
editor = new Editing(state, () => { savedState = structuredClone(state); }, endpoint);
await editor.create(new TextEncoder().encode('first'));
await editor.replace(0n, 5n, 'first edited');
const first = state.source.buffer;
await editor.park();
await editor.create(new TextEncoder().encode('second'));
await editor.replace(0n, 6n, 'second edited');
const second = state.source.buffer;
connect();
await focus.create({ known: [first, second], active: second });
loseFocus = true;
await assert.rejects(switching.select(first), /Outcome unavailable/);
assert.equal(state.source.buffer, second);
const original = focused.receipt.id;
assert.equal(savedTransition.pending.request.id, original);
state = structuredClone(savedState); focused = structuredClone(savedFocus);
transition = structuredClone(savedTransition); connect();
loseView = true;
await assert.rejects(switching.recover(), /Outcome unavailable/);
assert.equal(focused.current.active, first);
assert.equal(transition.pending.phase, 'view');
assert.equal(state.source.buffer, first);
assert.equal(state.inactive[0].buffer, second);
state = structuredClone(savedState); focused = structuredClone(savedFocus);
transition = structuredClone(savedTransition); connect();
await switching.recover();
assert.deepEqual([...updates], [original]);
assert.equal(transition.pending, undefined);
assert.equal(state.window.units.map(unit => unit.scalar).join(''), 'first edited');

loseCleanup = true;
await assert.rejects(switching.select(second), /Outcome unavailable/);
assert.equal(transition.pending.phase, 'view');
assert.equal(focused.current.active, second);
assert.equal(state.source.buffer, first);
const acceptedSecond = transition.pending.request.id;
await switching.recover();
assert.deepEqual([...updates], [original, acceptedSecond]);
assert.equal(state.source.buffer, second);

const client = new Client(state.token, endpoint);
async function call(fields) {
  const receipt = await client.reserve(fields), reply = await client.send(receipt);
  await client.release(receipt.id); assert.match(reply, /^ok\t/); return reply;
}
const peer = context.observation(await call(context.update(focused.current,
  { known: [first, second], active: first })));
await assert.rejects(switching.select(second), /error/);
assert.equal(transition.pending, undefined);
assert.equal(state.source.buffer, second);
await focus.refresh();
await call(['write', peer.snapshot, 'kernel.session.release', '1']);
rejectView = true;
await assert.rejects(switching.select(first), /view unavailable/);
assert.equal(transition.pending.phase, 'view');
assert.equal(focused.current.active, first);
assert.equal(state.source.buffer, second);
await switching.select(second);
assert.equal(focused.current.active, second);
assert.equal(state.window.units.map(unit => unit.scalar).join(''), 'second edited');
await switching.select();
assert.equal(focused.current.active, undefined);
assert.deepEqual(new Set(focused.current.known), new Set([first, second]));
assert.equal(state.source, undefined);
await switching.select(first);
await editor.undo();
assert.equal(state.window.units.map(unit => unit.scalar).join(''), 'first');
await assert.rejects(switching.close(second), /current Buffer/);
await focus.set({ known: [first, second], active: second });
raceClose = { ...state.source };
loseClose = true;
await assert.rejects(switching.close(first), /Outcome unavailable/);
const refusedClose = state.receipt.id;
await assert.rejects(switching.recover(), /error/);
assert.deepEqual([...closes], [refusedClose]);
assert.equal(transition.pending, undefined);
assert.equal(state.source.buffer, first);
assert.deepEqual(focused.current.known, [second]);
assert.equal(focused.current.active, second);
await editor.observe();
assert.equal(state.window.units.map(unit => unit.scalar).join(''), '!first');
await switching.select(first);
loseFocus = true;
await assert.rejects(switching.close(first), /Outcome unavailable/);
assert.equal(state.source.buffer, first);
assert.equal(transition.pending.closing, first);
await assert.rejects(switching.select(second), /Recover/);
const closingFocus = transition.pending.request.id;
loseClose = true;
await assert.rejects(switching.recover(), /Outcome unavailable/);
assert.equal(focused.current.active, undefined);
assert.deepEqual(focused.current.known, [second]);
state = structuredClone(savedState); focused = structuredClone(savedFocus);
transition = structuredClone(savedTransition); connect();
await switching.recover();
assert.equal(transition.pending, undefined);
assert.equal(state.source, undefined);
assert.equal(focused.acceptedUpdate.id, closingFocus);
await switching.select(second);
assert.equal(state.window.units.map(unit => unit.scalar).join(''), 'second edited');
await switching.close(second);
assert.deepEqual(focused.current.known, []);
assert.equal(focused.current.active, undefined);
await focus.close();
const deadline = performance.now() + 5000;
for (;;) {
  const reply = await client.wire([client.token, 'disconnect']);
  if (reply === 'ok') break;
  assert.equal(reply, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline); await setTimeout(10);
}
console.log('Context switching and close survive lost replies, reload, stale focus and concurrent edits.');
