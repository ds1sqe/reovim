export type ResourceId = `@${bigint}`;
export interface Message { contract: string; schema: number; data: Uint8Array; resources: ResourceId[] }
export interface Receipt { client: string; id: string; fields: string[]; response?: string }
/** Persist before connect; never replace an uncertain opening with a new one. */
export interface Opening { key: string; sent?: boolean; response?: string;
  released?: boolean }
export function opening(): Opening {
  return { key: hex(crypto.getRandomValues(new Uint8Array(16))) };
}
export type Outcome = { kind: 'ok'; value: Message } | {
  kind: 'rejected' | 'failed' | 'unknown'; code: bigint;
  resource?: ResourceId; recovery: string
};
function integer(value: string): bigint {
  if (!/^(?:0|[1-9][0-9]{0,19})$/.test(value)) throw new Error('invalid integer');
  const result = BigInt(value);
  if (result > 0xffffffffffffffffn) throw new Error('u64 range');
  return result;
}
export function outcome(line: string): Outcome {
  if (line.startsWith('ok\t')) return { kind: 'ok', value: message(line) };
  const [tag, kind, code, id, recovery, extra] = line.split('\t');
  if (tag !== 'error' || !['rejected', 'failed', 'unknown'].includes(kind ?? '')
      || code === undefined || id === undefined || recovery === undefined || extra !== undefined) {
    throw new Error('invalid outcome');
  }
  return { kind: kind as 'rejected' | 'failed' | 'unknown', code: integer(code),
    resource: id ? resource(id) : undefined,
    recovery: new TextDecoder('utf-8', { fatal: true }).decode(unhex(recovery)) };
}
export class DeliveryError extends Error {
  constructor(public readonly receipt: Receipt, cause: unknown) {
    super('Outcome unavailable; recover this receipt before repeating the operation', { cause });
  }
}
/** A failed boot retains its Client and delivery receipt until cleanup settles. */
export class StartupError extends Error {
  constructor(public readonly client: Client, public readonly receipt: string,
    public readonly result: Exclude<Outcome, { kind: 'ok' }>,
    private readonly start: Opening) {
    super(`Client startup ${result.kind} (code ${result.code}); result retained`);
  }
  async cleanup(persist: () => void): Promise<void> {
    if (this.result.kind === 'unknown') {
      throw new Error('Startup outcome is unknown; reconcile before cleanup');
    }
    if (!this.start.released) {
      await this.client.release(this.receipt);
      this.start.released = true;
      persist();
    }
    const response = await this.client.wire([this.client.token, 'disconnect']);
    if (response !== 'ok') throw new Error(response);
  }
}
export function startupFailure(start: Opening, endpoint = '/rpc'): StartupError | undefined {
  if (!start.response?.startsWith('error\tboot\t')) return;
  const parts = start.response.split('\t');
  if (!/^[0-9a-f]{32}$/.test(parts[2] ?? '') || integer(parts[3] ?? '') === 0n) {
    throw new Error('invalid startup owner');
  }
  const result = outcome(['error', ...parts.slice(4)].join('\t'));
  if (result.kind === 'ok') throw new Error('invalid startup failure');
  return new StartupError(new Client(parts[2]!, endpoint), parts[3]!, result, start);
}
export const u64 = (value: bigint): Uint8Array => {
  if (value < 0n || value > 0xffffffffffffffffn) throw new Error('u64 range');
  const data = new Uint8Array(8);
  new DataView(data.buffer).setBigUint64(0, value, true);
  return data;
};
export const hex = (data: Uint8Array): string => [...data].map(n => n.toString(16).padStart(2, '0')).join('');
export function unhex(value: string): Uint8Array {
  if (!/^(?:[0-9a-f]{2})*$/.test(value)) throw new Error('invalid bytes');
  return Uint8Array.from(value.match(/../g) ?? [], n => Number.parseInt(n, 16));
}
export function resource(value: string): ResourceId {
  if (!value.startsWith('@')) {
    throw new Error('invalid Resource identity');
  }
  integer(value.slice(1));
  return value as ResourceId;
}
export function message(line: string): Message {
  const parts = line.split('\t');
  if (parts.length !== 5 || parts[0] !== 'ok' || !/^\d+$/.test(parts[2]!)) {
    throw new Error(line);
  }
  const schema = Number(parts[2]);
  if (!Number.isSafeInteger(schema) || schema > 0xffffffff) throw new Error('invalid schema');
  return { contract: parts[1]!, schema, data: unhex(parts[3]!),
    resources: parts[4] ? parts[4].split(',').map(resource) : [] };
}
export class Client {
  constructor(public readonly token: string, private readonly endpoint = '/rpc') {}
  async wire(fields: string[]): Promise<string> {
    const response = await fetch(this.endpoint, { method: 'POST', headers: {
      'Content-Type': 'application/json', 'X-Reovim': '1'
    }, body: JSON.stringify(fields) });
    if (!response.ok) throw new Error(`transport ${response.status}`);
    return response.text();
  }
  static async connect(start: Opening, persist: () => void,
    endpoint = '/rpc'): Promise<Client> {
    const temporary = new Client('', endpoint);
    if (start.response === undefined) {
      const action = start.sent ? 'recover' : 'open';
      start.sent = true;
      persist();
      const response = await temporary.wire([action, start.key]);
      // Control errors are not startup results; keep recovery possible.
      if (!response.startsWith('ok\t') && !response.startsWith('error\tboot\t')) {
        throw new Error(response);
      }
      start.response = response;
      persist();
    }
    const failure = startupFailure(start, endpoint);
    if (failure) throw failure;
    const parts = start.response.split('\t');
    if (parts.length !== 3 || parts[0] !== 'ok' || !/^[0-9a-f]{32}$/.test(parts[1]!)) {
      throw new Error('invalid client handshake');
    }
    const client = new Client(parts[1]!, endpoint);
    integer(parts[2]!);
    await client.release(parts[2]!);
    return client;
  }
  async reserve(fields: string[]): Promise<Receipt> {
    const parts = (await this.wire([this.token, 'reserve'])).split('\t');
    if (parts.length !== 2 || parts[0] !== 'ok' || integer(parts[1]!) === 0n) throw new Error('reservation failed');
    return { client: this.token, id: parts[1]!, fields: [...fields] };
  }
  async send(receipt: Receipt): Promise<string> {
    if (receipt.client !== this.token) throw new Error('foreign receipt');
    if (receipt.response !== undefined) return receipt.response;
    try { return await this.wire([this.token, 'call', receipt.id, ...receipt.fields]); }
    catch (error) { throw new DeliveryError(receipt, error); }
  }
  async release(id: string): Promise<void> {
    if (await this.wire([this.token, 'release', id]) === 'ok') return;
    const list = (await this.wire([this.token, 'requests'])).split('\t');
    const entries = list.slice(1).filter(Boolean);
    if (list[0] !== 'ok' || entries.some(item => !/^[0-9]+=(reserved|pending|finished)$/.test(item))
        || entries.some(item => item.split('=')[0] === id)) {
      throw new Error(`receipt ${id} remains owned`);
    }
  }
}
