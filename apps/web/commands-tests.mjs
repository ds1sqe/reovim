import assert from 'node:assert/strict';
import test from 'node:test';
import { Commands } from '../../target/browser/clients/gui/runtime/commands.js';
import { editor } from '../../target/browser/clients/gui/runtime/editor.js';
import { activate } from '../../target/browser/clients/gui/modules/uppercase/index.js';
import { selectedRange } from '../../target/browser/clients/gui/contract/input.js';
import { Keymaps } from '../../target/browser/clients/gui/core/keymaps.js';

test('key resolution ranks scope then priority and rejects equal active matches', async () => {
  const maps = new Keymaps(), commands = new Commands(), calls = [];
  for (const name of ['default', 'text', 'other']) commands.register(name, async () => { calls.push(name); });
  const bind = (command, scope, priority) => maps.register({
    sequence: [{ key: 'x', ctrl: true }], command, scope, priority
  }, commands.target(command));
  bind('default', '', 100);
  const scoped = bind('text', 'text', 0);
  assert.throws(() => bind('other', 'text', 0), /conflict/);
  let result = maps.press({ key: 'x', ctrl: true }, ['text'], 0);
  assert.equal(result.kind, 'command'); await result.target.invoke();
  assert.deepEqual(calls, ['text']);
  const equal = bind('other', 'other', 0);
  assert.equal(maps.press({ key: 'x', ctrl: true }, ['other', 'text'], 1).kind, 'ambiguous');
  equal.close(); scoped.close();
  result = maps.press({ key: 'x', ctrl: true }, ['text'], 2);
  await result.target.invoke(); assert.deepEqual(calls, ['text', 'default']);
});

test('chords expire, retry only the mismatching key and fence context and removal', async () => {
  const maps = new Keymaps(), commands = new Commands(), calls = [];
  for (const name of ['short', 'long', 'fresh']) commands.register(name, async () => { calls.push(name); });
  const bind = (command, sequence) => maps.register({
    sequence: sequence.map(key => ({ key })), command, scope: 'text', priority: 0
  }, commands.target(command));
  bind('short', ['k']); const long = bind('long', ['k', 'u']); bind('fresh', ['x']);
  assert.deepEqual(maps.press({ key: 'k' }, ['text'], 0), { kind: 'pending', deadline: 750 });
  assert.equal(maps.expire(['text'], 749).kind, 'pending');
  let result = maps.expire(['text'], 750); await result.target.invoke();
  assert.deepEqual(calls, ['short']);
  maps.press({ key: 'k' }, ['text'], 800);
  result = maps.press({ key: 'u' }, ['text'], 801); await result.target.invoke();
  maps.press({ key: 'k' }, ['text'], 900);
  result = maps.press({ key: 'x' }, ['text'], 901); await result.target.invoke();
  assert.deepEqual(calls, ['short', 'long', 'fresh']);
  maps.press({ key: 'k' }, ['text'], 1000);
  assert.equal(maps.expire(['other'], 1750).kind, 'unhandled');
  maps.press({ key: 'k' }, ['text'], 1800); long.close();
  assert.equal(maps.expire(['text'], 2600).kind, 'unhandled');
  assert.throws(() => maps.expire(['text'], 2599), /monotonic/);
});

test('retired commands cannot be reached through old bindings after name reuse', async () => {
  const maps = new Keymaps(), commands = new Commands();
  const old = commands.register('same', async () => { assert.fail('retired callback'); });
  maps.register({ sequence: [{ key: 'k' }, { key: 'u' }], scope: '',
    priority: 0, command: 'same' }, commands.target('same'));
  maps.press({ key: 'k' }, [], 0); old.close();
  commands.register('same', async () => { assert.fail('retargeted callback'); });
  assert.equal(maps.press({ key: 'u' }, [], 1).kind, 'unhandled');
  assert.equal(maps.press({ key: 'k' }, [], 2).kind, 'unhandled');
});

