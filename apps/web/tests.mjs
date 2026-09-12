import assert from 'node:assert/strict';
import test from 'node:test';
import net from 'node:net';
import http from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { Client, DeliveryError, hex, message, opening, outcome, resource,
  startupFailure, u64 }
  from '../../target/browser/sdk/typescript/resource.js';
import * as text from '../../target/browser/modules/text/api/typescript.js';
import { project, replacement } from '../../target/browser/clients/gui/contract/input.js';
import { input as bindInput } from '../../target/browser/apps/web/input.js';

test('composition submits once at completion and failed input keeps its draft', async () => {
  class Field extends EventTarget {
    value = ''; selectionStart = 2; selectionEnd = 3; readOnly = false;
    setSelectionRange(start, end) { this.selectionStart = start; this.selectionEnd = end; }
    emit(type, properties = {}) {
      this.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), properties));
    }
  }
  const field = new Field(), requests = [];
  const window = { start: '0', end: '3', eof: true,
    units: [...'# a'].map(scalar => ({ scalar })) };
  const adapter = bindInput(field, async (...request) => { requests.push(request); },
    async () => {}, error => { throw error; });
  adapter.update(window);
  field.emit('compositionstart');
  field.value = '# h'; field.emit('input');
  field.value = '# 漢'; field.emit('input');
  assert.deepEqual(requests, []);
  field.emit('compositionend'); field.emit('input');
  await new Promise(setImmediate);
  assert.deepEqual(requests, [[2n, 3n, '漢']]);
  adapter.update(window);
  field.setSelectionRange(2, 3);
  field.emit('compositionstart');
  field.value = '# temporary'; field.emit('input');
  field.value = '# a'; field.emit('compositionend'); field.emit('input');
  await new Promise(setImmediate);
  assert.deepEqual(requests, [[2n, 3n, '漢']]);
  const failed = new Field();
  let error;
  const failing = bindInput(failed, async () => { throw new Error('stale'); },
    async () => {}, value => { error = value; failing.update(window); });
  failing.update(window);
  failed.emit('beforeinput', { inputType: 'insertText' });
  failed.value = '# draft'; failed.emit('input');
  await new Promise(setImmediate);
  assert.equal(error.message, 'stale');
  assert.equal(failed.value, '# draft');
  assert.equal(failed.readOnly, true);
});

test('browser input maps UTF-16, CRLF and opaque units with selection anchors', () => {
  const scalar = value => [...value].map(scalar => ({ scalar }));
  const window = { start: '10', end: '18', eof: true,
    units: [...scalar('🦀\r\n'), { opaque: 255 }, ...scalar('tail')] };
  const view = project(window);
  assert.equal(view.value, '🦀\n\ufffctail');
  assert.deepEqual(replacement(view, '🦀!\n\ufffctail',
    { start: 2, end: 2, backward: false }), { start: 11n, end: 11n, text: '!' });
  assert.throws(() => replacement(view, '', { start: 1, end: 1, backward: false }), /scalar/);
  const breaks = project({ start: '0', end: '3', eof: true, units: scalar('\r\n\n') });
  assert.deepEqual(replacement(breaks, '\n', { start: 0, end: 0, backward: false }),
    { start: 0n, end: 2n, text: '' });
  assert.deepEqual(replacement(breaks, '\n', { start: 2, end: 2, backward: true }),
    { start: 2n, end: 3n, text: '' });
  assert.deepEqual(replacement(view, '🦀\nXtail', { start: 3, end: 4, backward: false }),
    { start: 13n, end: 14n, text: 'X' });
});

test('Text delivery preserves observation identity and exact event values', () => {
  const observed = { access: '@5', buffer: '@6', history: '@7', watch: '@8' };
  const frame = bytes => `ok\ttext.next\t1\t${hex(bytes)}\t`;
  assert.deepEqual(text.change(frame(new Uint8Array([1])), observed),
    { ...observed, delivery: { kind: 'gap' } });
  assert.equal(text.change(frame(new Uint8Array([3])), observed).delivery.kind, 'pending');
  assert.deepEqual(text.change(frame(new Uint8Array([2, 1])), observed).delivery,
    { kind: 'end', lost: true });
  const data = new Uint8Array(42);
  const view = new DataView(data.buffer);
  data[1] = 1;
  for (const [offset, value] of [[2, 9007199254740993n], [10, 4n], [18, 5n], [26, 3n], [34, 2n]]) {
    view.setBigUint64(offset, value, true);
  }
  assert.deepEqual(text.change(frame(data), observed), { ...observed,
    delivery: { kind: 'item', transition: 'undo', request: '9007199254740993',
      before: '4', after: '5', fromNode: '3', toNode: '2' } });
  assert.throws(() => text.change(frame(new Uint8Array([2, 2])), observed));
});

