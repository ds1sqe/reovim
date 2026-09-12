import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import { Client, DeliveryError, message, outcome } from '../../target/browser/sdk/typescript/resource.js';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';
import * as context from '../../target/browser/modules/context/api/typescript.js';

const origin = new URL(process.argv[2]).origin, nativeFetch = globalThis.fetch;
let loseFollow = false;
globalThis.fetch = async (url, options) => {
  const response = await nativeFetch(url, { ...options,
    headers: { ...options.headers, Origin: origin } });
  const fields = JSON.parse(options.body);
  if (loseFollow && fields[1] === 'call' && fields[5] === 'kernel.session.follow') {
    loseFollow = false; await response.text(); throw new Error('lost follow reply');
  }
  return response;
};
const journal = { retiring: [] }, focusJournal = { retiring: [] };
const editing = new Editing(journal, () => {}, `${origin}/rpc`);
await editing.join();
const client = new Client(journal.token, `${origin}/rpc`);
const focus = new Focus(client, focusJournal, () => {});
const buffer = journal.source.buffer, before = journal.source.revision;
await focus.create({ known: [buffer], active: buffer });
const initial = focusJournal.current, snapshots = [], watches = [];
async function call(fields, decode = message) {
  const receipt = await client.reserve(fields);
  const adopted = decode(await client.send(receipt));
  await client.release(receipt.id);
  return adopted;
}
function observed(line) {
  const value = context.observation(line);
  snapshots.push(value.snapshot); return value;
}
function followed(line) {
  const value = context.followed(line);
  snapshots.push(value.observation.snapshot); watches.push(value.watch);
  return value;
}
const participant = await call(['read', initial.session, 'kernel.session.attach', '1'],
  line => message(line).resources[0]);
function update(base, active) {
  const fields = context.update(base, { known: [buffer], active });
  fields[1] = participant; return call(fields, observed);
}
const cleared = await update(initial);
assert.equal(cleared.session, initial.session);
assert.deepEqual(cleared.known, [buffer]);
assert.equal(cleared.active, undefined);
const receipt = await client.reserve(['read', participant, 'kernel.session.follow', '1']);
loseFollow = true;
await assert.rejects(client.send(receipt), DeliveryError);
const active = await update(cleared, buffer);
const recovered = await client.send(receipt);
assert.equal(await client.send(receipt), recovered);
const baseline = followed(recovered);
await client.release(receipt.id);
assert.equal(baseline.observation.active, undefined);
const edge = await call(['read', baseline.watch, 'kernel.session.next', '1', '00'], context.change);
assert.equal(edge.kind, 'item');
assert.equal(edge.after, (BigInt(baseline.revision) + 1n).toString());
await call(['write', participant, 'kernel.session.seek', '1', '', initial.snapshot]);
const historical = await call(['read', participant, 'editing.context', '1'], observed);
assert.equal(historical.active, buffer);
const stale = context.update(historical, { known: [buffer] });
stale[1] = participant;
const rejected = await call(stale, outcome);
assert.equal(rejected.kind, 'failed'); assert.equal(rejected.code, 7n);
const live = await call(['read', participant, 'kernel.session.follow', '1'], followed);
assert.equal(live.observation.active, active.active);
assert.equal(live.revision, edge.after);
await call(['write', participant, 'kernel.session.detach', '1']);
const fenced = await call(['read', live.watch, 'kernel.session.next', '1', '00'], outcome);
assert.equal(fenced.kind, 'failed'); assert.equal(fenced.code, 6n);
for (const id of watches) await call(['close', id], line => assert.equal(line, 'ok'));
for (const id of snapshots) await call(['write', id, 'kernel.session.release', '1']);
await editing.observe();
assert.equal(journal.source.revision, before);
await focus.close(); await editing.close();
const deadline = performance.now() + 5000;
for (;;) {
  const result = await client.wire([client.token, 'disconnect']);
  if (result === 'ok') break;
  assert.equal(result, 'error\tclient cleanup: Resource error 8');
  assert.ok(performance.now() < deadline); await setTimeout(10);
}
console.log('Native reference-bearing participation updates, retained follow recovery, ABA rejection and cleanup passed');
