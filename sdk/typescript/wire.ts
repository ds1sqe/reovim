// The wire: meaning only, as kernel/interface/src/wire.rs writes it. A
// sequence and one Message; a transport moves whole messages. The
// Endpoint knows what went unanswered.

import { Reader, Writer } from './codec.js';

export type Rd = bigint;
export type Handle = bigint;
export type Token = bigint;
export type Hash = bigint;

export const READ = 1;
export const WRITE = 2;
export const CONTAINER = 4;

/** Every bit `Rights` spells: what a handle may do. */
const RIGHTS = READ | WRITE;
/** Every bit `Flags` spells: the shape a Driver reports. */
const FLAGS = READ | WRITE | CONTAINER;

/** A bit set as the Rust `from_bits` takes one: a u32 of known bits
 * and nothing else. An unknown bit is refused at both ends, a frame
 * this side would write being one the other side would not read. */
function bits(value: number, known: number, what: string): number {
  const whole = Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
  if (!whole || (value & ~known) !== 0) throw new RangeError(what);
  return value;
}

export type Op =
  | { kind: 'lookup'; rd: Rd; name: string }
  | { kind: 'open'; rd: Rd; rights: number }
  | { kind: 'read'; h: Handle; at: bigint; len: number }
  | { kind: 'write'; h: Handle; at: bigint; bytes: Uint8Array }
  | { kind: 'call'; h: Handle; api: Hash; method: number; input: Uint8Array }
  | { kind: 'watch'; h: Handle; on: boolean }
  | { kind: 'close'; h: Handle }
  | { kind: 'ask'; token: Token }
  | { kind: 'cancel'; token: Token }
  | { kind: 'since'; cursor: bigint }
  | { kind: 'info'; rd: Rd }
  | { kind: 'parent'; rd: Rd }
  | { kind: 'children'; rd: Rd }
  | { kind: 'done'; token: Token; outcome: Outcome }
  // `change` is what the Rust calls `kind`, which here names the op.
  | { kind: 'changed'; rd: Rd; change: Kind };

export const KINDS = [
  'modify', 'attrib', 'create', 'delete', 'move', 'gone', 'overflow',
] as const;
export type Kind = (typeof KINDS)[number];

export interface Event {
  seq: bigint;
  rd: Rd;
  kind: Kind;
  child?: Rd;
}

/** One op the server asks this client to serve, with the token its
 * answering `done` names again. A client that serves nothing gets an
 * empty list and never sends one. */
export type Served = [Token, Op];

export interface Info {
  name?: string;
  flags: number;
  ctime: bigint;
  mtime: bigint;
  atime: bigint;
  apis: Hash[];
}

export type Out =
  | { kind: 'unit' }
  | { kind: 'rd'; rd: Rd }
  | { kind: 'handle'; h: Handle }
  | { kind: 'len'; len: number }
  | { kind: 'bytes'; bytes: Uint8Array }
  | { kind: 'info'; info: Info }
  | { kind: 'rds'; rds: Rd[] }
  | { kind: 'events'; events: Event[]; cursor: bigint; serve: Served[] };

export const CAUSES = [
  'denied', 'unsupported', 'invalid', 'stale', 'busy', 'nospace', 'closed',
  'gone', 'cancelled',
] as const;
export type Cause = (typeof CAUSES)[number];

/** A reply that finished, which is what `done` carries. */
export type Outcome = { kind: 'ok'; out: Out } | { kind: 'err'; cause: Cause };

export type Reply = Outcome | { kind: 'pending'; token: Token };

export type Message =
  | { kind: 'connect'; credential: Uint8Array; session?: Rd }
  | { kind: 'request'; op: Op }
  | { kind: 'reply'; reply: Reply }
  | { kind: 'load'; path: string }
  | { kind: 'unload'; name: string };

const MESSAGES = ['connect', 'request', 'reply', 'load', 'unload'] as const;
const OPS = [
  'lookup', 'open', 'read', 'write', 'call', 'watch', 'close', 'ask',
  'cancel', 'since', 'info', 'parent', 'children', 'done', 'changed',
] as const;
const OUTS = [
  'unit', 'rd', 'handle', 'len', 'bytes', 'info', 'rds', 'events',
] as const;
const REPLIES = ['ok', 'err', 'pending'] as const;

function tagOf<T extends string>(tags: readonly T[], tag: T): number {
  return tags.indexOf(tag);
}

function fromTag<T extends string>(tags: readonly T[], tag: number): T {
  const found = tags[tag];
  if (found === undefined) throw new RangeError(`tag ${tag}`);
  return found;
}

