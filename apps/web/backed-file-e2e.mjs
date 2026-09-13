import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Client, hex, message } from '../../target/browser/sdk/typescript/resource.js';
import { Editing } from '../../target/browser/apps/web/editing.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
globalThis.fetch = (url, options) => nativeFetch(url, { ...options,
  headers: { ...options.headers, Origin: origin } });
const first = { retiring: [] }, second = { retiring: [] };
const a = new Editing(first, () => {}, `${origin}/rpc`);
const b = new Editing(second, () => {}, `${origin}/rpc`);
await a.openFile('draft.md'); await b.openFile('draft.md');
assert.notEqual(first.token, second.token);
const clients = [first, second].map(state => new Client(state.token, `${origin}/rpc`));

async function select(client) {
  const request = await client.reserve(['write', 'modules/files', 'host.files.select',
    '1', hex(new TextEncoder().encode('draft.md'))]);
  const selected = message(await client.send(request));
  await client.release(request.id); return selected;
}
const left = await select(clients[0]), right = await select(clients[1]);
// IDs are scoped. Resolve the shared directory subject independently in each
// Client, then compare its backend-owned child name across those namespaces.
assert.equal(left.resources[0], first.source.backing);
assert.equal(right.resources[0], second.source.backing);
assert.deepEqual(left.data, right.data);
const units = text => [...text].map(scalar => ({ scalar }));
assert.deepEqual(first.window.units, units('# peers\n'));
assert.deepEqual(second.window.units, units('# peers\n'));
await a.replace(2n, 7n, 'first');
await b.observe();
assert.equal(second.source.revision, '0');
assert.deepEqual(second.window.units, units('# peers\n'));
await b.replace(2n, 7n, 'second');
await a.undo();
assert.deepEqual(first.window.units, units('# peers\n'));
await b.observe();
assert.deepEqual(second.window.units, units('# second\n'));
await a.close();
await b.replace(2n, 8n, 'last'); await b.undo();
assert.deepEqual(second.window.units, units('# second\n'));
await b.close();
for (const client of clients) {
  const deadline = performance.now() + 5000;
  for (;;) {
    const result = await client.wire([client.token, 'disconnect']);
    if (result === 'ok') break;
    assert.equal(result, 'error\tclient cleanup: Resource error 8');
    assert.ok(performance.now() < deadline); await setTimeout(10);
  }
}
console.log(JSON.stringify({ twoClients: true, sharedBacking: true,
  independentStateAndHistory: true, independentClose: true, cleanup: true }));
