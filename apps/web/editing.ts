import { Client, hex, message, opening, outcome, resource, u64, unhex,
  type Opening, type Receipt, type ResourceId } from '../../sdk/typescript/resource.js';
import { deliver } from '../../sdk/typescript/delivery.js';
import { result as retainedResult } from '../../sdk/typescript/requests.js';
import * as text from '../../modules/text/api/typescript.js';

interface Source { buffer: ResourceId; history: ResourceId; revision: string;
  shared?: boolean; readOnly?: boolean; backing?: ResourceId; name?: string; windowStart?: string }
interface AcceptedInput { client: string; id: string; revision: string; buffer: ResourceId }
export interface Journal {
  token?: string; opening?: Opening; receipt?: Receipt;
  source?: Source; access?: ResourceId; observed?: text.Observation;
  inactive?: Source[]; openingName?: string;
  readOnlyTarget?: ResourceId;
  window?: text.Window; retiring: ResourceId[];
  change?: text.Change; needsRefresh?: boolean;
  acceptedInput?: AcceptedInput;
  resuming?: { command: string; locator: string; original?: Receipt; work?: ResourceId;
    probe?: ResourceId; probes?: ResourceId[]; next?: string; depth?: number;
    checked?: boolean; failed?: string; applied?: string };
  openingCleanup?: string;
  openingFailed?: string;
  openingWork?: ResourceId[];
}

