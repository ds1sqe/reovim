import test from 'node:test';
import assert from 'node:assert/strict';
import { initial, observation, update, watching, followed, change } from '../../target/browser/modules/context/api/typescript.js';
import { Client, hex, u64 } from '../../target/browser/sdk/typescript/resource.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';
import { Editing } from '../../target/browser/apps/web/editing.js';
import { result as retainedResult } from '../../target/browser/sdk/typescript/requests.js';

test('retained results preserve values, failure context and nested locators', () => {
  const name = new TextEncoder().encode('source.bytes');
  assert.deepEqual(retainedResult(`ok\tkernel.request.result\t1\t0001000000${hex(u64(BigInt(name.length)))}${hex(name)}ff\t@1,@2`),
    { kind: 'ok', value: { contract: 'source.bytes', schema: 1,
      data: new Uint8Array([255]), resources: ['@1', '@2'] } });
  assert.deepEqual(retainedResult(`ok\tkernel.request.result\t1\t01${hex(u64(5n))}\t@3`),
    { kind: 'failed', code: 5n, resource: '@3', recovery: '' });
  const locator = hex(new TextEncoder().encode('requests/original'));
  assert.equal(retainedResult(`ok\tkernel.request.result\t1\t02${hex(u64(8n))}${locator}\t`).recovery,
    'requests/original');
  for (const data of ['00', `01${hex(u64(5n))}00`, `02${hex(u64(8n))}`]) {
    assert.throws(() => retainedResult(`ok\tkernel.request.result\t1\t${data}\t`));
  }
});

test('nested opening recovery retains its work across original receipt retirement', async t => {
  let state = { token: 'client', retiring: [] }, id = 0, failRelease = true;
  let cleanupReceipt, loseCleanup = true;
  const calls = [], locator = hex(new TextEncoder().encode('requests/original'));
  t.mock.method(Client.prototype, 'wire', async fields => {
    if (fields[1] === 'reserve') return `ok\t${++id}`;
    if (fields[1] === 'release') {
      if (fields[2] === cleanupReceipt && loseCleanup) {
        loseCleanup = false; throw new Error('lost cleanup retirement');
      }
      if (fields[2] === '1' && failRelease) {
        failRelease = false; throw new Error('lost retirement');
      }
      return 'ok';
    }
    const request = fields.slice(3); calls.push(request);
    if (request[0] === 'close') { assert.equal(request[1], '@33'); return 'ok'; }
    if (request[2] === 'command.open.cleanup') {
      cleanupReceipt = fields[2]; return 'ok\tcommand.open.cleanup\t1\t\t';
    }
    if (request[0] === 'resolve') return 'ok\t@33';
    if (request[2] === 'kernel.request.result') {
      return 'ok\tkernel.request.result\t1\t0001000000070000000000000063617074757265\t@40';
    }
    if (request[2] === 'command.open') return `error\tunknown\t5\t\t${locator}`;
    assert.deepEqual(request, ['write', 'open', 'command.open.resume', '1', '', '@33']);
    return `ok\tcommand.open.resume\t1\t${hex(u64(0n))}\t@7,@8,@40`;
  });
  let editing = new Editing(state, () => {});
  await assert.rejects(editing.openFile('draft.md'), /Unknown outcome/);
  await assert.rejects(editing.recover(), /lost retirement/);
  assert.equal(state.resuming.original.id, '1'); assert.equal(state.resuming.work, '@33');
  assert.equal(state.receipt, undefined);
  state = JSON.parse(JSON.stringify(state)); editing = new Editing(state, () => {});
  await assert.rejects(editing.openFile('another.md'), /Recover/);
  await assert.rejects(editing.recover(), /lost cleanup retirement/);
  assert.equal(state.openingCleanup, undefined);
  assert.equal(state.receipt.id, cleanupReceipt);
  state = JSON.parse(JSON.stringify(state)); editing = new Editing(state, () => {});
  await editing.recover();
  assert.equal(state.source.buffer, '@7'); assert.equal(state.resuming, undefined);
  assert.deepEqual(calls.map(fields => ['resolve', 'close'].includes(fields[0]) ? fields[0] : fields[2]),
    ['command.open', 'resolve', 'kernel.request.result', 'command.open.resume', 'close', 'command.open.cleanup']);
  assert.equal(state.openingCleanup, undefined);
});

