import type { Editor, RegisterCommand, Registration } from '../../contract/commands.js';
import type { RegisterKeymap } from '../../contract/keymaps.js';

/** Example author contribution: no DOM, transport or concrete Runtime imports. */
export function activate(editor: Editor, register: RegisterCommand,
  keymap?: RegisterKeymap): Registration {
  const command = register('text.uppercase', async () => {
    const selection = editor.selection();
    if (selection.start === selection.end) return;
    const text = selection.units.map(unit => {
      if ('opaque' in unit) throw new Error('uppercase requires a scalar-only selection');
      return unit.scalar;
    }).join('');
    await editor.replace(selection, text.toUpperCase());
  });
  let keys: Registration | undefined;
  try {
    keys = keymap?.({ sequence: [{ key: 'k', ctrl: true }, { key: 'u', ctrl: true }],
      scope: 'text', priority: 0, command: 'text.uppercase' });
  } catch (error) { command.close(); throw error; }
  return { close: () => { keys?.close(); command.close(); } };
}
