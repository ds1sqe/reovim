import { Client, message, type Receipt, type ResourceId } from '../../../sdk/typescript/resource.js';
import { deliver } from '../../../sdk/typescript/delivery.js';
import * as context from '../../../modules/context/api/typescript.js';

export interface ContextJournal {
  receipt?: Receipt; current?: context.Observation; retiring: ResourceId[];
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
  }
  private send(fields: string[]): Promise<void> {
    return deliver(this.client, this.journal, this.persist, fields, (request, line) => {
      const name = request[2];
      if (name === 'kernel.session.release' || name === 'kernel.session.close') {
        const reply = message(line);
        if (reply.contract !== name || reply.schema !== 1 || reply.data.length || reply.resources.length) {
          throw new Error('invalid context lifecycle result');
        }
        if (name === 'kernel.session.release') {
          this.journal.retiring = this.journal.retiring.filter(id => id !== request[1]);
        } else if (this.journal.current) {
          this.journal.retiring.push(this.journal.current.snapshot);
          delete this.journal.current;
        }
        return;
      }
      const next = context.observation(line), old = this.journal.current;
      if (old && old.session !== next.session) throw new Error('foreign context result');
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
