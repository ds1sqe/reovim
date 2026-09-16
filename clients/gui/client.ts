// The Client over a Carrier, as clients/core has it: the ops, and the
// helpers above them. Bytes go out and come back through the Carrier;
// the Client owns the Endpoint, asks a Pending reply until it is ready,
// and recovers on a cut link with the same credential.

import { Writer } from '../../sdk/typescript/codec.js';
import {
  type Cause, Endpoint, type Event, type Handle, type Hash, type Info,
  type Message, type Op, type Out, type Rd, READ, WRITE,
} from '../../sdk/typescript/wire.js';
import { type Decl, type Method, parseDecl } from '../../sdk/typescript/value.js';

export { READ, WRITE };
export const ROOT: Rd = 0n;
const CHUNK = 64 * 1024;

/** Why a call did not happen or did not finish. */
export class Fault extends Error {
  constructor(
    readonly kind: 'kernel' | 'carrier' | 'wire' | 'usage',
    readonly cause?: Cause,
    message: string = cause ?? kind,
  ) {
    super(message);
  }
}

/** Moves one message out and brings the next reply in. */
export interface Carrier {
  exchange(bytes: Uint8Array): Promise<Uint8Array>;
  /** A fresh link to the same end, after a carrier fault. */
  reconnect(): Promise<void>;
}

export class Client {
  private endpoint = new Endpoint();
  session: Rd = ROOT;
  cid: Rd = ROOT;
  private cursor = 0n;

  private constructor(private carrier: Carrier, private credential: Uint8Array) {}

  static async connect(carrier: Carrier, credential = new Uint8Array()): Promise<Client> {
    const client = new Client(carrier, credential);
    client.connected(await client.exchange({ kind: 'connect', credential }));
    return client;
  }

  private connected(out: Out): void {
    if (out.kind !== 'rds' || out.rds.length !== 2) throw new Fault('wire');
    [this.session, this.cid] = [out.rds[0]!, out.rds[1]!];
  }

  private async exchange(message: Message): Promise<Out> {
    const [seq, bytes] = this.endpoint.send(message);
    let answer: Uint8Array;
    try {
      answer = await this.carrier.exchange(bytes);
    } catch (error) {
      if (!(error instanceof Fault) || error.kind !== 'carrier') throw error;
      answer = await this.recover(seq);
    }
    const received = this.endpoint.receive(answer);
    if (received === undefined || received[1].kind !== 'reply') throw new Fault('wire');
    const reply = received[1].reply;
    switch (reply.kind) {
      case 'ok': return reply.out;
      case 'err': throw new Fault('kernel', reply.cause);
      case 'pending': return this.wait(reply.token);
    }
  }

  /** The link is gone: a fresh one, Connect again, everything unanswered
   * again in order; the answer to `seq` raw. */
  private async recover(seq: bigint): Promise<Uint8Array> {
    await this.carrier.reconnect();
    const connect: Message = {
      kind: 'connect', credential: this.credential, session: this.session,
    };
    const [, bytes] = this.endpoint.send(connect);
    const received = this.endpoint.receive(await this.carrier.exchange(bytes));
    if (received?.[1].kind !== 'reply' || received[1].reply.kind !== 'ok') {
      throw new Fault('carrier');
    }
    this.connected(received[1].reply.out);
    let ours: Uint8Array | undefined;
    for (const [at, sent] of [...this.endpoint.unanswered()]) {
      const answer = await this.carrier.exchange(sent);
      if (at === seq) ours = answer;
      else this.endpoint.receive(answer);
    }
    if (ours === undefined) throw new Fault('carrier');
    return ours;
  }

  private async wait(token: bigint): Promise<Out> {
    for (;;) {
      try {
        return await this.exchange({ kind: 'request', op: { kind: 'ask', token } });
      } catch (error) {
        if (!(error instanceof Fault) || error.cause !== 'busy') throw error;
      }
    }
  }

  private request(op: Op): Promise<Out> {
    return this.exchange({ kind: 'request', op });
  }

  private async expect<K extends Out['kind']>(op: Op, kind: K): Promise<Extract<Out, { kind: K }>> {
    const out = await this.request(op);
    if (out.kind !== kind) throw new Fault('wire');
    return out as Extract<Out, { kind: K }>;
  }

  async lookup(rd: Rd, name: string): Promise<Rd> {
    return (await this.expect({ kind: 'lookup', rd, name }, 'rd')).rd;
  }

  async info(rd: Rd): Promise<Info> {
    return (await this.expect({ kind: 'info', rd }, 'info')).info;
  }

  async parent(rd: Rd): Promise<Rd> {
    return (await this.expect({ kind: 'parent', rd }, 'rd')).rd;
  }

  async children(rd: Rd): Promise<Rd[]> {
    return (await this.expect({ kind: 'children', rd }, 'rds')).rds;
  }

