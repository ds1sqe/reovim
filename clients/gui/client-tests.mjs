// The Client against a real reovim server over its Unix socket:
// walkthrough 1 from Node, and a cut link taken up again.

import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { decodeValue, field, show } from '../../target/browser/sdk/typescript/value.js';
import {
  Client, Fault, READ, WRITE, edit, rdInput, rdOutput,
} from '../../target/browser/clients/gui/client.js';
import { Socket } from './socket.mjs';

const root = new URL('../../', import.meta.url).pathname;
const bin = join(root, 'target/debug/reovim');

/** A server over a directory of its own; `stop` ends it. */
async function server() {
  // Always, a build that is fresh being nothing: a binary left from
  // before the wire moved speaks another wire.
  execFileSync('cargo', ['build', '-p', 'reovim'], { cwd: root, stdio: 'inherit' });
  const dir = mkdtempSync(join(tmpdir(), 'reovim-gui-'));
  const socket = join(dir, 's');
  writeFileSync(join(dir, 'note'), 'first\n');
  const child = spawn(bin, ['server', '--socket', socket, '--root', dir], { stdio: 'ignore' });
  const carrier = new Socket(socket);
  for (let tries = 0; ; tries += 1) {
    try {
      await carrier.connect();
      break;
    } catch (error) {
      if (tries > 200) throw error;
      await new Promise((r) => setTimeout(r, 10));
    }
  }
  return {
    dir, socket, carrier,
    disk: (name) => readFileSync(join(dir, name), 'utf8'),
    stop() { child.kill(); rmSync(dir, { recursive: true, force: true }); },
  };
}

const text = (s) => new TextEncoder().encode(s);
const utf8 = (b) => new TextDecoder().decode(b);

test('walkthrough 1 from Node through a real server', async () => {
  const s = await server();
  try {
    const client = await Client.connect(s.carrier, text('web'));
    await client.capability(text('web'));
    const file = await client.resolve('/module/fs/root/note');
    const open = await client.resolve('/module/text/open');
    const [, made] = await client.callByName(open, 'open', rdInput(file));
    const buffer = rdOutput(made);
    const h = await client.open(buffer, READ | WRITE);
    await client.watch(h, true);
    await client.since();
    assert.equal(utf8(await client.readWhole(h)), 'first\n');

    await client.callByName(buffer, 'replace', edit(0, 0, 'x'));
    assert.equal(utf8(await client.readWhole(h)), 'xfirst\n');
    const events = await client.since();
    assert.ok(events.some((e) => e.rd === buffer && e.kind === 'modify'));
    const [method, bytes] = await client.callByName(buffer, 'tree', new Uint8Array());
    const tree = decodeValue(method.output, bytes);
    assert.equal(field(tree, 'head')?.value, 2n);
    assert.equal(show(tree), '{nodes=[{id=1 parent=0 label=open} {id=2 parent=1 label=replace}] head=2 published=1}');

    await client.callByName(buffer, 'undo', new Uint8Array());
    assert.equal(utf8(await client.readWhole(h)), 'first\n');
    await client.callByName(buffer, 'redo', new Uint8Array());
    assert.equal(s.disk('note'), 'first\n', 'disk waits for save');
    await client.callByName(buffer, 'save', new Uint8Array());
    assert.equal(s.disk('note'), 'xfirst\n');
    await assert.rejects(client.callByName(buffer, 'nothing', new Uint8Array()), (e) => e instanceof Fault && e.kind === 'usage');
    await assert.rejects(client.resolve('/module/none'), (e) => e instanceof Fault && e.cause === 'invalid');

    // The link is cut; the named Client goes on with its handle.
    s.carrier.end();
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(utf8(await client.readWhole(h)), 'xfirst\n');
    await client.close(h);
  } finally {
    s.stop();
  }
});
