import type { Editor, RegisterCommand, Registration } from '../../contract/commands.js';

/** Example author contribution: no DOM, transport or concrete Runtime imports. */
export function activate(editor: Editor, register: RegisterCommand): Registration {
  return register('text.uppercase', async () => {
    const selection = editor.selection();
    const text = selection.units.map(unit => {
      if ('opaque' in unit) throw new Error('uppercase requires a scalar-only selection');
      return unit.scalar;
    }).join('');
    await editor.replace(selection, text.toUpperCase());
  });
}
