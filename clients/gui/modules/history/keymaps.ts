import type { RegisterKeymap } from '../../contract/keymaps.js';

/** Default History policy; the device adapter supplies only logical keys. */
export function historyBindings(register: RegisterKeymap): void {
  for (const modifier of ['ctrl', 'meta'] as const) {
    for (const shift of [false, true]) {
      register({ sequence: [{ key: 'z', [modifier]: true, shift }],
        scope: '', priority: 0, command: shift ? 'history.redo' : 'history.undo' });
    }
  }
  register({ sequence: [{ key: 'y', ctrl: true }], scope: '', priority: 0, command: 'history.redo' });
}
