import { Client, message, type Receipt, type ResourceId } from '../../../sdk/typescript/resource.js';
import { deliver } from '../../../sdk/typescript/delivery.js';
import * as context from '../../../modules/context/api/typescript.js';

export interface ContextJournal {
  acceptedUpdate?: Pick<Receipt, 'client' | 'id'>;
  receipt?: Receipt; current?: context.Observation; retiring: ResourceId[];
  ownership?: 'private' | 'shared'; participation?: ResourceId;
  checkpoint?: context.Observation; checkpointRequest?: string;
  historical?: boolean; needsRead?: boolean; closing?: boolean;
  watch?: { id: ResourceId; session: ResourceId; revision: string };
  retiringWatches?: ResourceId[]; change?: context.Change; needsRefresh?: boolean;
}

/** Owned participation and observations never own referenced Buffers. */
export class Focus {
  private busy = false;
  constructor(private readonly client: Client, public readonly journal: ContextJournal,
    private readonly persist: () => void) {
    // Earlier journals could only create private Sessions.
    if (journal.current && !journal.ownership) {
      journal.ownership = 'private'; persist();
    }
  }

  private async run(action: () => Promise<void>, recovery = false): Promise<void> {
    if (this.busy) throw new Error('context operation pending');
    this.busy = true;
    try {
      if (this.journal.receipt && !recovery) throw new Error('Recover context work first');
      if (!recovery) await this.retire();
      await action();
      if (this.journal.closing) await this.finishClose();
      else if (this.journal.current) await this.ready();
      await this.retire();
      if (this.journal.closing) { delete this.journal.closing; this.persist(); }
    } finally { this.busy = false; }
  }
  private queue(id: ResourceId): void {
    if (!this.journal.retiring.includes(id)) this.journal.retiring.push(id);
  }
  private clearWatch(): void {
    if (this.journal.watch) {
      const queue = this.journal.retiringWatches ??= [];
      if (!queue.includes(this.journal.watch.id)) queue.push(this.journal.watch.id);
    }
    delete this.journal.watch; delete this.journal.needsRefresh;
  }
  private async retire(): Promise<void> {
    while (this.journal.retiring.length) {
      await this.send(['write', this.journal.retiring[0]!, 'kernel.session.release', '1']);
    }
    while (this.journal.retiringWatches?.length) {
      await this.send(['close', this.journal.retiringWatches[0]!]);
    }
  }
  private async send(fields: string[], checkpoint = false,
    correlate?: (receipt: Receipt) => void): Promise<void> {
    try {
      await deliver(this.client, this.journal, this.persist, fields,
        (request, line) => this.adopt(request, line), receipt => {
          if (checkpoint) this.journal.checkpointRequest = receipt.id;
          correlate?.(receipt);
        });
    } finally {
      if (!this.journal.receipt && this.journal.checkpointRequest) {
        delete this.journal.checkpointRequest; this.persist();
      }
    }
  }
  private adopt(request: string[], line: string): void {
    if (request[0] === 'close' && line === 'ok') {
      this.journal.retiringWatches = this.journal.retiringWatches?.filter(id => id !== request[1]);
      return;
    }
    const name = request[2];
    if (name === 'kernel.session.attach') {
      const reply = message(line), id = reply.resources[0];
      if (reply.contract !== name || reply.schema !== 1 || reply.data.length
          || reply.resources.length !== 1 || request[1] !== this.journal.current?.session
          || (this.journal.participation && this.journal.participation !== id)) {
        throw new Error('invalid context participation');
      }
      this.journal.participation = id; this.journal.needsRead = true;
      this.clearWatch(); return;
    }
    if (name === 'kernel.session.next') {
      if (!this.journal.watch || this.journal.watch.id !== request[1]
          || this.journal.watch.session !== this.journal.current?.session) throw new Error('foreign context watch');
      this.journal.change = context.change(line);
      this.journal.needsRefresh = ['item', 'gap'].includes(this.journal.change.kind);
      return;
    }
    if (['kernel.session.release', 'kernel.session.close',
      'kernel.session.seek', 'kernel.session.detach'].includes(name!)) {
      this.lifecycle(request, line); return;
    }
    const watched = name === 'kernel.session.follow' ? context.followed(line)
      : name === 'kernel.session.observe' ? context.watching(line) : undefined;
    const next = watched?.observation ?? context.observation(line), old = this.journal.current;
    if (old && old.session !== next.session) throw new Error('foreign context result');
    if (['editing.context', 'editing.context.update'].includes(name!)
        && this.journal.participation && request[1] !== this.journal.participation) {
      throw new Error('foreign context participation');
    }
    if (name === 'kernel.session.follow' && request[1] !== this.journal.participation) {
      throw new Error('foreign context participation');
    }
    if (this.journal.checkpointRequest && this.journal.checkpointRequest === this.journal.receipt?.id) {
      const previous = this.journal.checkpoint;
      if (previous && previous.snapshot !== next.snapshot) this.queue(previous.snapshot);
      this.journal.checkpoint = next; return;
    }
    if (name === 'kernel.session.create') this.journal.ownership = 'private';
    else if (!old && !this.journal.ownership) this.journal.ownership = 'shared';
    if (watched) {
      if (this.journal.watch?.id !== watched.watch) this.clearWatch();
      this.journal.watch = { id: watched.watch, session: next.session, revision: watched.revision };
      this.journal.needsRefresh = false; this.journal.historical = false;
    }
    if (old && old.snapshot !== next.snapshot) this.queue(old.snapshot);
    this.journal.current = next; this.journal.needsRead = false;
    if (name === 'editing.context.update') {
      const receipt = this.journal.receipt!;
      this.journal.acceptedUpdate = { client: receipt.client, id: receipt.id };
    }
  }
  private lifecycle(request: string[], line: string): void {
    const name = request[2], reply = message(line);
    if (reply.contract !== name || reply.schema !== 1 || reply.data.length || reply.resources.length) {
      throw new Error('invalid context lifecycle result');
    }
    if (name === 'kernel.session.release') {
      this.journal.retiring = this.journal.retiring.filter(id => id !== request[1]); return;
    }
    if (name === 'kernel.session.close') {
      if (this.journal.current && request[1] !== this.journal.current.session) throw new Error('foreign context close');
      this.clearContext(); return;
    }
    if (request[1] !== this.journal.participation) {
      // An adopted detach can be replayed while its receipt retirement retries.
      if (name === 'kernel.session.detach' && this.journal.closing && !this.journal.participation) return;
      throw new Error('foreign context participation');
    }
    this.clearWatch(); delete this.journal.change;
    if (name === 'kernel.session.seek') {
      this.journal.historical = true; this.journal.needsRead = true;
    } else delete this.journal.participation;
  }
  private clearContext(): void {
    if (this.journal.current) this.queue(this.journal.current.snapshot);
    if (this.journal.checkpoint) this.queue(this.journal.checkpoint.snapshot);
    this.clearWatch();
    delete this.journal.current; delete this.journal.checkpoint;
    delete this.journal.ownership; delete this.journal.historical;
    delete this.journal.needsRead; delete this.journal.change;
    delete this.journal.acceptedUpdate;
  }
  private async ready(): Promise<ResourceId> {
    if (this.journal.closing) throw new Error('Finish closing context first');
    if (!this.journal.current) throw new Error('Create or join a context first');
    if (!this.journal.participation) {
      if (this.journal.historical) throw new Error('Historical participation is missing');
      await this.send(['read', this.journal.current.session, 'kernel.session.attach', '1']);
    }
    const participant = this.journal.participation!;
    if (this.journal.needsRead) await this.send(['read', participant, 'editing.context', '1']);
    return participant;
  }
  create(value: context.Value, model = 'context'): Promise<void> {
    return this.run(async () => {
      if (this.journal.current || this.journal.closing) throw new Error('Close the existing context first');
      await this.send(context.initial(model, value));
    });
  }
  join(): Promise<void> {
    return this.run(async () => {
      if (this.journal.current || this.journal.closing) throw new Error('Close the existing context first');
      await this.send(['read', 'session', 'editing.context', '1']);
    });
  }
  set(value: context.Value, correlate?: (receipt: Receipt) => void): Promise<void> {
    return this.run(async () => {
      const participant = await this.ready();
      if (this.journal.historical) throw new Error('Follow live context before changing focus');
      const fields = context.update(this.journal.current!, value);
      fields[1] = participant; await this.send(fields, false, correlate);
    });
  }
  refresh(): Promise<void> {
    return this.run(async () => { await this.send(['read', await this.ready(), 'editing.context', '1']); });
  }
  remember(): Promise<void> {
    return this.run(async () => { await this.send(['read', await this.ready(), 'editing.context', '1'], true); });
  }
  seek(): Promise<void> {
    return this.run(async () => {
      const participant = await this.ready();
      if (!this.journal.checkpoint) throw new Error('Retain a context checkpoint first');
      await this.send(['write', participant, 'kernel.session.seek', '1', '', this.journal.checkpoint.snapshot]);
    });
  }
  follow(): Promise<void> {
    return this.run(async () => {
      const participant = await this.ready();
      if (this.journal.watch && !this.journal.needsRefresh) {
        await this.send(['read', this.journal.watch.id, 'kernel.session.next', '1', '00']);
      }
      if (!this.journal.watch || this.journal.needsRefresh) {
        await this.send(['read', participant, 'kernel.session.follow', '1']);
      }
    });
  }
  private async finishClose(): Promise<void> {
    if (this.journal.participation) {
      await this.send(['write', this.journal.participation, 'kernel.session.detach', '1']);
    }
    if (this.journal.current && this.journal.ownership === 'private') {
      await this.send(['write', this.journal.current.session, 'kernel.session.close', '1']);
    } else this.clearContext();
    this.persist();
  }
  close(): Promise<void> {
    return this.run(async () => { this.journal.closing = true; this.persist(); });
  }
  recover(): Promise<void> {
    return this.run(async () => {
      if (this.journal.receipt) await this.send(this.journal.receipt.fields);
    }, true);
  }
}
