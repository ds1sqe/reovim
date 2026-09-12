import assert from 'node:assert/strict';
import test from 'node:test';
import { input } from '../../target/browser/apps/web/input.js';
import { apply } from '../../target/browser/clients/gui/contract/input.js';

class Field extends EventTarget {
  value = ''; selectionStart = 0; selectionEnd = 0; readOnly = false;
  emit(type, properties = {}) {
    this.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), properties));
  }
  insert(text) {
    this.selectionStart = this.value.length; this.selectionEnd = this.value.length;
    this.emit('beforeinput', { inputType: 'insertText' });
    this.value += text;
    this.selectionStart = this.value.length; this.selectionEnd = this.value.length;
    this.emit('input');
  }
}
const window = value => ({ start: '0', end: [...value].length.toString(), eof: true,
  units: [...value].map(scalar => ({ scalar })) });
const tick = () => new Promise(setImmediate);

test('typing continues while acknowledgments verify each queued semantic basis', async () => {
  const field = new Field(), calls = [], replies = [];
  let saved;
  const adapter = input(field, (...args) => {
    calls.push(args);
    return new Promise(resolve => replies.push(resolve));
  }, async () => {}, error => { throw error; }, {
    load: () => undefined, save: value => { saved = structuredClone(value); }
  });
  adapter.update(window('a'));
  field.insert('🦀'); field.insert('b');
  assert.equal(field.readOnly, false);
  assert.equal(field.value, 'a🦀b');
  assert.deepEqual(calls, [[1n, 1n, '🦀']]);
  assert.deepEqual(saved.queue.map(edit => edit.text), ['🦀', 'b']);
  replies.shift()(window('a🦀')); await tick();
  assert.deepEqual(calls, [[1n, 1n, '🦀'], [2n, 2n, 'b']]);
  assert.equal(field.value, 'a🦀b');
  replies.shift()(window('a🦀b')); await tick();
  assert.equal(saved, undefined);
  assert.equal(field.readOnly, false);
});

test('unexpected accepted view fences the queue and preserves remaining input', async () => {
  const field = new Field(), calls = [];
  let reply, failure, saved;
  const adapter = input(field, (...args) => {
    calls.push(args); return new Promise(resolve => { reply = resolve; });
  }, async () => {}, error => { failure = error; }, {
    load: () => undefined, save: value => { saved = structuredClone(value); }
  });
  adapter.update(window('a')); field.insert('b'); field.insert('c');
  reply(window('peer')); await tick();
  assert.match(failure.message, /differs/);
  assert.deepEqual(calls, [[1n, 1n, 'b']]);
  assert.equal(saved.value, 'abc');
  assert.equal(field.value, 'abc');
  assert.equal(field.readOnly, true);
  assert.deepEqual(saved.queue.map(edit => edit.text), ['b', 'c']);
});

test('an acknowledgment cannot overwrite a newer composition draft', async () => {
  const field = new Field(), calls = [], replies = [];
  let saved;
  const adapter = input(field, (...args) => {
    calls.push(args); return new Promise(resolve => replies.push(resolve));
  }, async () => {}, error => { throw error; }, {
    load: () => undefined, save: value => { saved = structuredClone(value); }
  });
  adapter.update(window('a')); field.insert('b');
  field.emit('compositionstart');
  field.value = 'ab漢'; field.emit('input');
  replies.shift()(window('ab')); await tick();
  assert.equal(field.value, 'ab漢');
  assert.equal(saved.phase, 'composition');
  assert.deepEqual(calls, [[1n, 1n, 'b']]);
  field.emit('compositionend'); field.emit('input'); await tick();
  assert.deepEqual(calls, [[1n, 1n, 'b'], [2n, 2n, '漢']]);
  replies.shift()(window('ab漢')); await tick();
  assert.equal(field.value, 'ab漢');
  assert.equal(saved, undefined);
});

test('local prediction preserves opaque bytes and enforces the window bound', () => {
  const source = { start: '8', end: '10', eof: true,
    units: [{ opaque: 255 }, { scalar: '\n' }] };
  assert.deepEqual(apply(source, { start: 9n, end: 9n, text: '🦀' }),
    { start: '8', end: '11', eof: true,
      units: [{ opaque: 255 }, { scalar: '🦀' }, { scalar: '\n' }] });
  assert.throws(() => apply(window('a'.repeat(1024)),
    { start: 0n, end: 0n, text: 'b' }), /capacity/);
});

test('queue pressure retains overflow text without sending subsequent edits', async () => {
  const field = new Field(), calls = [];
  let reply, failure, saved;
  const adapter = input(field, (...args) => {
    calls.push(args); return new Promise(resolve => { reply = resolve; });
  }, async () => {}, error => { failure = error; }, {
    load: () => undefined, save: value => { saved = structuredClone(value); }
  });
  adapter.update(window(''));
  for (let index = 0; index < 33; index++) field.insert(String(index));
  assert.match(failure.message, /queue capacity/);
  assert.equal(saved.value, Array.from({ length: 33 }, (_, index) => String(index)).join(''));
  assert.deepEqual(saved.queue.map(edit => edit.text),
    Array.from({ length: 32 }, (_, index) => String(index)));
  reply(window('0')); await tick();
  assert.deepEqual(calls, [[0n, 0n, '0']]);
  assert.equal(field.readOnly, true);
});
