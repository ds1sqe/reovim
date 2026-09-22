// The wire and the values: every message round-trips, and fixed bytes
// from the Rust encoder decode.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { CONTAINER, Endpoint, KINDS, READ, WRITE, decode, encode } from '../../target/browser/sdk/typescript/wire.js';
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
  { kind: 'done', token: 8n, outcome: { kind: 'ok', out: { kind: 'rd', rd: 9n } } },
  { kind: 'done', token: 8n, outcome: { kind: 'err', cause: 'gone' } },
  { kind: 'changed', rd: 1n, change: 'modify' },
  { kind: 'changed', rd: 1n, change: 'attrib' },
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
  { kind: 'events', events: [{ seq: 1n, rd: 2n, kind: 'create', child: 3n }, { seq: 2n, rd: 2n, kind: 'overflow' }], cursor: 2n, serve: [] },
  { kind: 'events', events: [], cursor: 3n, serve: [[4n, { kind: 'read', h: 5n, at: 6n, len: 7 }]] },
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

// Every strict prefix of one encoded message is refused. A frame the
// transport cut short is no frame: the codec says so rather than
// reading past what it was given or taking a part for the whole.
function everyPrefixIsRefused(what, message) {
  const encoded = encode(9n, message);
  for (let len = 0; len < encoded.length; len += 1) {
    assert.equal(decode(encoded.slice(0, len)), undefined, `${what} cut at ${len} of ${encoded.length}`);
  }
}

test('a frame cut inside a nested body is refused at every length', () => {
  // The `out` shapes that carry a length of their own — a byte string,
  // a count of rds, a name and a list of apis, a list of events and a
  // list of served ops — each inside the `done` that hands it back,
  // where the length stands two bodies deep.
  const nested = outs.filter((out) => ['bytes', 'info', 'rds', 'events'].includes(out.kind));
  assert.equal(nested.length, 6);
  for (const out of nested) {
    const op = { kind: 'done', token: 18n, outcome: { kind: 'ok', out } };
    everyPrefixIsRefused(`done ${out.kind}`, { kind: 'request', op });
  }
  for (const change of KINDS) {
    const op = { kind: 'changed', rd: 21n, change };
    everyPrefixIsRefused(`changed ${change}`, { kind: 'request', op });
  }
});

test('a bit no Rights or Flags spells is refused at both ends', () => {
  // Rust reads both through `from_bits`, which takes the known bits
  // and nothing else, and answers nothing at all to a frame it cannot
  // decode; a frame this side wrote with an unknown bit in it would
  // therefore go out and never be answered.
  const open = (rights) => ({ kind: 'request', op: { kind: 'open', rd: 7n, rights } });
  assert.throws(() => encode(0n, open(READ | WRITE | CONTAINER)), RangeError);
  assert.throws(() => encode(0n, open(1 << 31)), RangeError);
  assert.throws(() => encode(0n, open(-1)), RangeError);
  // The same bytes read back: `rights` stands as a u32 at offset 18,
  // after the sequence, the message tag, the op tag and the rd.
  const bytes = encode(0n, open(READ | WRITE));
  assert.deepEqual(decode(bytes)?.[1], open(READ | WRITE));
  const unknown = Uint8Array.from(bytes);
  unknown[18] = 4;
  assert.equal(decode(unknown), undefined, 'an unknown rights bit');

  // `flags` has the third bit too, and nothing above it.
  const info = (flags) => ({ kind: 'reply', reply: { kind: 'ok', out: { kind: 'info', info: { flags, ctime: 0n, mtime: 0n, atime: 0n, apis: [] } } } });
  assert.deepEqual(decode(encode(0n, info(READ | CONTAINER)))?.[1], info(READ | CONTAINER));
  assert.throws(() => encode(0n, info(8)), RangeError);
  const flags = encode(0n, info(READ | CONTAINER));
  // `flags` is a u32 after the sequence, the message tag, the reply
  // tag, the out tag and the `name` option's own tag.
  flags[12] = 8;
  assert.equal(decode(flags), undefined, 'an unknown flags bit');
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

test('an Invalid reply to a frame that never decoded retires it all the same', () => {
  // kernel/runtime/src/connection.rs answers a frame it cannot decode
  // with `Reply::err(Invalid)` at the sequence it can still read,
  // rather than nothing: this is what an `Endpoint` does with that
  // answer, the same as any other reply.
  const endpoint = new Endpoint();
  const [seq] = endpoint.send({ kind: 'request', op: { kind: 'info', rd: 0n } });
  assert.equal(endpoint.unanswered().length, 1);
  const invalid = encode(seq, { kind: 'reply', reply: { kind: 'err', cause: 'invalid' } });
  assert.deepEqual(endpoint.receive(invalid), [seq, { kind: 'reply', reply: { kind: 'err', cause: 'invalid' } }]);
  assert.equal(endpoint.unanswered().length, 0);
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
  // The records the api names, after the methods: one `Tree`, whose
  // own field list is part of the hash the Kernel checks.
  canonical.push(1, 0, 0, 0);
  str('Tree'); canonical.push(1, 0, 0, 0); str('head'); canonical.push(4);
  const decl = parseDecl(Uint8Array.from(canonical));
  assert.equal(decl.api, 'history');
  assert.equal(decl.methods[0].name, 'tree');
  assert.equal(decl.records.length, 1);
  assert.equal(decl.records[0][0], 'Tree');
  assert.deepEqual(decl.records[0][1].map((f) => f.name), ['head']);
  const tree = [1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 111, 112, 101, 110,
    1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
  const value = decodeValue(decl.methods[0].output, Uint8Array.from(tree));
  assert.equal(show(value), '{nodes=[{id=1 parent=0 label=open}] head=1 published=1}');
  assert.equal(field(value, 'head')?.value, 1n);
  assert.equal(decodeValue(decl.methods[0].output, Uint8Array.from(tree.slice(1))), undefined);
});