test('prefix-only expires and named chord bounds reject without publishing', () => {
  const maps = new Keymaps(), target = { available: () => true, invoke: async () => {} };
  const binding = { sequence: [{ key: 'k' }, { key: 'u' }], scope: '', priority: 0, command: 'test' };
  maps.register(binding, target);
  assert.equal(maps.press({ key: 'k' }, [], 0).kind, 'pending');
  assert.equal(maps.expire([], 750).kind, 'unhandled');
  assert.throws(() => maps.register({ ...binding, sequence: Array(5).fill({ key: 'x' }) }, target), /invalid/);
  assert.throws(() => maps.register({ ...binding, priority: NaN }, target), /invalid/);
});

test('browser selections preserve Text endpoints across UTF-16, CRLF and opaque units', () => {
  const window = { start: '9007199254740993', end: '9007199254740998', eof: true,
    units: [{ scalar: '🦀' }, { scalar: '\r' }, { scalar: '\n' }, { opaque: 255 }, { scalar: 'a' }] };
  const displayed = '🦀\n\ufffca';
  assert.deepEqual(selectedRange(window, displayed, 2, 4),
    { start: 9007199254740994n, end: 9007199254740997n });
  assert.deepEqual(selectedRange(window, displayed, 0, 2),
    { start: 9007199254740993n, end: 9007199254740994n });
  assert.throws(() => selectedRange(window, displayed, 1, 2), /scalar/);
  assert.throws(() => selectedRange(window, displayed, 4, 2), /reversed/);
  assert.throws(() => selectedRange(window, 'unobserved text', 0, 2), /differs/);
});

test('unload fences new calls and keeps the active registration until settlement', async () => {
  const commands = new Commands();
  let settle, result;
  const old = commands.register('test.command', () => new Promise(resolve => { settle = resolve; }));
  const running = commands.invoke('test.command');
  assert.throws(() => old.close(), /busy/);
  await assert.rejects(commands.invoke('test.command'), /unavailable/);
  assert.throws(() => commands.register('test.command', async () => {}), /already registered/);
  settle(); await running; old.close(); old.close();
  const current = commands.register('test.command', async () => { result = 'new'; });
  old.close(); await commands.invoke('test.command');
  assert.equal(result, 'new'); current.close();
  const failed = commands.register('test.failure', async () => { throw new Error('command failed'); });
  await assert.rejects(commands.invoke('test.failure'), /command failed/);
  failed.close();
});

test('author contribution edits Unicode through a qualified public capability', async () => {
  const units = [...'straße'].map(scalar => ({ scalar }));
  let view = { snapshot: '@7', start: 0n, end: 6n,
    window: { start: '0', end: '6', eof: true, units } };
  const writes = [];
  const access = editor(() => view, async (...args) => { writes.push(args); });
  const selected = access.selection();
  selected.units[0].scalar = 'changed copy';
  assert.equal(units[0].scalar, 's');
  const commands = new Commands();
  const contribution = activate(access, commands.register);
  await commands.invoke('text.uppercase');
  assert.deepEqual(writes, [[0n, 6n, 'STRASSE']]);
  view = { ...view, start: 0n, end: 0n };
  await commands.invoke('text.uppercase');
  assert.deepEqual(writes, [[0n, 6n, 'STRASSE']]);
  view = { ...view, snapshot: '@8' };
  await assert.rejects(access.replace(selected, 'stale'), /stale/);
  view = { ...view, start: 0n, end: 1n,
    window: { start: '0', end: '1', eof: true, units: [{ opaque: 255 }] } };
  await assert.rejects(commands.invoke('text.uppercase'), /scalar-only/);
  assert.deepEqual(writes, [[0n, 6n, 'STRASSE']]);
  contribution.close();
  await assert.rejects(commands.invoke('text.uppercase'), /unavailable/);
});

test('command invocation capacity rejects excess work while existing work drains', async () => {
  const commands = new Commands(), settle = [];
  const registration = commands.register('bounded.work', () => new Promise(resolve => settle.push(resolve)));
  const pending = Array.from({ length: 32 }, () => commands.invoke('bounded.work'));
  await assert.rejects(commands.invoke('bounded.work'), /capacity/);
  assert.throws(() => registration.close(), /busy/);
  for (const finish of settle) finish();
  await Promise.all(pending);
  registration.close();
  await assert.rejects(commands.invoke('bounded.work'), /unavailable/);
});
