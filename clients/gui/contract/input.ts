import type { Window } from '../../../modules/text/api/typescript.js';

/** One bounded Text window projected into a browser's normalized string. */
export interface Projection {
  value: string; characters: string[]; positions: bigint[];
}
export interface Anchor { start: number; end: number; backward: boolean }
export interface Replacement { start: bigint; end: bigint; text: string }

/** Predict only this bounded view; the server still decides acceptance. */
export function apply(window: Window, edit: Replacement): Window {
  const first = BigInt(window.start);
  const start = edit.start - first, end = edit.end - first;
  if (start < 0n || end < start || end > BigInt(window.units.length)
      || /[\uD800-\uDFFF]/u.test(edit.text)) throw new Error('invalid local edit');
  const units = [...window.units.slice(0, Number(start)),
    ...[...edit.text].map(scalar => ({ scalar })), ...window.units.slice(Number(end))];
  if (units.length > 1024) throw new Error('input window capacity; draft retained');
  return { start: window.start, end: (first + BigInt(units.length)).toString(),
    eof: window.eof, units };
}

export function sameWindow(left: Window, right: Window): boolean {
  return left.start === right.start && left.end === right.end && left.eof === right.eof
    && left.units.length === right.units.length && left.units.every((unit, index) => {
      const other = right.units[index]!;
      return 'scalar' in unit ? 'scalar' in other && unit.scalar === other.scalar
        : 'opaque' in other && unit.opaque === other.opaque;
    });
}

export function project(window: Window): Projection {
  const characters: string[] = [], positions = [BigInt(window.start)];
  for (let index = 0; index < window.units.length; index++) {
    const unit = window.units[index]!;
    let character = 'scalar' in unit ? unit.scalar : '\ufffc';
    if (character === '\r') {
      const next = window.units[index + 1];
      if (next && 'scalar' in next && next.scalar === '\n') index++;
      character = '\n';
    }
    characters.push(character);
    positions.push(BigInt(window.start) + BigInt(index + 1));
  }
  return { value: characters.join(''), characters, positions };
}

function position(characters: string[], offset: number): number {
  let consumed = 0;
  for (let index = 0; index <= characters.length; index++) {
    if (consumed === offset) return index;
    consumed += characters[index]?.length ?? 0;
  }
  throw new Error('selection splits a scalar or leaves the observed window');
}

/** Browser selection offsets are UTF-16; returned endpoints belong to Text. */
export function selectedRange(window: Window, value: string, start: number, end: number): {
  start: bigint; end: bigint
} {
  if (end < start) throw new Error('reversed selection');
  const view = project(window);
  if (value !== view.value) throw new Error('selection view differs from observed Text');
  return { start: view.positions[position(view.characters, start)]!,
    end: view.positions[position(view.characters, end)]! };
}

/** Selection anchoring disambiguates equal characters and normalized newlines. */
export function replacement(old: Projection, value: string, anchor: Anchor): Replacement | undefined {
  const next = [...value];
  const start = position(old.characters, anchor.start);
  const end = position(old.characters, anchor.end);
  if (end < start) throw new Error('reversed selection');
  let prefix = 0, suffix = 0;
  const head = () => {
    while (prefix < start && prefix < next.length - suffix
        && old.characters[prefix] === next[prefix]) prefix++;
  };
  const tail = () => {
    while (suffix < old.characters.length - end && suffix < next.length - prefix
        && old.characters[old.characters.length - suffix - 1] === next[next.length - suffix - 1]) suffix++;
  };
  if (anchor.backward) { tail(); head(); } else { head(); tail(); }
  const last = old.characters.length - suffix;
  const text = next.slice(prefix, next.length - suffix).join('');
  if (prefix === last && !text) return;
  return { start: old.positions[prefix]!, end: old.positions[last]!, text };
}