test('lost file opening adopts the original Buffer after journal reload', async t => {
  let state = { token: 'client', retiring: [] }, calls = 0, lost = true;
  const opened = `ok\tcommand.open\t1\t${hex(u64(0n))}\t@7,@8,@40`;
  t.mock.method(Client.prototype, 'wire', async fields => {
    if (fields[1] === 'reserve') return 'ok\t1';
    if (fields[1] === 'release') return 'ok';
    if (fields[1] === 'call') {
      calls++;
      assert.deepEqual(fields.slice(3), ['write', 'open', 'command.open', '1',
        hex(new TextEncoder().encode('draft.md')), 'modules/files,file_source']);
      if (lost) { lost = false; throw new Error('lost response'); }
      return opened;
    }
    assert.fail(`unexpected exchange ${fields}`);
  });
  await assert.rejects(new Editing(state, () => {}).openFile('draft.md'), /Outcome unavailable/);
  assert.equal(state.source, undefined);
  assert.equal(state.receipt.id, '1');
  state = JSON.parse(JSON.stringify(state));
  const editing = new Editing(state, () => {});
  await assert.rejects(editing.openFile('different.md'), /Recover/);
  assert.equal(calls, 1);
  await editing.recover();
  assert.deepEqual(state.source, { buffer: '@7', history: '@8', revision: '0',
    shared: undefined, backing: '@40', name: 'draft.md' });
  assert.equal(state.receipt, undefined);
  assert.equal(calls, 2, 'same receipt retrieves the server-retained result');
  await assert.rejects(editing.openFile('different.md'), /Keep the current/);
});

test('opening rejects legacy or incomplete descriptors without adopting a Buffer', async t => {
  let response;
  t.mock.method(Client.prototype, 'wire', async fields => {
    if (fields[1] === 'reserve') return 'ok\t1';
    assert.equal(fields[1], 'call'); return response;
  });
  for (const [data, refs] of [[hex(u64(0n)).repeat(3), '@7,@8'],
    [hex(u64(0n)), '@7,@8'], [hex(u64(0n)), '@7,@8,@9,@10']]) {
    const state = { token: 'client', retiring: [] };
    response = `ok\tcommand.open\t1\t${data}\t${refs}`;
    await assert.rejects(new Editing(state, () => {}).openFile('draft.md'),
      /invalid source acceptance/);
    assert.equal(state.source, undefined);
    assert.equal(state.receipt.response, response);
  }
});

test('checkpoint recovery keeps its separate owner and does not replace the selected view', async () => {
  const client = new Client('client'), seen = [];
  client.wire = async fields => { seen.push(fields); return 'ok'; };
  const state = { ownership: 'shared', participation: '@3',
    current: { session: '@1', snapshot: '@2', known: [] },
    checkpoint: { session: '@1', snapshot: '@4', known: [] },
    checkpointRequest: '7', retiring: [],
    receipt: { client: 'client', id: '7', fields: ['read', '@3', 'editing.context', '1'],
      response: 'ok\tediting.context\t1\tffffffffffffffff\t@1,@4' } };
  await new Focus(client, state, () => {}).recover();
  assert.equal(state.current.snapshot, '@2');
  assert.equal(state.checkpoint.snapshot, '@4');
  assert.deepEqual(state.retiring, []);
  assert.equal(state.receipt, undefined); assert.equal(state.checkpointRequest, undefined);
  assert.deepEqual(seen, [['client', 'release', '7']]);
});

test('shared close recovers adopted detach before retiring only its own observations', async () => {
  const client = new Client('client'), operations = [];
  let id = 0, lost = true;
  client.wire = async fields => {
    if (fields[1] === 'reserve') return `ok\t${++id}`;
    if (fields[1] === 'release') {
      if (lost) { lost = false; throw new Error('lost retirement'); }
      return 'ok';
    }
    assert.equal(fields[1], 'call');
    const request = fields.slice(3); operations.push(request);
    assert.notEqual(request[2], 'kernel.session.close');
    return request[0] === 'close' ? 'ok' : `ok\t${request[2]}\t1\t\t`;
  };
  let state = { ownership: 'shared', participation: '@3',
    current: { session: '@1', snapshot: '@2', known: [] },
    checkpoint: { session: '@1', snapshot: '@4', known: [] },
    watch: { id: '@5', session: '@1', revision: '1' }, retiring: [] };
  await assert.rejects(new Focus(client, state, () => {}).close(), /lost retirement/);
  assert.equal(state.participation, undefined); assert.equal(state.closing, true);
  state = JSON.parse(JSON.stringify(state));
  await new Focus(client, state, () => {}).recover();
  assert.equal(state.current, undefined); assert.equal(state.checkpoint, undefined);
  assert.equal(state.closing, undefined); assert.deepEqual(state.retiring, []);
  assert.deepEqual(operations, [
    ['write', '@3', 'kernel.session.detach', '1'],
    ['write', '@2', 'kernel.session.release', '1'],
    ['write', '@4', 'kernel.session.release', '1'], ['close', '@5']
  ]);
});

