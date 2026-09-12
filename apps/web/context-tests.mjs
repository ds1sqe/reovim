import test from 'node:test';
import assert from 'node:assert/strict';
import { initial, observation, update } from '../../target/browser/modules/context/api/typescript.js';

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
