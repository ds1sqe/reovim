// Run against a bridge serving configured client-owned source and Text modules.
import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Client, hex, message, u64, unhex } from '../../target/browser/sdk/typescript/resource.js';
import * as text from '../../target/browser/modules/text/api/typescript.js';

const origin = new URL(process.argv[2]).origin;
const nativeFetch = globalThis.fetch;
let loseEdit = false;
let loseNext = false;
let intervene = false;
globalThis.fetch = async (url, options) => {
  const fields = JSON.parse(options.body);
  const reply = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  if (intervene && fields[1] === 'call' && fields[5] === 'text.replace') {
    intervene = false;
    const accepted = message(await reply.clone().text());
    const revision = new DataView(accepted.data.buffer).getBigUint64(0, true);
    const peer = new Client(fields[0], url);
    const request = await peer.reserve(['write', accepted.resources[0], 'source.splice', '1',
      hex(u64(revision)) + text.range(0n, 0n), accepted.resources[0]]);
    assert.match(await peer.send(request), /^ok\t/);
    await peer.release(request.id);
  }
  if (loseEdit && fields[1] === 'call' && fields[5] === 'text.replace') {
    loseEdit = false;
    await reply.text();
    throw new Error('injected lost accepted edit reply');
  }
  if (loseNext && fields[1] === 'call' && fields[5] === 'text.next') {
    loseNext = false;
    await reply.text();
    throw new Error('injected lost change reply');
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
const slow = saved.observed;
let revision = BigInt(saved.source.revision);
for (let index = 0; index < 33; index++) {
  const request = await client.reserve(['write', saved.source.buffer, 'source.splice', '1',
    hex(u64(revision)) + text.range(0n, 0n) + '78', saved.source.buffer]);
  const reply = message(await client.send(request));
  revision = new DataView(reply.data.buffer).getBigUint64(0, true);
  await client.release(request.id);
}
loseNext = true;
await assert.rejects(resumed.follow(), /Outcome unavailable/);
await resumed.recover();
assert.equal(saved.change.delivery.kind, 'gap');
assert.equal(saved.needsRefresh, true);
await resumed.follow();
assert.equal(saved.observed.revision, revision.toString());
assert.notEqual(saved.observed.watch, slow.watch);
assert.deepEqual(saved.window.units, [...scalar('x'.repeat(33)), ...before]);
await resumed.follow();
assert.equal(saved.change.delivery.kind, 'pending');
const current = saved.observed;
for (const value of ['y', 'z']) {
  const request = await client.reserve(['write', saved.source.buffer, 'source.splice', '1',
    hex(u64(revision)) + text.range(0n, 0n) + hex(new TextEncoder().encode(value)), saved.source.buffer]);
  const reply = message(await client.send(request));
  revision = new DataView(reply.data.buffer).getBigUint64(0, true);
  await client.release(request.id);
}
loseNext = true;
await assert.rejects(resumed.follow(), /Outcome unavailable/);
await resumed.recover();
assert.equal(saved.change.delivery.kind, 'item');
assert.equal(saved.change.delivery.before, current.revision);
assert.equal(saved.change.delivery.after, (BigInt(current.revision) + 1n).toString());
const next = await client.reserve(['read', current.watch, 'text.next', '1', '00']);
const remaining = text.change(await client.send(next), current);
await client.release(next.id);
assert.equal(remaining.delivery.after, revision.toString());
await resumed.follow();
assert.deepEqual(saved.window.units, [...scalar('zy' + 'x'.repeat(33)), ...before]);
const prior = BigInt(saved.source.revision);
intervene = true;
await assert.rejects(resumed.replace(0n, 0n, '!'), /changed after acceptance/);
assert.equal(BigInt(saved.source.revision), prior + 2n);
assert.deepEqual(saved.window.units, [...scalar('!zy' + 'x'.repeat(33)), ...before]);
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
console.log('Browser editing, lost edit/change replies, stale input, Undo, Gap renewal and cleanup passed');
