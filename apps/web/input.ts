import { project, replacement, type Anchor } from '../../clients/gui/contract/input.js';
import type { Window } from '../../modules/text/api/typescript.js';

/** DOM adapter: composing text remains a local draft until composition ends. */
export function input(element: HTMLTextAreaElement,
  submit: (start: bigint, end: bigint, text: string) => Promise<void>,
  undo: () => Promise<void>, failed: (error: unknown) => void) {
  let projection = project({ start: '0', end: '0', eof: true, units: [] });
  let anchor: Anchor | undefined;
  let composing = false, finishing = false, pending = false, available = false;
  const selection = (backward: boolean): Anchor => ({
    start: element.selectionStart, end: element.selectionEnd, backward
  });
  const commit = async () => {
    if (!anchor || pending || !available) return;
    const selected = anchor;
    anchor = undefined;
    const draft = element.value;
    try {
      const edit = replacement(projection, element.value, selected);
      if (!edit) return;
      pending = true; element.readOnly = true;
      const caret = element.selectionStart;
      await submit(edit.start, edit.end, edit.text);
      element.setSelectionRange(caret, caret);
    } catch (error) {
      failed(error);
      element.value = draft;
      available = false;
    }
    finally { pending = false; element.readOnly = !available; }
  };
  element.addEventListener('beforeinput', event => {
    if (pending || !available) { event.preventDefault(); return; }
    if (event.inputType === 'historyUndo' || event.inputType === 'historyRedo') {
      event.preventDefault();
      if (event.inputType === 'historyUndo') void undo().catch(failed);
      return;
    }
    if (!composing && !finishing) anchor = selection(event.inputType.endsWith('Backward'));
  });
  element.addEventListener('input', () => {
    if (!composing && !finishing) void commit();
  });
  element.addEventListener('compositionstart', () => {
    composing = true; anchor = selection(false);
  });
  element.addEventListener('compositionend', () => {
    composing = false; finishing = true;
    if (element.value === projection.value) anchor = undefined;
    queueMicrotask(() => { finishing = false; void commit(); });
  });
  return {
    update(window: Window | undefined) {
      available = !!window;
      if (window && !composing) {
        projection = project(window);
        element.value = projection.value;
      }
      element.readOnly = pending || !available;
    },
    get composing() { return composing; }
  };
}
