import { Client, message, type Receipt, type ResourceId } from '../../../sdk/typescript/resource.js';
import { deliver } from '../../../sdk/typescript/delivery.js';
import * as context from '../../../modules/context/api/typescript.js';

export interface ContextJournal {
  receipt?: Receipt; current?: context.Observation; retiring: ResourceId[];
  watch?: { id: ResourceId; session: ResourceId; revision: string };
  retiringWatches?: ResourceId[]; change?: context.Change; needsRefresh?: boolean;
}

/** Private focus owns Session snapshots, never the referenced Buffers. */
export class Focus {
  private busy = false;
  constructor(private readonly client: Client, public readonly journal: ContextJournal,
    private readonly persist: () => void) {}

  private async run(action: () => Promise<void>, recovery = false): Promise<void> {
    if (this.busy) throw new Error('context operation pending');
    this.busy = true;
    try {
      if (this.journal.receipt && !recovery) throw new Error('Recover context work first');
      if (!recovery) await this.retire();
      await action();
      await this.retire();
    } finally { this.busy = false; }
  }
  private async retire(): Promise<void> {
    while (this.journal.retiring.length) {
      await this.send(['write', this.journal.retiring[0]!, 'kernel.session.release', '1']);
    }
    while (this.journal.retiringWatches?.length) {
      await this.send(['close', this.journal.retiringWatches[0]!]);
    }
  }
  private send(fields: string[]): Promise<void> {
    return deliver(this.client, this.journal, this.persist, fields, (request, line) => {
      if (request[0] === 'close' && line === 'ok') {
        this.journal.retiringWatches = this.journal.retiringWatches?.filter(id => id !== request[1]);
        return;
      }
      const name = request[2];
      if (name === 'kernel.session.next') {
        if (!this.journal.watch || this.journal.watch.id !== request[1]
            || this.journal.watch.session !== this.journal.current?.session) throw new Error('foreign context watch');
        this.journal.change = context.change(line);
        this.journal.needsRefresh = ['item', 'gap'].includes(this.journal.change.kind);
        return;
      }
      if (name === 'kernel.session.release' || name === 'kernel.session.close') {
        const reply = message(line);
        if (reply.contract !== name || reply.schema !== 1 || reply.data.length || reply.resources.length) {
          throw new Error('invalid context lifecycle result');
        }
        if (name === 'kernel.session.release') {
          this.journal.retiring = this.journal.retiring.filter(id => id !== request[1]);
        } else if (this.journal.current) {
          if (request[1] !== this.journal.current.session) throw new Error('foreign context close');
          this.journal.retiring.push(this.journal.current.snapshot);
          if (this.journal.watch) (this.journal.retiringWatches ??= []).push(this.journal.watch.id);
          delete this.journal.watch; delete this.journal.change; delete this.journal.needsRefresh;
          delete this.journal.current;
        }
        return;
      }
      const watched = name === 'kernel.session.observe' ? context.watching(line) : undefined;
      const next = watched?.observation ?? context.observation(line), old = this.journal.current;
      if (old && old.session !== next.session) throw new Error('foreign context result');
      if (watched) {
        const previous = this.journal.watch;
        if (previous && previous.id !== watched.watch) (this.journal.retiringWatches ??= []).push(previous.id);
        this.journal.watch = { id: watched.watch, session: next.session, revision: watched.revision };
        this.journal.needsRefresh = false;
      }
      if (old && old.snapshot !== next.snapshot) this.journal.retiring.push(old.snapshot);
      this.journal.current = next;
    });
  }
  create(value: context.Value, model = 'context'): Promise<void> {
    return this.run(async () => {
      if (this.journal.current) throw new Error('context already exists');
      await this.send(context.initial(model, value));
    });
  }
  set(value: context.Value): Promise<void> {
    return this.run(async () => {
      if (!this.journal.current) throw new Error('Create a context first');
      await this.send(context.update(this.journal.current, value));
    });
  }
  refresh(): Promise<void> {
    return this.run(async () => {
      if (!this.journal.current) throw new Error('Create a context first');
      await this.send(['read', this.journal.current.session, 'editing.context', '1']);
    });
  }
  follow(): Promise<void> {
    return this.run(async () => {
      if (!this.journal.current) throw new Error('Create a context first');
      if (this.journal.watch && !this.journal.needsRefresh) {
        await this.send(['read', this.journal.watch.id, 'kernel.session.next', '1', '00']);
      }
      if (!this.journal.watch || this.journal.needsRefresh) {
        await this.send(['read', this.journal.current.session, 'kernel.session.observe', '1']);
      }
    });
  }
  close(): Promise<void> {
    return this.run(async () => {
      if (this.journal.current) {
        await this.send(['write', this.journal.current.session, 'kernel.session.close', '1']);
      }
    });
  }
  recover(): Promise<void> {
    return this.run(async () => {
      if (this.journal.receipt) await this.send(this.journal.receipt.fields);
    }, true);
  }
}
