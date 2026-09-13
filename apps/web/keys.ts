import { Keymaps, type Resolution } from '../../clients/gui/core/keymaps.js';

/** DOM/clock adapter; text and composition remain owned by beforeinput/input. */
export function bindKeys(element: HTMLTextAreaElement, maps: Keymaps,
  composing: () => boolean, failed: (error: unknown) => void) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed = false;
  const cancel = () => { if (timer !== undefined) clearTimeout(timer); timer = undefined; };
  const clear = () => { cancel(); maps.clear(); };
  const deliver = (result: Resolution): boolean => {
    cancel();
    if (result.kind === 'pending') {
      timer = setTimeout(expire, Math.max(0, result.deadline - performance.now()));
    } else if (result.kind === 'command') {
      void result.target.invoke().catch(error => { if (!disposed) failed(error); });
    } else if (result.kind === 'ambiguous') failed(new Error('ambiguous key binding'));
    return result.kind !== 'unhandled';
  };
  const expire = () => {
    if (disposed) return;
    try { deliver(maps.expire(['text'], performance.now())); }
    catch (error) { clear(); failed(error); }
  };
  const keydown = (event: KeyboardEvent) => {
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) return;
    if (event.defaultPrevented || event.isComposing || composing()
      || event.key === 'Dead' || event.key === 'Process'
      || event.getModifierState?.('AltGraph')) { clear(); return; }
    try {
      const now = performance.now();
      deliver(maps.expire(['text'], now));
      const result = maps.press({ key: event.key, ctrl: event.ctrlKey,
        alt: event.altKey, shift: event.shiftKey, meta: event.metaKey }, ['text'], now);
      if (deliver(result)) event.preventDefault();
    } catch (error) { clear(); failed(error); }
  };
  element.addEventListener('keydown', keydown);
  element.addEventListener('blur', clear);
  element.addEventListener('compositionstart', clear);
  return { clear, dispose: () => {
    disposed = true; clear();
    element.removeEventListener('keydown', keydown);
    element.removeEventListener('blur', clear);
    element.removeEventListener('compositionstart', clear);
  } };
}
