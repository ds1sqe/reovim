import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Client, unhex } from '../../target/browser/sdk/typescript/resource.js';
import { Commands } from '../../target/browser/clients/gui/runtime/commands.js';
import { editor } from '../../target/browser/clients/gui/runtime/editor.js';

const origin = new URL(process.argv[2]).origin;
const nativeFetch = globalThis.fetch;
globalThis.fetch = (url, options) => nativeFetch(url, { ...options,
  headers: { ...options.headers, Origin: origin } });
const state = { retiring: [] };
const editing = new Editing(state, () => {}, `${origin}/rpc`);
await editing.create(unhex('73747261c39f650d0aff'));
let end = 6n;
const capability = editor(() => ({ snapshot: state.observed.snapshot,
  window: state.window, start: 0n, end }),
  (start, end, text) => editing.replace(start, end, text));
const commands = new Commands();
const { activate } = await import('../../target/browser/clients/gui/modules/uppercase/index.js');
const installed = activate(capability, commands.register);
const old = capability.selection();
await commands.invoke('text.uppercase');
assert.deepEqual(state.window.units,
  [...'STRASSE\r\n'].map(scalar => ({ scalar })).concat({ opaque: 255 }));
await assert.rejects(capability.replace(old, 'stale'), /stale/);
await editing.undo();
assert.deepEqual(state.window.units,
  [...'straße\r\n'].map(scalar => ({ scalar })).concat({ opaque: 255 }));
end = 9n;
await assert.rejects(commands.invoke('text.uppercase'), /scalar-only/);
assert.equal(state.source.revision, '2');
installed.close();
await assert.rejects(commands.invoke('text.uppercase'), /unavailable/);
await editing.close();
const client = new Client(state.token, `${origin}/rpc`);
const deadline = performance.now() + 5000;
for (;;) {
  const response = await client.wire([client.token, 'disconnect']);
  if (response === 'ok') break;
  assert.equal(response, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline, 'Client cleanup did not settle');
  await setTimeout(10);
}
console.log('Public GUI contribution activation, native Unicode edit/Undo, stale/opaque rejection and unload passed');