/** Buffer ownership outlives the active Text view. */
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
    this.resumeUnknown();
    if (this.journal.receipt) await this.send(client, this.journal.receipt.fields);
    const resume = this.journal.resuming;
    if (resume) {
      if (!resume.work) await this.send(client, ['resolve', resume.locator]);
      if (resume.original) {
        await client.release(resume.original.id);
        delete resume.original; this.persist();
      }
      await this.checkOpening(client);
      if (resume.failed) {
        this.journal.retiring.unshift(resume.work!);
        this.journal.openingCleanup = resume.command;
        this.journal.openingFailed = resume.failed;
        this.journal.openingWork = resume.probes ?? [];
        delete this.journal.resuming; this.persist();
      } else {
        await this.send(client, ['write', resume.command, 'command.open.resume', '1', '', resume.work!]);
      }
    }
    await this.retire(client);
    await this.finishOpening(client);
  }

  private async finishOpening(client: Client): Promise<void> {
    if (this.journal.openingCleanup) {
      await this.send(client, ['write', this.journal.openingCleanup, 'command.open.cleanup', '1']);
    }
    while (this.journal.openingWork?.length) {
      await this.send(client, ['close', this.journal.openingWork[0]!]);
    }
    if (this.journal.openingWork) { delete this.journal.openingWork; this.persist(); }
    if (this.journal.openingFailed) {
      const code = this.journal.openingFailed;
      delete this.journal.openingFailed; this.persist();
      throw new Error(`Opening failed (code ${code}); cleanup completed`);
    }
  }

  private async checkOpening(client: Client): Promise<void> {
    const resume = this.journal.resuming!;
    while (!resume.checked) {
      if (resume.next) await this.send(client, ['resolve', resume.next]);
      if ((resume.depth ?? 0) === 16) throw new Error('Opening recovery chain limit; work retained');
      await this.send(client, ['read', resume.probe ?? resume.work!, 'kernel.request.result', '1']);
    }
  }

  private resumeUnknown(): void {
    const receipt = this.journal.receipt;
    if (!receipt?.response?.startsWith('error\tunknown\t')
        || !['command.open', 'command.open.resume'].includes(receipt.fields[2]!)) return;
    const result = outcome(receipt.response);
    if (result.kind !== 'unknown' || !result.recovery) throw new Error('Missing opening recovery');
    if (this.journal.resuming?.original) throw new Error('Retire the original opening receipt first');
    if (this.journal.resuming?.work) this.journal.retiring.unshift(this.journal.resuming.work);
    this.journal.resuming = { command: receipt.fields[1]!, locator: result.recovery, original: receipt };
    delete this.journal.receipt; this.persist();
  }

  private send(client: Client, fields: string[], correlate?: (receipt: Receipt) => void): Promise<void> {
    return deliver(client, this.journal, this.persist, fields,
      (request, line) => this.adopt(request, line), correlate);
  }

  private adopt(fields: string[], line: string): void {
    const state = this.journal;
    if (fields[0] === 'resolve') {
      if (state.resuming?.applied && state.resuming.applied === state.receipt?.id) return;
      const parts = line.split('\t');
      if (!state.resuming || fields[1] !== (state.resuming.next ?? state.resuming.locator)
          || parts.length !== 2 || parts[0] !== 'ok') throw new Error('foreign opening recovery');
      if (state.resuming.next) {
        state.resuming.probe = resource(parts[1]!);
        (state.resuming.probes ??= []).push(state.resuming.probe);
        delete state.resuming.next;
      } else state.resuming.work = resource(parts[1]!);
      state.resuming.applied = state.receipt!.id;
      return;
    }
    if (fields[0] === 'close' && line === 'ok') {
      state.retiring = state.retiring.filter(id => id !== fields[1]);
      state.openingWork = state.openingWork?.filter(id => id !== fields[1]);
      if (state.access === fields[1]) delete state.access;
      return;
    }
    const value = message(line);
    if (fields[2] === 'kernel.request.result') {
      const resume = state.resuming;
      if (resume?.applied && resume.applied === state.receipt?.id) return;
      if (!resume || fields[1] !== (resume.probe ?? resume.work)) throw new Error('foreign opening result');
      const result = retainedResult(line);
      if (result.kind === 'unknown') {
        resume.next = result.recovery; resume.depth = (resume.depth ?? 0) + 1;
      } else {
        resume.checked = true;
        if (result.kind !== 'ok') resume.failed = result.code.toString();
      }
      resume.applied = state.receipt!.id;
      return;
    }
    if (fields[2] === 'command.open.cleanup') {
      if (value.contract !== fields[2] || value.schema !== 1 || value.data.length
          || value.resources.length || (state.openingCleanup && fields[1] !== state.openingCleanup)) {
        throw new Error('invalid opening cleanup');
      }
      delete state.openingCleanup; return;
    }
    if (['source.initial', 'text.replace', 'command.open', 'command.open.resume'].includes(fields[2]!)) {
      const backed = fields[2] === 'command.open' || fields[2] === 'command.open.resume';
      const contract = fields[2] === 'source.initial' ? 'source.bytes' : fields[2];
      if (value.contract !== contract || value.schema !== 1
          || value.resources.length !== (backed ? 3 : 2)
          || value.data.length !== (backed ? 8 : 24)) {
        throw new Error('invalid source acceptance');
      }
      if (state.source && state.source.buffer !== value.resources[0]) {
        throw new Error('foreign source acceptance');
      }
      state.source = { shared: state.source?.shared,
        name: state.source?.name ?? state.openingName,
        backing: backed ? value.resources[2] : state.source?.backing,
        buffer: value.resources[0]!, history: value.resources[1]!,
        revision: new DataView(value.data.buffer).getBigUint64(0, true).toString() };
      delete state.openingName;
      if (fields[2] === 'command.open.resume') {
        // Newer resume records refer to older work; retire dependants first.
        if (state.resuming?.work) state.retiring.unshift(state.resuming.work);
        state.openingCleanup = fields[1];
        delete state.resuming;
      }
      if (fields[2] === 'text.replace') {
        state.acceptedInput = { client: state.receipt!.client, id: state.receipt!.id,
          revision: state.source.revision, buffer: state.source.buffer };
      }
      delete state.window;
    } else if (fields[2] === 'kernel.buffer.describe') {
      if (value.contract !== 'kernel.buffer.describe' || value.schema !== 1
          || value.data.length !== 8 || value.resources.length < 2
          || value.resources.length > 3) throw new Error('invalid Buffer descriptor');
      if (state.readOnlyTarget && value.resources[0] !== state.readOnlyTarget) throw new Error('foreign read-only Buffer');
      state.source = { shared: true, readOnly: state.readOnlyTarget !== undefined,
        buffer: value.resources[0]!, history: value.resources[1]!,
        backing: value.resources[2],
        revision: new DataView(value.data.buffer).getBigUint64(0, true).toString() };
      delete state.readOnlyTarget;
    } else if (fields[2] === 'kernel.history.undo' || fields[2] === 'kernel.history.redo') {
      if (value.contract !== fields[2] || value.schema !== 1
          || value.data.length !== 16 || value.resources.length || !state.source) {
        throw new Error('invalid History acceptance');
      }
      state.source.revision = new DataView(value.data.buffer).getBigUint64(0, true).toString();
      delete state.window;
    } else if (fields[2] === 'text.attach' || fields[2] === 'text.attach.read') {
      if (value.contract !== fields[2] || value.schema !== 1
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
    if (this.journal.receipt || this.journal.resuming) throw new Error('Recover the pending request first');
    const client = await this.client();
    await this.retire(client);
    await this.finishOpening(client);
    if (this.journal.readOnlyTarget) { delete this.journal.readOnlyTarget; this.persist(); }
    return client;
  }

  async create(bytes: Uint8Array, observe = true): Promise<void> {
    const client = await this.ready();
    if (this.journal.source) throw new Error('Keep the current source open or close it first');
    this.journal.openingName = `Untitled ${(this.journal.inactive?.length ?? 0) + 1}`;
    this.persist();
    await this.send(client, ['write', 'source', 'source.initial', '1', hex(bytes)]);
    if (observe) await this.observe();
  }

  async join(observe = true): Promise<void> {
    const client = await this.ready();
    if (this.journal.source) throw new Error('Detach the existing source first');
    await this.send(client, ['read', 'buffer', 'kernel.buffer.describe', '1']);
    if (observe) await this.observe();
  }

  async openFile(name: string, command = 'open', directory = 'modules/files',
    model = 'file_source', observe = true): Promise<void> {
    if (!name) throw new Error('Choose a file first');
    const client = await this.ready();
    if (this.journal.source) throw new Error('Keep the current source open or close it first');
    this.journal.openingName = name; this.persist();
    await this.send(client, ['write', command, 'command.open', '1',
      hex(new TextEncoder().encode(name)), `${directory},${model}`]);
    if (observe) await this.observe();
  }

  async observe(start = 0n, count = 1024n): Promise<void> {
    const client = await this.ready();
    const state = this.journal;
    if (!state.source) throw new Error('Create a source first');
    if (!state.access) {
      await this.send(client, ['write', 'text', state.source.readOnly ? 'text.attach.read' : 'text.attach', '1', '', state.source.buffer]);
    }
    await this.send(client, ['read', state.access!, 'text.observe', '1']);
    await this.retire(client);
    await this.read(start, count);
  }

  async read(start: bigint, count = 1024n): Promise<void> {
    const client = await this.ready();
    if (!this.journal.observed) throw new Error('Observe Text first');
    await this.send(client, ['read', this.journal.observed.snapshot, 'text.window', '1',
      hex(u64(start)) + hex(u64(count))]);
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

  async replace(start: bigint, end: bigint, replacement: string,
    correlate?: (receipt: Receipt) => void): Promise<text.Window> {
    const client = await this.ready();
    if (this.journal.source?.readOnly) throw new Error('This source is read-only');
    if (!this.journal.observed) throw new Error('Observe Text first');
    const window = this.journal.window;
    let count = 1024n;
    if (window && start >= BigInt(window.start) && end <= BigInt(window.end)) {
      count = BigInt(window.units.length + [...replacement].length) - (end - start);
      if (count > 1024n) count = 1024n;
    }
    await this.send(client, text.replace(this.journal.observed, start, end, replacement), correlate);
    const accepted = this.journal.source!.revision;
    await this.observe(BigInt(window?.start ?? '0'), count);
    if (this.journal.observed.revision !== accepted) {
      throw new Error('Source changed after acceptance; queued input retained');
    }
    return this.journal.window!;
  }

  async inputCheckpoint(client: string, id: string, expected: text.Window): Promise<text.Window> {
    await this.recover();
    const accepted = this.journal.acceptedInput;
    if (!accepted || accepted.client !== client || accepted.id !== id
        || this.journal.token !== client || this.journal.source?.buffer !== accepted.buffer) {
      throw new Error('Original input acceptance is unavailable; queue retained');
    }
    await this.observe(BigInt(expected.start), BigInt(expected.units.length));
    if (this.journal.observed?.revision !== accepted.revision
        || this.journal.observed.buffer !== accepted.buffer) {
      throw new Error('Source changed after original acceptance; queue retained');
    }
    return this.journal.window!;
  }

  undo(): Promise<void> { return this.history('undo'); }
  redo(): Promise<void> { return this.history('redo'); }

  async history(direction: 'undo' | 'redo'): Promise<void> {
    const client = await this.ready();
    const source = this.journal.source;
    if (source?.readOnly) throw new Error('This source is read-only');
    if (!source) throw new Error('Create a source first');
    await this.send(client, ['write', source.history, `kernel.history.${direction}`, '1',
      hex(u64(BigInt(source.revision))), source.buffer]);
    await this.observe();
  }

  async close(): Promise<void> {
    const client = await this.ready();
    const state = this.journal;
    await this.releaseView(client);
    if (state.source?.shared) {
      delete state.source;
      this.persist();
    } else if (state.source) {
      await this.send(client, ['write', state.source.buffer, 'kernel.buffer.close', '1',
        hex(u64(BigInt(state.source.revision))) + '01', state.source.buffer]);
    }
  }

  async park(): Promise<void> {
    const client = await this.ready();
    const state = this.journal;
    if (!state.source) return;
    state.source.windowStart = state.window?.start ?? state.source.windowStart ?? '0';
    await this.releaseView(client);
    state.inactive ??= [];
    state.inactive.push(state.source);
    delete state.source; delete state.acceptedInput;
    this.persist();
  }

  async switchTo(buffer: ResourceId): Promise<void> {
    await this.ready();
    const state = this.journal;
    if (state.source?.buffer === buffer) { await this.observe(); return; }
    const selected = state.inactive?.find(source => source.buffer === buffer);
    if (!selected) throw new Error('That Buffer is not open in this client');
    await this.park();
    state.inactive = state.inactive!.filter(source => source.buffer !== buffer);
    state.source = selected;
    this.persist();
    await this.observe(BigInt(selected.windowStart ?? '0'));
  }

  async showContext(buffer: ResourceId): Promise<void> {
    if (this.journal.source?.buffer === buffer || this.journal.inactive?.some(source => source.buffer === buffer)) {
      await this.switchTo(buffer); return;
    }
    await this.park();
    const client = await this.ready();
    this.journal.readOnlyTarget = buffer; this.persist();
    await this.send(client, ['read', buffer, 'kernel.buffer.describe', '1']);
    await this.observe();
  }

  private async releaseView(client: Client): Promise<void> {
    const state = this.journal;
    if (state.observed) {
      state.retiring.push(state.observed.snapshot, state.observed.watch);
      delete state.observed; delete state.window;
      delete state.change; delete state.needsRefresh;
      this.persist();
    }
    await this.retire(client);
    if (state.access) await this.send(client, ['close', state.access]);
  }
}