function putOption(w: Writer, value: bigint | undefined): void {
  if (value === undefined) {
    w.u8(0);
  } else {
    w.u8(1);
    w.u64(value);
  }
}

function getOption(r: Reader): bigint | undefined {
  const tag = r.u8();
  if (tag === 0) return undefined;
  if (tag === 1) return r.u64();
  throw new RangeError('option');
}

function putOp(w: Writer, op: Op): void {
  w.u8(tagOf(OPS, op.kind));
  switch (op.kind) {
    case 'lookup': w.u64(op.rd); w.str(op.name); break;
    case 'open':
      w.u64(op.rd); w.u32(bits(op.rights, RIGHTS, 'rights'));
      break;
    case 'read': w.u64(op.h); w.u64(op.at); w.u32(op.len); break;
    case 'write': w.u64(op.h); w.u64(op.at); w.bytes(op.bytes); break;
    case 'call':
      w.u64(op.h); w.u64(op.api); w.u16(op.method); w.bytes(op.input);
      break;
    case 'watch': w.u64(op.h); w.u8(op.on ? 1 : 0); break;
    case 'close': w.u64(op.h); break;
    case 'ask': case 'cancel': w.u64(op.token); break;
    case 'since': w.u64(op.cursor); break;
    case 'info': case 'parent': case 'children': w.u64(op.rd); break;
    case 'done': w.u64(op.token); putReply(w, op.outcome); break;
    case 'changed': w.u64(op.rd); w.u8(tagOf(KINDS, op.change)); break;
  }
}

function getOp(r: Reader): Op {
  const kind = fromTag(OPS, r.u8());
  switch (kind) {
    case 'lookup': return { kind, rd: r.u64(), name: r.str() };
    case 'open':
      return { kind, rd: r.u64(), rights: bits(r.u32(), RIGHTS, 'rights') };
    case 'read': return { kind, h: r.u64(), at: r.u64(), len: r.u32() };
    case 'write': return { kind, h: r.u64(), at: r.u64(), bytes: r.bytes() };
    case 'call':
      return { kind, h: r.u64(), api: r.u64(), method: r.u16(), input: r.bytes() };
    case 'watch': return { kind, h: r.u64(), on: r.bool() };
    case 'close': return { kind, h: r.u64() };
    case 'ask': case 'cancel': return { kind, token: r.u64() };
    case 'since': return { kind, cursor: r.u64() };
    case 'info': case 'parent': case 'children': return { kind, rd: r.u64() };
    case 'done': return { kind, token: r.u64(), outcome: getOutcome(r) };
    case 'changed': return { kind, rd: r.u64(), change: fromTag(KINDS, r.u8()) };
  }
}

/** The two `ready` tags of a reply, which is what `done` carries. */
function getOutcome(r: Reader): Outcome {
  const reply = getReply(r);
  if (reply.kind === 'pending') throw new RangeError('outcome');
  return reply;
}

function putInfo(w: Writer, info: Info): void {
  if (info.name === undefined) {
    w.u8(0);
  } else {
    w.u8(1);
    w.str(info.name);
  }
  w.u32(bits(info.flags, FLAGS, 'flags'));
  w.u64(info.ctime);
  w.u64(info.mtime);
  w.u64(info.atime);
  w.u32(info.apis.length);
  for (const api of info.apis) w.u64(api);
}

function getInfo(r: Reader): Info {
  const named = r.u8();
  if (named > 1) throw new RangeError('name');
  const name = named === 1 ? r.str() : undefined;
  const flags = bits(r.u32(), FLAGS, 'flags');
  const ctime = r.u64(), mtime = r.u64(), atime = r.u64();
  const apis = items(r, () => r.u64());
  return name === undefined
    ? { flags, ctime, mtime, atime, apis }
    : { name, flags, ctime, mtime, atime, apis };
}

function items<T>(r: Reader, item: () => T): T[] {
  const n = r.u32();
  const out: T[] = [];
  for (let i = 0; i < n; i += 1) out.push(item());
  return out;
}

function putEvent(w: Writer, event: Event): void {
  w.u64(event.seq);
  w.u64(event.rd);
  w.u8(tagOf(KINDS, event.kind));
  putOption(w, event.child);
}

function getEvent(r: Reader): Event {
  const seq = r.u64(), rd = r.u64();
  const kind = fromTag(KINDS, r.u8());
  const child = getOption(r);
  return child === undefined ? { seq, rd, kind } : { seq, rd, kind, child };
}

