import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Client } from '../../target/browser/sdk/typescript/resource.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
let loseClose = false, loseObservation = false;
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (loseClose && fields[1] === 'call' && fields[3] === 'close') {
    loseClose = false; await response.text(); throw new Error('lost view close');
  }
  if (loseObservation && fields[1] === 'call' && fields[5] === 'text.observe') {
    loseObservation = false; await response.text(); throw new Error('lost observation');
  }
  return response;
};
let state = { retiring: [] }, saved;
const persist = () => { saved = structuredClone(state); };
let editor = new Editing(state, persist, `${origin}/rpc`);
const value = () => state.window.units.map(unit => unit.scalar).join('');
const files = process.argv[3] === 'files';
const ending = files ? '\n' : '';
if (files) await editor.openFile('one.md');
else await editor.create(new TextEncoder().encode('first'));
await editor.replace(0n, 5n, 'first edit');
const first = { ...state.source }, oldView = state.access;
loseClose = true;
await assert.rejects(editor.park(), /Outcome unavailable/);
assert.equal(saved.source.buffer, first.buffer);
assert.ok(saved.receipt);
state = JSON.parse(JSON.stringify(saved));
editor = new Editing(state, persist, `${origin}/rpc`);
await editor.recover();
await editor.park();
assert.equal(state.source, undefined);
assert.equal(state.inactive[0].buffer, first.buffer);
const client = new Client(state.token, `${origin}/rpc`);
const probe = await client.reserve(['read', oldView, 'text.observe', '1']);
assert.match(await client.send(probe), /^error\t/);
await client.release(probe.id);
if (files) {
  await assert.rejects(editor.openFile('missing.md'));
  assert.equal(state.inactive[0].buffer, first.buffer);
  await editor.openFile('two.md');
} else await editor.create(new TextEncoder().encode('second'));
await editor.replace(0n, 6n, 'second edit');
const second = { ...state.source };
loseObservation = true;
await assert.rejects(editor.switchTo(first.buffer), /Outcome unavailable/);
assert.equal(saved.source.buffer, first.buffer);
assert.equal(saved.inactive[0].buffer, second.buffer);
state = JSON.parse(JSON.stringify(saved));
editor = new Editing(state, persist, `${origin}/rpc`);
await editor.recover();
await editor.observe();
assert.equal(value(), 'first edit' + ending);
assert.equal(state.source.history, first.history);
assert.equal(state.inactive[0].buffer, second.buffer);
await editor.undo(); assert.equal(value(), 'first' + ending);
await editor.redo(); assert.equal(value(), 'first edit' + ending);
await editor.switchTo(second.buffer);
assert.equal(value(), 'second edit' + ending);
assert.equal(state.source.history, second.history);
await assert.rejects(editor.switchTo('@999999'), /not open/);
assert.equal(state.source.buffer, second.buffer);
await editor.undo(); assert.equal(value(), 'second' + ending);
await editor.close();
await editor.switchTo(first.buffer);
assert.equal(value(), 'first edit' + ending);
await editor.close();
assert.deepEqual(state.inactive, []);
assert.equal(state.source, undefined);
const deadline = performance.now() + 5000;
for (;;) {
  const response = await client.wire([client.token, 'disconnect']);
  if (response === 'ok') break;
  assert.equal(response, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline, 'Client cleanup did not settle');
  await setTimeout(10);
}
console.log('Two dirty Buffers retain independent History across cleanup loss, reload and switching.');
