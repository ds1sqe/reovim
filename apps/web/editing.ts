import { Client, hex, message, opening, u64, unhex,
  type Opening, type Receipt, type ResourceId } from '../../sdk/typescript/resource.js';
import { deliver } from '../../sdk/typescript/delivery.js';
import * as text from '../../modules/text/api/typescript.js';

interface Source { buffer: ResourceId; history: ResourceId; revision: string }
export interface Journal {
  token?: string; opening?: Opening; receipt?: Receipt;
  source?: Source; access?: ResourceId; observed?: text.Observation;
  window?: text.Window; retiring: ResourceId[];
  change?: text.Change; needsRefresh?: boolean;
}

/** This composition owns its source; sharing/follow policy is a later consumer. */
export class Editing {
  constructor(public readonly journal: Journal, private readonly persist: () => void,
    private readonly endpoint = '/rpc') {}

  private async client(): Promise<Client> {
    const state = this.journal;
    if (!state.token) {
      state.opening ??= opening();
      this.persist();
      const client = await Client.connect(state.opening, this.persist, this.endpoint);
      state.token = client.token;
      delete state.opening;
      this.persist();
    }
    return new Client(state.token, this.endpoint);
  }

  async recover(): Promise<void> {
    const client = await this.client();
    if (this.journal.receipt) await this.send(client, this.journal.receipt.fields);
    await this.retire(client);
  }

  private send(client: Client, fields: string[]): Promise<void> {
    return deliver(client, this.journal, this.persist, fields,
      (request, line) => this.adopt(request, line));
  }

  private adopt(fields: string[], line: string): void {
    const state = this.journal;
    if (fields[0] === 'close' && line === 'ok') {
      state.retiring = state.retiring.filter(id => id !== fields[1]);
      if (state.access === fields[1]) delete state.access;
      return;
    }
    const value = message(line);
    if (fields[2] === 'source.initial' || fields[2] === 'text.replace') {
      const contract = fields[2] === 'text.replace' ? 'text.replace' : 'source.bytes';
      if (value.contract !== contract || value.schema !== 1
          || value.resources.length !== 2 || value.data.length !== 24) {
        throw new Error('invalid source acceptance');
      }
      if (state.source && state.source.buffer !== value.resources[0]) {
        throw new Error('foreign source acceptance');
      }
      state.source = { buffer: value.resources[0]!, history: value.resources[1]!,
        revision: new DataView(value.data.buffer).getBigUint64(0, true).toString() };
      delete state.window;
    } else if (fields[2] === 'kernel.history.undo') {
      if (value.contract !== 'kernel.history.undo' || value.schema !== 1
          || value.data.length !== 16 || value.resources.length || !state.source) {
        throw new Error('invalid History acceptance');
      }
      state.source.revision = new DataView(value.data.buffer).getBigUint64(0, true).toString();
      delete state.window;
    } else if (fields[2] === 'text.attach') {
      if (value.contract !== 'text.attach' || value.schema !== 1
          || value.resources.length !== 1 || value.data.length) {
        throw new Error('invalid Text attachment');
      }
      state.access = value.resources[0]!;
    } else if (fields[2] === 'text.observe') {
      const next = text.observation(line, state.access!);
      if (next.buffer !== state.source?.buffer) throw new Error('foreign Text Buffer');
      if (state.observed && state.observed.snapshot !== next.snapshot) {
        state.retiring.push(state.observed.snapshot, state.observed.watch);
      }
      state.observed = next;
      state.source.revision = next.revision;
      state.needsRefresh = false;
      delete state.window;
    } else if (fields[2] === 'text.next') {
      if (!state.observed || fields[1] !== state.observed.watch) throw new Error('foreign Text watch');
      state.change = text.change(line, state.observed);
      const kind = state.change.delivery.kind;
      state.needsRefresh = kind === 'item' || kind === 'gap';
    } else if (fields[2] === 'text.window') {
      if (fields[1] !== state.observed?.snapshot) throw new Error('foreign Text snapshot');
      const range = new DataView(unhex(fields[4]!).buffer);
      state.window = text.window(line, range.getBigUint64(0, true), range.getBigUint64(8, true));
    } else if (fields[2] === 'kernel.buffer.close') {
      if (value.contract !== 'kernel.buffer.close' || value.schema !== 1
          || hex(value.data) !== '636c6f736564' || value.resources.length) {
        throw new Error('invalid Buffer close');
      }
      delete state.source;
    } else { throw new Error('unexpected editing result'); }
  }

  private async retire(client: Client): Promise<void> {
    while (this.journal.retiring.length) {
      await this.send(client, ['close', this.journal.retiring[0]!]);
    }
  }

  private async ready(): Promise<Client> {
    if (this.journal.receipt) throw new Error('Recover the pending request first');
    const client = await this.client();
    await this.retire(client);
    return client;
  }

  async create(bytes: Uint8Array): Promise<void> {
    const client = await this.ready();
    if (this.journal.source) throw new Error('Close the existing source first');
    await this.send(client, ['write', 'source', 'source.initial', '1', hex(bytes)]);
    await this.observe();
  }

  async observe(): Promise<void> {
    const client = await this.ready();
    const state = this.journal;
    if (!state.source) throw new Error('Create a source first');
    if (!state.access) {
      await this.send(client, ['write', 'text', 'text.attach', '1', '', state.source.buffer]);
    }
    await this.send(client, ['read', state.access!, 'text.observe', '1']);
    await this.retire(client);
    await this.read(0n);
  }

  async read(start: bigint): Promise<void> {
    const client = await this.ready();
    if (!this.journal.observed) throw new Error('Observe Text first');
    await this.send(client, ['read', this.journal.observed.snapshot, 'text.window', '1',
      hex(u64(start)) + hex(u64(1024n))]);
  }

  async follow(): Promise<void> {
    const client = await this.ready();
    const state = this.journal;
    if (!state.observed) throw new Error('Observe Text first');
    if (!state.needsRefresh) {
      await this.send(client, ['read', state.observed.watch, 'text.next', '1', '00']);
    }
    if (state.needsRefresh) await this.observe();
  }

  async replace(start: bigint, end: bigint, replacement: string): Promise<void> {
    const client = await this.ready();
    if (!this.journal.observed) throw new Error('Observe Text first');
    await this.send(client, text.replace(this.journal.observed, start, end, replacement));
    await this.observe();
  }

  async undo(): Promise<void> {
    const client = await this.ready();
    const source = this.journal.source;
    if (!source) throw new Error('Create a source first');
    await this.send(client, ['write', source.history, 'kernel.history.undo', '1',
      hex(u64(BigInt(source.revision))), source.buffer]);
    await this.observe();
  }

  async close(): Promise<void> {
    const client = await this.ready();
    const state = this.journal;
    if (state.observed) {
      state.retiring.push(state.observed.snapshot, state.observed.watch);
      delete state.observed; delete state.window;
      delete state.change; delete state.needsRefresh;
      this.persist();
    }
    await this.retire(client);
    if (state.access) await this.send(client, ['close', state.access]);
    if (state.source) {
      await this.send(client, ['write', state.source.buffer, 'kernel.buffer.close', '1',
        hex(u64(BigInt(state.source.revision))) + '01', state.source.buffer]);
    }
  }
}
