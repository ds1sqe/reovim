// Declarations and values, as kernel/interface/src/decl.rs and value.rs
// and sdk/rust/src/dynamic.rs have them: a declaration parsed from its
// canonical bytes, a value decoded by its type, shown on one line.

import { Reader } from './codec.js';

export type Ty =
  | { tag: 'unit' | 'bool' | 'u8' | 'u32' | 'u64' | 'i64' | 'f64' | 'rd' }
  | { tag: 'bytes' | 'utf8'; max: number }
  | { tag: 'record' | 'choice'; name: string; fields: Field[] }
  | { tag: 'sequence' | 'array'; n: number; item: Ty };

export interface Field {
  name: string;
  ty: Ty;
}

export interface Method {
  number: number;
  name: string;
  input_name?: string;
  input: Ty;
  output: Ty;
  right: number;
}

export interface Decl {
  api: string;
  ver: number;
  methods: Method[];
  /** The records the api names, each with its fields, in the order
   * declared: an api of records and no method is its records. */
  records: [string, Field[]][];
}

const TAGS = [
  'unit', 'bool', 'u8', 'u32', 'u64', 'i64', 'f64', 'bytes', 'utf8',
  'record', 'sequence', 'array', 'choice', 'rd',
] as const;

function readTy(r: Reader): Ty {
  const tag = TAGS[r.u8()];
  if (tag === undefined) throw new RangeError('type tag');
  switch (tag) {
    case 'bytes': case 'utf8': return { tag, max: r.u32() };
    case 'record': case 'choice': return { tag, name: r.str(), fields: readFields(r) };
    case 'sequence': case 'array': {
      const n = r.u32();
      return { tag, n, item: readTy(r) };
    }
    default: return { tag };
  }
}

function readFields(r: Reader): Field[] {
  const n = r.u32();
  const fields: Field[] = [];
  for (let i = 0; i < n; i += 1) {
    const name = r.str();
    fields.push({ name, ty: readTy(r) });
  }
  return fields;
}

/** The declaration whose canonical bytes these are. */
export function parseDecl(bytes: Uint8Array): Decl {
  const r = new Reader(bytes);
  const api = r.str(), ver = r.u32(), n = r.u32();
  const methods: Method[] = [];
  for (let i = 0; i < n; i += 1) {
    const number = r.u16(), name = r.str();
    const input_name = r.bool() ? r.str() : undefined;
    const input = readTy(r), output = readTy(r), right = r.u32();
    methods.push({ number, name, input_name, input, output, right });
  }
  const records: [string, Field[]][] = [];
  const count = r.u32();
  for (let i = 0; i < count; i += 1) records.push([r.str(), readFields(r)]);
  if (!r.done()) throw new RangeError('trailing bytes');
  return { api, ver, methods, records };
}

export type Value =
  | { kind: 'unit' }
  | { kind: 'bool'; value: boolean }
  | { kind: 'number'; value: number }
  | { kind: 'big'; value: bigint }
  | { kind: 'bytes'; value: Uint8Array }
  | { kind: 'text'; value: string }
  | { kind: 'record'; fields: [string, Value][] }
  | { kind: 'sequence'; items: Value[] }
  | { kind: 'choice'; name: string; value: Value };

function readValue(r: Reader, ty: Ty): Value {
  switch (ty.tag) {
    case 'unit': return { kind: 'unit' };
    case 'bool': return { kind: 'bool', value: r.bool() };
    case 'u8': return { kind: 'number', value: r.u8() };
    case 'u32': return { kind: 'number', value: r.u32() };
    case 'u64': case 'rd': return { kind: 'big', value: r.u64() };
    case 'i64': return { kind: 'big', value: r.i64() };
    case 'f64': return { kind: 'number', value: r.f64() };
    case 'bytes': return { kind: 'bytes', value: r.bytes() };
    case 'utf8': return { kind: 'text', value: r.str() };
    case 'record': {
      const fields: [string, Value][] = [];
      for (const f of ty.fields) fields.push([f.name, readValue(r, f.ty)]);
      return { kind: 'record', fields };
    }
    case 'sequence': case 'array': {
      const n = ty.tag === 'sequence' ? r.u32() : ty.n;
      const items: Value[] = [];
      for (let i = 0; i < n; i += 1) items.push(readValue(r, ty.item));
      return { kind: 'sequence', items };
    }
    case 'choice': {
      const field = ty.fields[r.u8()];
      if (field === undefined) throw new RangeError('choice tag');
      return { kind: 'choice', name: field.name, value: readValue(r, field.ty) };
    }
  }
}

/** `bytes` as one value of `ty`; undefined unless exactly one. */
export function decodeValue(ty: Ty, bytes: Uint8Array): Value | undefined {
  try {
    const r = new Reader(bytes);
    const value = readValue(r, ty);
    return r.done() ? value : undefined;
  } catch {
    return undefined;
  }
}

/** The field named `name` of a record. */
export function field(value: Value, name: string): Value | undefined {
  if (value.kind !== 'record') return undefined;
  return value.fields.find(([n]) => n === name)?.[1];
}

/** One line: a scalar as itself, `{a=1 b=x}`, `[1 2]`, `name(value)`. */
export function show(value: Value): string {
  switch (value.kind) {
    case 'unit': return '';
    case 'bool': case 'number': case 'big': return String(value.value);
    case 'bytes': return new TextDecoder().decode(value.value);
    case 'text': return value.value;
    case 'record':
      return `{${value.fields.map(([n, v]) => `${n}=${show(v)}`).join(' ')}}`;
    case 'sequence': return `[${value.items.map(show).join(' ')}]`;
    case 'choice': return `${value.name}(${show(value.value)})`;
  }
}