test('Text replacement preserves scalar ranges and rejects lone surrogates', () => {
  const observed = { access: '@5', snapshot: '@6' };
  assert.deepEqual(text.replace(observed, 9007199254740993n,
    9007199254740994n, '🦀'), ['write', '@5', 'text.replace', '1',
    hex(u64(9007199254740993n)) + hex(u64(9007199254740994n)) + 'f09fa680', '@6']);
  assert.throws(() => text.replace(observed, 0n, 1n, '\ud800'), /Unicode/);
  assert.throws(() => text.range(3n, 2n), /reversed/);
  const data = new Uint8Array(27);
  const view = new DataView(data.buffer);
  data[0] = 1; view.setBigUint64(9, 2n, true);
  view.setUint32(18, 0x1f980, true);
  data[22] = 1; view.setUint32(23, 255, true);
  assert.deepEqual(text.window(`ok\ttext.window\t1\t${hex(data)}\t`, 0n, 2n),
    { start: '0', end: '2', eof: true, units: [{ scalar: '🦀' }, { opaque: 255 }] });
  view.setUint32(18, 0xd800, true);
  assert.throws(() => text.window(`ok\ttext.window\t1\t${hex(data)}\t`, 0n, 2n), /unit/);
});

test('failed startup cleanup retains its owner across uncertain teardown', async () => {
  const token = 'b'.repeat(32);
  const start = { key: 'c'.repeat(32),
    response: `error\tboot\t${token}\t1\tfailed\t8\t@7\t` };
  const failure = startupFailure(start);
  assert.equal(failure.result.resource, '@7');
  const frames = [];
  failure.client.wire = async fields => {
    frames.push(fields);
    if (frames.length === 2) throw new Error('disconnect reply lost');
    return 'ok';
  };
  let saved;
  const persist = () => { saved = structuredClone(start); };
  await assert.rejects(failure.cleanup(persist), /disconnect reply lost/);
  assert.equal(saved.released, true);
  const recovered = startupFailure(saved);
  recovered.client.wire = failure.client.wire;
  await recovered.cleanup(() => { assert.fail('receipt already retired'); });
  assert.deepEqual(frames, [[token, 'release', '1'], [token, 'disconnect'],
    [token, 'disconnect']]);
  const unknown = startupFailure({ ...start,
    response: `error\tboot\t${token}\t1\tunknown\t8\t\t776f726b` });
  unknown.client.wire = async () => { assert.fail('unknown must retain ownership'); };
  await assert.rejects(unknown.cleanup(() => {}), /reconcile/);
  assert.equal(unknown.result.recovery, 'work');
});

test('opening uncertainty recovers before startup receipt retirement', async t => {
  const original = Client.prototype.wire;
  t.after(() => { Client.prototype.wire = original; });
  const start = opening();
  let saved;
  const persist = () => { saved = structuredClone(start); };
  const frames = [];
  const response = `ok\t${'a'.repeat(32)}\t1`;
  Client.prototype.wire = async fields => {
    frames.push(fields);
    assert.equal(saved.sent, true);
    if (fields[0] === 'open') throw new Error('reply lost');
    if (fields[0] === 'recover') return response;
    assert.equal(saved.response, response);
    if (frames.length === 3) throw new Error('release reply lost');
    return 'ok';
  };
  await assert.rejects(Client.connect(start, persist), /reply lost/);
  await assert.rejects(Client.connect(start, persist), /release reply lost/);
  const client = await Client.connect(start, persist);
  assert.equal(client.token, 'a'.repeat(32));
  assert.deepEqual(frames, [['open', start.key], ['recover', start.key],
    [client.token, 'release', '1'], [client.token, 'release', '1']]);
});

