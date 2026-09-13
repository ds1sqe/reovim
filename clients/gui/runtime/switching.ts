import type { Receipt, ResourceId } from '../../../sdk/typescript/resource.js';
import type { Value } from '../../../modules/context/api/typescript.js';
import type { BufferViews } from '../contract/buffers.js';
import { Focus } from './context.js';

export interface SwitchJournal {
  pending?: { session: ResourceId; value: Value; phase: 'focus' | 'view';
    closing?: ResourceId;
    request?: Pick<Receipt, 'client' | 'id'> };
}

/** Context acceptance precedes view acquisition; failures keep both owners. */
export class Switching {
  private busy = false;
  constructor(private readonly focus: Focus, private readonly views: BufferViews,
    public readonly journal: SwitchJournal, private readonly persist: () => void) {}

  private async run(action: () => Promise<void>): Promise<void> {
    if (this.busy) throw new Error('Buffer switch is running');
    this.busy = true;
    try { await action(); } finally { this.busy = false; }
  }

  select(target?: ResourceId): Promise<void> {
    return this.run(async () => {
      if (this.journal.pending?.phase === 'focus' || this.journal.pending?.closing) throw new Error('Recover the context switch first');
      if (this.journal.pending) {
        await this.focus.recover(); await this.views.recover();
      }
      const current = this.focus.journal.current;
      if (this.focus.journal.receipt || this.focus.journal.closing) throw new Error('Recover context work first');
      if (!current || this.focus.journal.historical) throw new Error('Select a live context first');
      const owned = this.views.known();
      if (target && !owned.includes(target)) throw new Error('That Buffer has no owned view');
      this.journal.pending = { session: current.session, phase: 'focus',
        value: { known: [...new Set([...current.known, ...owned])], active: target } };
      this.persist();
      await this.update();
      await this.show();
    });
  }

  close(target: ResourceId): Promise<void> {
    return this.run(async () => {
      if (this.views.current() !== target) throw new Error('Close the current Buffer first');
      if (this.journal.pending || this.focus.journal.receipt || this.focus.journal.closing) {
        throw new Error('Recover context work first');
      }
      const current = this.focus.journal.current;
      if (!current || this.focus.journal.historical) throw new Error('Select a live context first');
      const owned = this.views.known();
      if (!owned.includes(target)) throw new Error('That Buffer has no owned view');
      this.journal.pending = { session: current.session, phase: 'focus', closing: target,
        value: { known: [...new Set([...current.known, ...owned])].filter(id => id !== target),
          active: current.active === target ? undefined : current.active } };
      this.persist();
      await this.update();
      await this.show();
    });
  }

  private accepted(): boolean {
    const request = this.journal.pending?.request, accepted = this.focus.journal.acceptedUpdate;
    return !!request && request.client === accepted?.client && request.id === accepted.id;
  }

  private async update(): Promise<void> {
    const pending = this.journal.pending!;
    try {
      await this.focus.set(pending.value, request => {
        pending.request = { client: request.client, id: request.id };
        this.persist();
      });
    } catch (error) {
      if (this.accepted()) pending.phase = 'view';
      else if (!this.focus.journal.receipt) delete this.journal.pending;
      this.persist(); throw error;
    }
    if (!this.accepted()) throw new Error('Context acceptance is unavailable');
    pending.phase = 'view'; this.persist();
  }

  private async show(): Promise<void> {
    const pending = this.journal.pending!;
    if (this.focus.journal.current?.session !== pending.session) throw new Error('Context changed during switch');
    if (pending.closing) {
      try { await this.views.close(pending.closing); }
      catch (error) { this.failedView(error); }
    } else await this.views.show(pending.value.active);
    delete this.journal.pending; this.persist();
  }

  private failedView(error: unknown): never {
    // With no recovery work left, end this decision without another close.
    if (this.journal.pending?.closing && !this.views.pending()) {
      delete this.journal.pending; this.persist();
    }
    throw error;
  }

  recover(): Promise<void> {
    return this.run(async () => {
      if (!this.journal.pending) return;
      try { await this.focus.recover(); }
      catch (error) {
        if (this.accepted()) this.journal.pending.phase = 'view';
        else if (!this.focus.journal.receipt) delete this.journal.pending;
        this.persist(); throw error;
      }
      const pending = this.journal.pending;
      if (pending.phase === 'focus') {
        if (!pending.request) await this.update();
        else if (this.accepted()) { pending.phase = 'view'; this.persist(); }
        else throw new Error('Original context acceptance is unavailable');
      }
      try { await this.views.recover(); }
      catch (error) { this.failedView(error); }
      await this.show();
    });
  }
}
