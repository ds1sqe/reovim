import { apply, project, replacement, sameWindow, type Anchor } from '../../clients/gui/contract/input.js';
import type { Window } from '../../modules/text/api/typescript.js';

interface Queued { start: string; end: string; text: string; after: Window }
export interface Draft {
  value: string; phase: 'composition' | 'input' | 'submitted';
  base?: Window; queue?: Queued[];
}
export interface DraftStorage {
  load(): Draft | undefined;
  save(value: Draft | undefined): void;
}

/** Local prediction is input state; only verified server replies advance work. */
export function input(element: HTMLTextAreaElement,
  submit: (start: bigint, end: bigint, text: string) => Promise<Window>,
  undo: () => Promise<void>, failed: (error: unknown) => void,
  storage?: DraftStorage) {
  let draft = storage?.load();
  let queue: Queued[] = draft?.queue ?? [];
  let observed: Window | undefined, basis: Window | undefined;
  let projection = project({ start: '0', end: '0', eof: true, units: [] });
  let anchor: Anchor | undefined;
  let composing = false, finishing = false, pending = false, halted = !!draft;

  const save = (value: Draft | undefined) => { draft = value; storage?.save(value); };
  const retain = (phase: Draft['phase']) => save({ value: element.value, phase,
    base: observed, queue: [...queue] });
  const refresh = () => {
    if (!pending && !queue.length && !composing && !halted) {
      basis = observed;
      if (basis) {
        projection = project(basis);
        if (element.value !== projection.value) element.value = projection.value;
      }
    }
    if (halted && draft) element.value = draft.value;
    element.readOnly = halted || !basis;
  };
  const halt = (error: unknown) => { halted = true; failed(error); refresh(); };

  const drain = async () => {
    if (pending || halted) return;
    pending = true;
    try {
      while (queue.length && !halted) {
        const next = queue[0]!;
        const actual = await submit(BigInt(next.start), BigInt(next.end), next.text);
        if (!sameWindow(actual, next.after)) {
          throw new Error('Accepted view differs from queued basis; input retained');
        }
        queue.shift();
        observed = actual;
        if (halted || composing || queue.length) retain(composing ? 'composition' : 'submitted');
        else save(undefined);
      }
    } catch (error) { halt(error); }
    finally { pending = false; refresh(); }
  };
  const commit = () => {
    if (!anchor || halted || !basis) return;
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
  element.addEventListener('beforeinput', event => {
    if (halted || !basis) { event.preventDefault(); return; }
    if (event.inputType === 'historyUndo' || event.inputType === 'historyRedo') {
      event.preventDefault();
      if (pending || composing) failed(new Error('Finish pending input before History navigation'));
      else if (event.inputType === 'historyUndo') void undo().catch(halt);
      else failed(new Error('Redo input is not yet supported'));
      return;
    }
    if (!composing && !finishing) anchor = selection(event.inputType.endsWith('Backward'));
  });
  element.addEventListener('input', () => {
    if (composing) {
      try { retain('composition'); } catch (error) { halt(error); }
    } else if (!finishing) commit();
  });
  element.addEventListener('compositionstart', () => {
    if (halted || !basis) return;
    composing = true; anchor = selection(false);
  });
  element.addEventListener('compositionend', () => {
    composing = false; finishing = true;
    if (element.value === projection.value) {
      anchor = undefined;
      try { if (queue.length) retain('submitted'); else save(undefined); }
      catch (error) { halt(error); }
    }
    queueMicrotask(() => { finishing = false; commit(); refresh(); });
  });
  return {
    update(window: Window | undefined) { observed = window; refresh(); },
    discard() {
      if (pending || composing) throw new Error('Finish the active input first');
      queue = []; halted = false; save(undefined); refresh();
    },
    get composing() { return composing; },
    get retained() { return !!draft; }
  };
}
