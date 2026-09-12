import test from 'node:test';
import assert from 'node:assert/strict';
import { initial, observation, update, watching, change } from '../../target/browser/modules/context/api/typescript.js';
import { Client, hex, u64 } from '../../target/browser/sdk/typescript/resource.js';
import { Focus } from '../../target/browser/clients/gui/runtime/context.js';

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