test('identities and currentness remain exact beyond Number precision', () => {
  assert.equal(resource('@18446744073709551615'), '@18446744073709551615');
  assert.equal(hex(u64(18446744073709551615n)), 'ffffffffffffffff');
  assert.equal(message('ok\tvalue\t1\t00\t@9007199254740993').resources[0], '@9007199254740993');
  assert.throws(() => resource('@18446744073709551616'));
  assert.throws(() => u64(-1n));
  const failed = outcome('error\tunknown\t9007199254740993\t@17\t776f726b');
  assert.equal(failed.code, 9007199254740993n);
  assert.equal(failed.recovery, 'work');
});

test('lost delivery retains exact receipt; adopted results do not resend', async () => {
  const client = new Client('client');
  const calls = [];
  client.wire = async fields => {
    calls.push(fields);
    if (fields[1] === 'reserve') return 'ok\t9007199254740993';
    throw new Error('connection lost');
  };
  const receipt = await client.reserve(['write', '@7', 'edit', '1', 'ff']);
  await assert.rejects(client.send(receipt), error =>
    error instanceof DeliveryError && error.receipt === receipt);
  assert.deepEqual(calls[1], ['client', 'call', '9007199254740993', 'write', '@7', 'edit', '1', 'ff']);
  receipt.response = 'ok\tvalue\t1\tff\t';
  assert.equal(await client.send(receipt), receipt.response);
  assert.deepEqual(calls, [['client', 'reserve'], ['client', 'call',
    '9007199254740993', 'write', '@7', 'edit', '1', 'ff']]);
});

test('HTTP bridge preserves frames and rejects cross-origin forwarding', { timeout: 10000 }, async t => {
  const frames = [];
  const backend = net.createServer(socket => {
    socket.once('data', data => {
      frames.push(data.toString());
      if (data.includes(Buffer.from('park'))) backend.emit('parked');
      else if (data.includes(Buffer.from('lost'))) socket.end();
      else socket.end('pending\nok\t9007199254740993\n');
    });
  });
  backend.listen(0, '127.0.0.1'); await once(backend, 'listening');
  t.after(() => backend.close());
  const child = spawn(process.execPath, ['apps/web/bridge.mjs',
    `127.0.0.1:${backend.address().port}`, '0'], { stdio: ['ignore', 'pipe', 'pipe'] });
  t.after(() => child.kill());
  const [data] = await once(child.stdout, 'data');
  const origin = data.toString().trim().replace('browser ', '');
  const send = (fields, extra = {}) => fetch(`${origin}/rpc`, {
    method: 'POST', headers: { Origin: origin, 'X-Reovim': '1', ...extra },
    body: JSON.stringify(fields)
  });
  assert.equal((await send(['open'], { Origin: 'https://foreign.invalid' })).status, 403);
  assert.deepEqual(frames, []);
  assert.equal(await (await send(['한글'])).text(), 'ok\t9007199254740993');
  assert.deepEqual(frames, ['v1\t한글\n']);
  assert.equal((await send(['bad\nframe'])).status, 400);
  assert.equal((await send(['lost'])).status, 502);
  assert.deepEqual(frames, ['v1\t한글\n', 'v1\tlost\n']);
  const body = Buffer.from(JSON.stringify(['한글']));
  await new Promise((resolve, reject) => {
    const request = http.request(`${origin}/rpc`, { method: 'POST',
      headers: { Origin: origin, 'X-Reovim': '1' } }, response => {
      response.resume(); response.on('end', resolve);
    });
    request.on('error', reject);
    request.write(body.subarray(0, 3));
    setImmediate(() => request.end(body.subarray(3)));
  });
  assert.equal(frames.at(-1), 'v1\t한글\n');
  for (let index = 0; index < 20; index++) {
    const controller = new AbortController();
    const parked = once(backend, 'parked');
    const waiting = fetch(`${origin}/rpc`, { method: 'POST',
      headers: { Origin: origin, 'X-Reovim': '1' },
      body: JSON.stringify(['park']), signal: controller.signal });
    await parked;
    controller.abort();
    await assert.rejects(waiting);
  }
  assert.equal(await (await send(['open'])).text(), 'ok\t9007199254740993');
});
