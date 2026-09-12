import assert from 'node:assert/strict';
import test from 'node:test';
import { Commands } from '../../target/browser/clients/gui/runtime/commands.js';
import { editor } from '../../target/browser/clients/gui/runtime/editor.js';
import { activate } from '../../target/browser/clients/gui/modules/uppercase/index.js';
import { selectedRange } from '../../target/browser/clients/gui/contract/input.js';

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
