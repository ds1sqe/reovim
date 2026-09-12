import { hex, message, u64, type ResourceId } from '../../../sdk/typescript/resource.js';

export interface Observation {
  access: ResourceId; buffer: ResourceId; history: ResourceId;
  snapshot: ResourceId; watch: ResourceId; revision: string;
}
export type Unit = { scalar: string } | { opaque: number };
export interface Window { start: string; end: string; eof: boolean; units: Unit[] }

export function observation(line: string, access: ResourceId): Observation {
  const reply = message(line);
  if (reply.contract !== 'text.observe' || reply.schema !== 1
      || reply.data.length !== 24 || reply.resources.length !== 6
      || reply.resources[0] !== access || reply.resources[1] !== access) {
    throw new Error('invalid Text observation');
  }
  return { access, buffer: reply.resources[2]!, history: reply.resources[3]!,
    snapshot: reply.resources[4]!, watch: reply.resources[5]!,
    revision: new DataView(reply.data.buffer).getBigUint64(0, true).toString() };
}

export function window(line: string, start: bigint, count: bigint): Window {
  const reply = message(line);
  if (reply.contract !== 'text.window' || reply.schema !== 1
      || reply.resources.length || reply.data.length < 17
      || reply.data[0]! > 1 || (reply.data.length - 17) % 5) {
    throw new Error('invalid Text window');
  }
  const data = new DataView(reply.data.buffer);
  const first = data.getBigUint64(1, true);
  const end = data.getBigUint64(9, true);
  const units: Unit[] = [];
  for (let offset = 17; offset < reply.data.length; offset += 5) {
    const tag = data.getUint8(offset), value = data.getUint32(offset + 1, true);
    if (tag === 0 && value <= 0x10ffff && !(value >= 0xd800 && value <= 0xdfff)) {
      units.push({ scalar: String.fromCodePoint(value) });
    } else if (tag === 1 && value <= 255) { units.push({ opaque: value }); }
    else { throw new Error('invalid Text unit'); }
  }
  if (first !== start || end - first !== BigInt(units.length)
      || BigInt(units.length) > count) throw new Error('foreign Text range');
  return { start: first.toString(), end: end.toString(), eof: reply.data[0] === 1, units };
}

export function range(start: bigint, end: bigint): string {
  if (end < start) throw new Error('reversed Text range');
  return hex(u64(start)) + hex(u64(end));
}

export function replace(value: Observation, start: bigint, end: bigint,
  text: string): string[] {
  // TextEncoder replaces unpaired UTF-16 surrogates; reject before encoding.
  if (/[\uD800-\uDFFF]/u.test(text)) throw new Error('replacement is not Unicode text');
  return ['write', value.access, 'text.replace', '1',
    range(start, end) + hex(new TextEncoder().encode(text)), value.snapshot];
}