function putOut(w: Writer, out: Out): void {
  w.u8(tagOf(OUTS, out.kind));
  switch (out.kind) {
    case 'unit': break;
    case 'rd': w.u64(out.rd); break;
    case 'handle': w.u64(out.h); break;
    case 'len': w.u32(out.len); break;
    case 'bytes': w.bytes(out.bytes); break;
    case 'info': putInfo(w, out.info); break;
    case 'rds':
      w.u32(out.rds.length);
      for (const rd of out.rds) w.u64(rd);
      break;
    case 'events':
      w.u32(out.events.length);
      for (const event of out.events) putEvent(w, event);
      w.u64(out.cursor);
      w.u32(out.serve.length);
      for (const [token, op] of out.serve) { w.u64(token); putOp(w, op); }
      break;
  }
}

function getOut(r: Reader): Out {
  const kind = fromTag(OUTS, r.u8());
  switch (kind) {
    case 'unit': return { kind };
    case 'rd': return { kind, rd: r.u64() };
    case 'handle': return { kind, h: r.u64() };
    case 'len': return { kind, len: r.u32() };
    case 'bytes': return { kind, bytes: r.bytes() };
    case 'info': return { kind, info: getInfo(r) };
    case 'rds': return { kind, rds: items(r, () => r.u64()) };
    case 'events': {
      const events = items(r, () => getEvent(r));
      const cursor = r.u64();
      const serve = items(r, (): Served => [r.u64(), getOp(r)]);
      return { kind, events, cursor, serve };
    }
  }
}

function putReply(w: Writer, reply: Reply): void {
  w.u8(tagOf(REPLIES, reply.kind));
  switch (reply.kind) {
    case 'ok': putOut(w, reply.out); break;
    case 'err': w.u8(tagOf(CAUSES, reply.cause)); break;
    case 'pending': w.u64(reply.token); break;
  }
}

function getReply(r: Reader): Reply {
  const kind = fromTag(REPLIES, r.u8());
  switch (kind) {
    case 'ok': return { kind, out: getOut(r) };
    case 'err': return { kind, cause: fromTag(CAUSES, r.u8()) };
    case 'pending': return { kind, token: r.u64() };
  }
}

/** `seq` then the message. */
export function encode(seq: bigint, message: Message): Uint8Array {
  const w = new Writer();
  w.u64(seq);
  w.u8(tagOf(MESSAGES, message.kind));
  switch (message.kind) {
    case 'connect':
      w.bytes(message.credential);
      putOption(w, message.session);
      break;
    case 'request': putOp(w, message.op); break;
    case 'reply': putReply(w, message.reply); break;
    case 'load': w.str(message.path); break;
    case 'unload': w.str(message.name); break;
  }
  return w.done();
}

/** The inverse of `encode`; undefined unless `bytes` is one message. */
export function decode(bytes: Uint8Array): [bigint, Message] | undefined {
  try {
    const r = new Reader(bytes);
    const seq = r.u64();
    const kind = fromTag(MESSAGES, r.u8());
    let message: Message;
    switch (kind) {
      case 'connect': {
        const credential = r.bytes();
        const session = getOption(r);
        message = session === undefined
          ? { kind, credential }
          : { kind, credential, session };
        break;
      }
      case 'request': message = { kind, op: getOp(r) }; break;
      case 'reply': message = { kind, reply: getReply(r) }; break;
      case 'load': message = { kind, path: r.str() }; break;
      case 'unload': message = { kind, name: r.str() }; break;
    }
    return r.done() ? [seq, message] : undefined;
  } catch {
    return undefined;
  }
}

/** The client end: numbers what it sends and remembers it until the
 * reply with that sequence arrives. */
export class Endpoint {
  private next = 1n;
  private outstanding: [bigint, Uint8Array][] = [];

  send(message: Message): [bigint, Uint8Array] {
    const seq = this.next;
    this.next += 1n;
    const bytes = encode(seq, message);
    this.outstanding.push([seq, bytes]);
    return [seq, bytes];
  }

  /** The reply retires its sequence; a stray one is dropped. */
  receive(bytes: Uint8Array): [bigint, Message] | undefined {
    const decoded = decode(bytes);
    if (decoded === undefined) return undefined;
    const at = this.outstanding.findIndex(([seq]) => seq === decoded[0]);
    if (at < 0) return undefined;
    this.outstanding.splice(at, 1);
    return decoded;
  }

  /** What to send again, oldest first. */
  unanswered(): readonly [bigint, Uint8Array][] {
    return this.outstanding;
  }
}
