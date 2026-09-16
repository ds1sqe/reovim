// The bridge between a WebSocket and a real reovim server, driven with
// Node's own WebSocket client; the page is served.

import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { decode, encode } from '../../target/browser/sdk/typescript/wire.js';
import { serve } from './bridge.mjs';

const root = new URL('../../', import.meta.url).pathname;
const bin = join(root, 'target/debug/reovim');

test('a WebSocket reaches the server through the bridge', async () => {
  if (!existsSync(bin)) execFileSync('cargo', ['build', '-p', 'reovim'], { cwd: root, stdio: 'inherit' });
  const dir = mkdtempSync(join(tmpdir(), 'reovim-web-'));
  const socket = join(dir, 's');
  writeFileSync(join(dir, 'note'), 'first\n');
  const child = spawn(bin, ['server', '--socket', socket, '--root', dir], { stdio: 'ignore' });
  for (let tries = 0; !existsSync(socket) && tries < 300; tries += 1) await new Promise((r) => setTimeout(r, 10));
  const server = serve({ socket, port: 0 });
  await new Promise((r) => server.on('listening', r));
  const port = server.address().port;
  try {
    const html = await (await fetch(`http://127.0.0.1:${port}/?file=note`)).text();
    assert.ok(html.includes('/js/apps/web/edit.js'));
    assert.equal((await fetch(`http://127.0.0.1:${port}/js/../x`)).status, 404);

    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    ws.binaryType = 'arraybuffer';
    await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
    const ask = (seq, message) => new Promise((resolve) => {
      ws.onmessage = (event) => resolve(decode(new Uint8Array(event.data)));
      ws.send(encode(seq, message));
    });
    const connected = await ask(1n, { kind: 'connect', credential: new Uint8Array() });
    assert.equal(connected[1].reply.out.kind, 'rds');
    const lookup = await ask(2n, { kind: 'request', op: { kind: 'lookup', rd: 0n, name: 'module' } });
    assert.deepEqual(lookup, [2n, { kind: 'reply', reply: { kind: 'ok', out: { kind: 'rd', rd: 4n } } }]);
    ws.close();
  } finally {
    server.close();
    child.kill();
    rmSync(dir, { recursive: true, force: true });
  }
});