  async open(rd: Rd, rights: number): Promise<Handle> {
    return (await this.expect({ kind: 'open', rd, rights }, 'handle')).h;
  }

  async read(h: Handle, at: bigint, len: number): Promise<Uint8Array> {
    return (await this.expect({ kind: 'read', h, at, len }, 'bytes')).bytes;
  }

  async write(h: Handle, at: bigint, bytes: Uint8Array): Promise<number> {
    return (await this.expect({ kind: 'write', h, at, bytes }, 'len')).len;
  }

  async call(h: Handle, api: Hash, method: number, input: Uint8Array): Promise<Uint8Array> {
    return (await this.expect({ kind: 'call', h, api, method, input }, 'bytes')).bytes;
  }

  async watch(h: Handle, on: boolean): Promise<void> {
    await this.expect({ kind: 'watch', h, on }, 'unit');
  }

  async close(h: Handle): Promise<void> {
    await this.expect({ kind: 'close', h }, 'unit');
  }

  /** Events after the cursor; the cursor moves on. */
  async since(): Promise<Event[]> {
    const out = await this.expect({ kind: 'since', cursor: this.cursor }, 'events');
    this.cursor = out.cursor;
    return out.events;
  }

  async load(path: string): Promise<void> {
    const out = await this.exchange({ kind: 'load', path });
    if (out.kind !== 'unit') throw new Fault('wire');
  }

  async unload(name: string): Promise<void> {
    const out = await this.exchange({ kind: 'unload', name });
    if (out.kind !== 'unit') throw new Fault('wire');
  }

  /** `/a/b/c` by lookup from the root. */
  async resolve(path: string): Promise<Rd> {
    if (!path.startsWith('/')) throw new Fault('usage', undefined, `not a path: ${path}`);
    let rd = ROOT;
    for (const name of path.split('/').filter((n) => n !== '')) rd = await this.lookup(rd, name);
    return rd;
  }

  /** The whole content through an open handle. */
  async readWhole(h: Handle): Promise<Uint8Array> {
    const parts: Uint8Array[] = [];
    let at = 0n;
    for (;;) {
      const chunk = await this.read(h, at, CHUNK);
      if (chunk.length === 0) break;
      parts.push(chunk);
      at += BigInt(chunk.length);
    }
    return concat(parts);
  }

  /** `bytes` at 0, through a handle opened and closed here. */
  async writeAll(rd: Rd, bytes: Uint8Array): Promise<number> {
    const h = await this.open(rd, WRITE);
    try {
      return await this.write(h, 0n, bytes);
    } finally {
      await this.close(h);
    }
  }

  /** This Client's capability bytes. */
  async capability(bytes: Uint8Array): Promise<void> {
    await this.writeAll(await this.lookup(this.cid, 'capability'), bytes);
  }

  /** The declaration at /sys/decls/<hash>. */
  async decl(hash: Hash): Promise<Decl> {
    const name = hash.toString(16).padStart(16, '0');
    const rd = await this.resolve(`/sys/decls/${name}`);
    const h = await this.open(rd, READ);
    try {
      return parseDecl(await this.readWhole(h));
    } finally {
      await this.close(h);
    }
  }

  async decls(rd: Rd): Promise<[Hash, Decl][]> {
    const out: [Hash, Decl][] = [];
    for (const hash of (await this.info(rd)).apis) out.push([hash, await this.decl(hash)]);
    return out;
  }

  /** The first method named `name` among what `rd` offers. */
  async method(rd: Rd, name: string): Promise<[Hash, Method]> {
    for (const [hash, decl] of await this.decls(rd)) {
      const method = decl.methods.find((m) => m.name === name);
      if (method !== undefined) return [hash, method];
    }
    throw new Fault('usage', undefined, `no method ${name} on rd ${rd}`);
  }

  /** `name` on `rd` through a handle opened with the method's right. */
  async callByName(rd: Rd, name: string, input: Uint8Array): Promise<[Method, Uint8Array]> {
    const [api, method] = await this.method(rd, name);
    const h = await this.open(rd, method.right);
    try {
      return [method, await this.call(h, api, method.number, input)];
    } finally {
      await this.close(h);
    }
  }
}

function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

/** An rd id as call input. */
export function rdInput(rd: Rd): Uint8Array {
  const w = new Writer();
  w.u64(rd);
  return w.done();
}

/** The `Edit` record of `text.replace`. */
export function edit(at: number, len: number, text: string): Uint8Array {
  const w = new Writer();
  w.u64(BigInt(at));
  w.u64(BigInt(len));
  w.str(text);
  return w.done();
}

/** The rd an `Rd` output names. */
export function rdOutput(bytes: Uint8Array): Rd {
  if (bytes.length !== 8) throw new Fault('wire');
  return new DataView(bytes.buffer, bytes.byteOffset, 8).getBigUint64(0, true);
}
