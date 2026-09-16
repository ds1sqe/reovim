// The wire and the values: every message round-trips, and fixed bytes
// from the Rust encoder decode.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { Endpoint, decode, encode } from '../../target/browser/sdk/typescript/wire.js';
import { decodeValue, field, parseDecl, show } from '../../target/browser/sdk/typescript/value.js';

const bytes = (...b) => Uint8Array.from(b);

function roundTrip(seq, message) {
  const encoded = encode(seq, message);
  assert.deepEqual(decode(encoded), [seq, message]);
}

const ops = [
  { kind: 'lookup', rd: 0n, name: 'sys' },
  { kind: 'open', rd: 7n, rights: 3 },
  { kind: 'read', h: 1n, at: 2n, len: 3 },
  { kind: 'write', h: 1n, at: 0n, bytes: bytes(1, 2) },
  { kind: 'call', h: 1n, api: 0xfeedn, method: 2, input: bytes() },
  { kind: 'watch', h: 1n, on: true },
  { kind: 'close', h: 1n },
  { kind: 'ask', token: 9n },
  { kind: 'cancel', token: 9n },
  { kind: 'since', cursor: 4n },
  { kind: 'info', rd: 1n },
  { kind: 'parent', rd: 1n },
  { kind: 'children', rd: 1n },
];

const outs = [
  { kind: 'unit' },
  { kind: 'rd', rd: 5n },
  { kind: 'handle', h: 6n },
  { kind: 'len', len: 7 },
  { kind: 'bytes', bytes: bytes(9) },
  { kind: 'info', info: { name: 'x', flags: 5, ctime: 1n, mtime: 2n, atime: 3n, apis: [4n] } },
  { kind: 'info', info: { flags: 0, ctime: 0n, mtime: 0n, atime: 0n, apis: [] } },
  { kind: 'rds', rds: [1n, 2n] },
  { kind: 'events', events: [{ seq: 1n, rd: 2n, kind: 'create', child: 3n }, { seq: 2n, rd: 2n, kind: 'overflow' }], cursor: 2n },
];

test('every message round-trips', () => {
  roundTrip(0n, { kind: 'connect', credential: bytes(109, 101) });
  roundTrip(1n, { kind: 'connect', credential: bytes(), session: 5n });
  ops.forEach((op, i) => roundTrip(BigInt(i), { kind: 'request', op }));
  outs.forEach((out, i) => roundTrip(BigInt(i), { kind: 'reply', reply: { kind: 'ok', out } }));
  for (const cause of ['denied', 'unsupported', 'invalid', 'stale', 'busy', 'nospace', 'closed', 'gone', 'cancelled']) {
    roundTrip(2n ** 64n - 1n, { kind: 'reply', reply: { kind: 'err', cause } });
  }
  roundTrip(3n, { kind: 'reply', reply: { kind: 'pending', token: 4n } });
  roundTrip(5n, { kind: 'load', path: '/x/hello.rvm' });
  roundTrip(6n, { kind: 'unload', name: 'hello' });
});

test('the bytes are what the Rust encoder writes', () => {
  const connect = bytes(1, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 109, 101, 0);
  assert.deepEqual(encode(1n, { kind: 'connect', credential: bytes(109, 101) }), connect);
  const lookup = bytes(2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 115, 121, 115);
  assert.deepEqual(encode(2n, { kind: 'request', op: { kind: 'lookup', rd: 0n, name: 'sys' } }), lookup);
  const reply = bytes(2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0);
  assert.deepEqual(decode(reply), [2n, { kind: 'reply', reply: { kind: 'ok', out: { kind: 'rd', rd: 1n } } }]);
  assert.equal(decode(bytes(1, 2, 3)), undefined);
  assert.equal(decode(bytes(...connect, 0)), undefined, 'trailing byte');
});

test('the endpoint knows what went unanswered', () => {
  const endpoint = new Endpoint();
  const [seq, sent] = endpoint.send({ kind: 'request', op: { kind: 'info', rd: 0n } });
  assert.equal(seq, 1n);
  assert.equal(endpoint.unanswered().length, 1);
  const stray = encode(9n, { kind: 'reply', reply: { kind: 'ok', out: { kind: 'unit' } } });
  assert.equal(endpoint.receive(stray), undefined);
  assert.equal(endpoint.unanswered().length, 1);
  const answer = encode(1n, { kind: 'reply', reply: { kind: 'ok', out: { kind: 'unit' } } });
  assert.deepEqual(endpoint.receive(answer)?.[0], 1n);
  assert.equal(endpoint.unanswered().length, 0);
  assert.equal(sent[8], 1, 'a request');
});

test('a declaration parses and a value shows by it', () => {
  // history: 0 tree() -> Tree, as modules/text declares it (one method).
  const canonical = [];
  const str = (s) => { const b = new TextEncoder().encode(s); canonical.push(b.length, 0, 0, 0, ...b); };
  str('history'); canonical.push(1, 0, 0, 0, 1, 0, 0, 0);
  canonical.push(0, 0); str('tree'); canonical.push(0);
  canonical.push(9, 3, 0, 0, 0);
  str('nodes'); canonical.push(10, 0, 16, 0, 0, 9, 3, 0, 0, 0);
  str('id'); canonical.push(4); str('parent'); canonical.push(4); str('label'); canonical.push(8, 16, 0, 0, 0);
  str('head'); canonical.push(4); str('published'); canonical.push(4);
  canonical.push(1, 0, 0, 0);
  const decl = parseDecl(Uint8Array.from(canonical));
  assert.equal(decl.api, 'history');
  assert.equal(decl.methods[0].name, 'tree');
  const tree = [1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 111, 112, 101, 110,
    1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
  const value = decodeValue(decl.methods[0].output, Uint8Array.from(tree));
  assert.equal(show(value), '{nodes=[{id=1 parent=0 label=open}] head=1 published=1}');
  assert.equal(field(value, 'head')?.value, 1n);
  assert.equal(decodeValue(decl.methods[0].output, Uint8Array.from(tree.slice(1))), undefined);
});
