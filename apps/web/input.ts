import { project, replacement, type Anchor } from '../../clients/gui/contract/input.js';
import type { Window } from '../../modules/text/api/typescript.js';

export interface Draft { value: string; phase: 'composition' | 'input' | 'submitted' }
export interface DraftStorage {
  load(): Draft | undefined;
  save(value: Draft | undefined): void;
}

/** DOM adapter: composing text remains a local draft until composition ends. */
export function input(element: HTMLTextAreaElement,
  submit: (start: bigint, end: bigint, text: string) => Promise<void>,
  undo: () => Promise<void>, failed: (error: unknown) => void,
  storage?: DraftStorage) {
  let projection = project({ start: '0', end: '0', eof: true, units: [] });
  let draft = storage?.load();
  let observed: Window | undefined;
  const save = (value: Draft | undefined) => {
    draft = value;
    storage?.save(value);
  };
  const update = (window: Window | undefined) => {
    observed = window;
    available = !!window;
    if (window && !composing) projection = project(window);
    if (!composing) element.value = draft?.value ?? projection.value;
    element.readOnly = pending || !available || (!!draft && !composing);
  };
  let anchor: Anchor | undefined;
  let composing = false, finishing = false, pending = false, available = false;
  const selection = (backward: boolean): Anchor => ({
    start: element.selectionStart, end: element.selectionEnd, backward
  });
  const commit = async () => {
    if (!anchor || pending || !available) return;
    const selected = anchor;
    anchor = undefined;
    const value = element.value;
    try {
      save({ value, phase: 'input' });
      const edit = replacement(projection, element.value, selected);
      if (!edit) { save(undefined); update(observed); return; }
      save({ value, phase: 'submitted' });
      pending = true; element.readOnly = true;
      const caret = element.selectionStart;
      await submit(edit.start, edit.end, edit.text);
      save(undefined);
      update(observed);
      element.setSelectionRange(caret, caret);
    } catch (error) {
      failed(error);
      element.value = value;
      available = false;
    }
    finally { pending = false; element.readOnly = !available || !!draft; }
  };
  element.addEventListener('beforeinput', event => {
    if (pending || !available || (draft && !composing && !finishing)) {
      event.preventDefault(); return;
    }
    if (event.inputType === 'historyUndo' || event.inputType === 'historyRedo') {
      event.preventDefault();
      if (event.inputType === 'historyUndo') void undo().catch(failed);
      return;
    }
    if (!composing && !finishing) anchor = selection(event.inputType.endsWith('Backward'));
  });
  element.addEventListener('input', () => {
    if (composing) {
      try { save({ value: element.value, phase: 'composition' }); }
      catch (error) { failed(error); }
    }
    if (!composing && !finishing) void commit();
  });
  element.addEventListener('compositionstart', () => {
    composing = true; anchor = selection(false);
  });
  element.addEventListener('compositionend', () => {
    composing = false; finishing = true;
    if (element.value === projection.value) {
      anchor = undefined;
      try { save(undefined); } catch (error) { failed(error); }
    }
    queueMicrotask(() => { finishing = false; void commit(); });
  });
  return {
    update,
    discard() {
      if (pending || composing) throw new Error('Finish the active input first');
      save(undefined);
      update(observed);
    },
    get composing() { return composing; },
    get retained() { return !!draft; }
  };
}
