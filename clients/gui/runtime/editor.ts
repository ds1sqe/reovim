import type { Editor, Selection } from '../contract/commands.js';
import type { ResourceId } from '../../../sdk/typescript/resource.js';
import type { Window } from '../../../modules/text/api/typescript.js';

export interface View { snapshot: ResourceId; window: Window; start: bigint; end: bigint }

/** Composition supplies the view and write capability; modules receive copies. */
export function editor(current: () => View,
  write: (start: bigint, end: bigint, text: string) => Promise<unknown>): Editor {
  const range = (view: View, start: bigint, end: bigint) => {
    if (start < BigInt(view.window.start) || end < start || end > BigInt(view.window.end)) {
      throw new Error('selection outside observed window');
    }
  };
  return {
    selection(): Selection {
      const view = current();
      range(view, view.start, view.end);
      const first = BigInt(view.window.start);
      const units = view.window.units.slice(Number(view.start - first), Number(view.end - first))
        .map(unit => ({ ...unit }));
      return { snapshot: view.snapshot, start: view.start, end: view.end, units };
    },
    async replace(selection, text) {
      const view = current();
      if (selection.snapshot !== view.snapshot) throw new Error('selection is stale');
      range(view, selection.start, selection.end);
      await write(selection.start, selection.end, text);
    }
  };
}
