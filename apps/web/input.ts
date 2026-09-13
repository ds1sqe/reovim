import { apply, project, replacement, sameWindow, type Anchor } from '../../clients/gui/contract/input.js';
import type { Window } from '../../modules/text/api/typescript.js';
import { Keymaps } from '../../clients/gui/core/keymaps.js';
import { Commands } from '../../clients/gui/runtime/commands.js';
import { bindKeys } from './keys.js';
import { historyBindings } from '../../clients/gui/modules/history/keymaps.js';
import type { RegisterKeymap } from '../../clients/gui/contract/keymaps.js';

interface Queued { start: string; end: string; text: string; after: Window }
export interface Draft {
  value: string; phase: 'composition' | 'input' | 'submitted';
  base?: Window; queue?: Queued[];
  proof?: { client: string; id: string };
}
export interface DraftStorage {
  load(): Draft | undefined;
  save(value: Draft | undefined): void;
}

/** Local prediction is input state; only verified server replies advance work. */
export function input(element: HTMLTextAreaElement,
  submit: (start: bigint, end: bigint, text: string) => Promise<Window>,
  history: (direction: 'undo' | 'redo') => Promise<void>, failed: (error: unknown) => void,
  storage?: DraftStorage, commands = new Commands()) {
  let draft = storage?.load();
  let proof = draft?.proof;
  let queue: Queued[] = draft?.queue ?? [];
  let observed: Window | undefined, basis: Window | undefined;
  let projection = project({ start: '0', end: '0', eof: true, units: [] });
  let anchor: Anchor | undefined;
  let composing = false, finishing = false, pending = false, halted = !!draft;
  let historyPending = false;
  let disposed = false;
  let suspended = false;
  const listeners: (() => void)[] = [];
  const on = <K extends keyof HTMLElementEventMap>(name: K,
    listener: (event: HTMLElementEventMap[K]) => void) => {
    element.addEventListener(name, listener);
    listeners.push(() => element.removeEventListener(name, listener));
  };

  const save = (value: Draft | undefined) => { draft = value; storage?.save(value); };
  const retain = (phase: Draft['phase']) => save({ value: element.value, phase,
    base: observed, queue: [...queue], ...(proof ? { proof } : {}) });
  const refresh = () => {
    if (disposed) return;
    if (!pending && !historyPending && !queue.length && !composing && !halted) {
      basis = observed;
      if (basis) {
        projection = project(basis);
        if (element.value !== projection.value) element.value = projection.value;
      } else element.value = '';
    }
    if (halted && draft) element.value = draft.value;
    element.readOnly = suspended || halted || !basis || historyPending;
  };
  const halt = (error: unknown) => { halted = true; failed(error); refresh(); };
  const report = (error: unknown) => { if (!disposed) failed(error); };

  const drain = async () => {
    if (disposed || pending || halted) return;
    pending = true;
    try {
      while (queue.length && !halted) {
        const next = queue[0]!;
        const actual = await submit(BigInt(next.start), BigInt(next.end), next.text);
        // Transport owns admitted work; a retired view must not adopt its reply.
        if (disposed) return;
        if (!sameWindow(actual, next.after)) {
          throw new Error('Accepted view differs from queued basis; input retained');
        }
        queue.shift();
        proof = undefined;
        observed = actual;
        if (halted || composing || queue.length) retain(composing ? 'composition' : 'submitted');
        else save(undefined);
      }
    } catch (error) { if (!disposed) halt(error); }
    finally { pending = false; refresh(); }
  };
  const commit = () => {
    if (disposed || !anchor || halted || !basis) return;
    const selected = anchor;
    anchor = undefined;
    try {
      retain('input');
      const edit = replacement(projection, element.value, selected);
      if (!edit) {
        if (!queue.length) save(undefined);
        refresh(); return;
      }
      if (queue.length === 32) throw new Error('Input queue capacity; draft retained');
      const after = apply(basis, edit);
      queue.push({ start: edit.start.toString(), end: edit.end.toString(), text: edit.text, after });
      basis = after; projection = project(after);
      retain('submitted');
      void drain();
    } catch (error) { halt(error); }
  };
  const selection = (backward: boolean): Anchor => ({
    start: element.selectionStart, end: element.selectionEnd, backward
  });
  const navigate = async (direction: 'undo' | 'redo') => {
    if (suspended) throw new Error('Finish the view change first');
    if (disposed || halted || !basis) throw new Error('Resolve retained input first');
    if (pending || historyPending || composing || queue.length) {
      throw new Error('Finish pending input before History navigation');
    }
    historyPending = true; refresh();
    try { await history(direction); }
    finally { historyPending = false; refresh(); }
  };
  const maps = new Keymaps();
  const registerKeymap: RegisterKeymap = binding => {
    if (disposed) throw new Error('input adapter disposed');
    return maps.register(binding, commands.target(binding.command));
  };
  historyBindings(binding => maps.register(binding, { available: () => !disposed,
    invoke: () => navigate(binding.command === 'history.undo' ? 'undo' : 'redo') }));
  const keys = bindKeys(element, maps, () => composing || finishing, report);
  on('beforeinput', event => {
    if (suspended || halted || historyPending || !basis) { event.preventDefault(); return; }
    if (event.inputType === 'historyUndo' || event.inputType === 'historyRedo') {
      event.preventDefault();
      void navigate(event.inputType === 'historyUndo' ? 'undo' : 'redo').catch(report);
      return;
    }
    if (!composing && !finishing) anchor = selection(event.inputType.endsWith('Backward'));
  });
  on('input', () => {
    if (composing) {
      try { retain('composition'); } catch (error) { halt(error); }
    } else if (!finishing) {
      if (!anchor && element.value !== projection.value) {
        try { retain('input'); halt(new Error('Input selection unavailable; draft retained')); }
        catch (error) { halt(error); }
      } else commit();
    }
  });
  on('compositionstart', () => {
    if (suspended || halted || !basis) return;
    composing = true; anchor = selection(false);
  });
  on('compositionend', () => {
    composing = false; finishing = true;
    if (element.value === projection.value) {
      anchor = undefined;
      try { if (queue.length) retain('submitted'); else save(undefined); }
      catch (error) { halt(error); }
    }
    queueMicrotask(() => { finishing = false; commit(); refresh(); });
  });
  return {
    registerKeymap,
    history: navigate,
    correlate(client: string, id: string) {
      if (!pending || !queue.length) throw new Error('No input is awaiting correlation');
      proof = { client, id }; retain('submitted');
    },
    get resumeRequest() {
      if (disposed || !halted || pending || composing || !proof || !queue.length
          || draft?.phase !== 'submitted'
          || project(queue[queue.length - 1]!.after).value !== draft.value) {
        throw new Error('Queue needs manual reconciliation');
      }
      return { ...proof, after: queue[0]!.after };
    },
    resume(client: string, id: string, actual: Window) {
      const request = this.resumeRequest;
      if (request.client !== client || request.id !== id || !sameWindow(actual, request.after)) {
        throw new Error('Original queue acceptance does not match');
      }
      queue.shift(); proof = undefined; observed = actual;
      basis = queue.length ? queue[queue.length - 1]!.after : actual;
      projection = project(basis); halted = false;
      if (queue.length) retain('submitted'); else save(undefined);
      refresh(); void drain();
    },
    update(window: Window | undefined) {
      if (window !== observed) keys.clear();
      observed = window; refresh();
    },
    suspend(value: boolean) { if (value) keys.clear(); suspended = value; refresh(); },
    discard() {
      if (disposed || pending || historyPending || composing) throw new Error('Finish the active input first');
      queue = []; proof = undefined; halted = false; save(undefined); refresh();
    },
    dispose() {
      keys.dispose();
      disposed = true; halted = true;
      for (const remove of listeners.splice(0)) remove();
    },
    get composing() { return composing; },
    get retained() { return !!draft; }
  };
}