test('context recovery refuses lifecycle and watch records from another Session', async () => {
  const client = new Client('client');
  client.wire = async () => { assert.fail('foreign result must not retire its receipt'); };
  for (const fields of [['write', '@9', 'kernel.session.close', '1'],
    ['read', '@3', 'kernel.session.next', '1', '00']]) {
    const state = { current: { session: '@1', snapshot: '@2', known: [] }, retiring: [],
      watch: { id: '@3', session: '@9', revision: '0' },
      receipt: { client: 'client', id: '7', fields,
        response: fields[0] === 'write' ? 'ok\tkernel.session.close\t1\t\t'
          : 'ok\tkernel.session.next\t1\t03\t' } };
    const receipt = state.receipt;
    const focus = new Focus(client, state, () => {});
    await assert.rejects(focus.recover(), /foreign context/);
    assert.equal(state.current.session, '@1');
    assert.equal(state.current.snapshot, '@2');
    assert.equal(state.receipt, receipt);
    assert.deepEqual(state.retiring, []);
  }
});

test('coherent context observation decodes its own value offset and delivery variants', () => {
  const name = new TextEncoder().encode('editing.context');
  const data = new Uint8Array(16 + name.length + 8), view = new DataView(data.buffer);
  view.setBigUint64(0, 9007199254740993n, true);
  view.setUint32(8, 1, true); view.setUint32(12, name.length, true);
  data.set(name, 16); data.set(u64(0n), 16 + name.length);
  assert.deepEqual(watching(`ok\tkernel.session.observe\t1\t${hex(data)}\t@1,@2,@3,@4`),
    { watch: '@3', revision: '9007199254740993',
      observation: { session: '@1', snapshot: '@2', known: ['@4'], active: '@4' } });
  const follow = `ok\tkernel.session.follow\t1\t${hex(data)}\t@1,@2,@3,@4`;
  assert.deepEqual(followed(follow).observation,
    { session: '@1', snapshot: '@2', known: ['@4'], active: '@4' });
  assert.throws(() => watching(follow), /watch/);
  assert.throws(() => followed(follow.replace('kernel.session.follow',
    'kernel.session.observe')), /watch/);
  assert.deepEqual(change('ok\tkernel.session.next\t1\t01\t'), { kind: 'gap' });
  assert.deepEqual(change('ok\tkernel.session.next\t1\t03\t'), { kind: 'pending' });
  assert.deepEqual(change('ok\tkernel.session.next\t1\t0201\t'), { kind: 'end', lost: true });
  assert.deepEqual(change(`ok\tkernel.session.next\t1\t00${hex(u64(9n))}${hex(u64(3n))}${hex(u64(4n))}\t`),
    { kind: 'item', request: '9', before: '3', after: '4' });
  assert.throws(() => change('ok\tkernel.session.next\t1\t0202\t'));
});

test('context keeps Buffer identities in references and rejects invalid focus', () => {
  const id = '@9007199254740993';
  const fields = initial('context', { known: [id], active: id });
  assert.equal(fields[5], id);
  const empty = initial('context', { known: [] });
  assert.deepEqual(empty, ['write', 'context', 'kernel.session.create', '1',
    fields[4].slice(0, -16) + 'ffffffffffffffff']);
  const observed = observation(`ok\tediting.context\t1\t0000000000000000\t@7,@8,${id}`);
  assert.equal(observed.active, id);
  assert.deepEqual(update(observed, { known: [id] }), ['write', '@7',
    'editing.context.update', '1', 'ffffffffffffffff', `@7,@8,${id}`]);
  assert.throws(() => initial('context', { known: [id, id] }), /references/);
  assert.throws(() => initial('context', { known: [], active: id }), /not known/);
  assert.throws(() => observation('ok\tediting.context\t1\t0100000000000000\t@7,@8,@9'), /index/);
});
