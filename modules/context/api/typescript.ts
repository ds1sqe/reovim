import { hex, message, u64, type ResourceId } from '../../../sdk/typescript/resource.js';

export interface Value { known: ResourceId[]; active?: ResourceId }
export interface Observation extends Value { session: ResourceId; snapshot: ResourceId }
export interface Watching { observation: Observation; watch: ResourceId; revision: string }
export type Change = { kind: 'pending' | 'gap' } | { kind: 'end'; lost: boolean }
  | { kind: 'item'; request: string; before: string; after: string };

function encoded(value: Value): string {
  if (value.known.length > 254 || new Set(value.known).size !== value.known.length) {
    throw new Error('invalid context references');
  }
  const index = value.active === undefined ? -1 : value.known.indexOf(value.active);
  if (value.active !== undefined && index < 0) throw new Error('active Buffer is not known');
  return hex(u64(index < 0 ? 0xffffffffffffffffn : BigInt(index)));
}
export function initial(model: string, value: Value): string[] {
  const name = new TextEncoder().encode('editing.context.initial');
  const prefix = new Uint8Array(8);
  const header = new DataView(prefix.buffer);
  header.setUint32(0, 1, true); header.setUint32(4, name.length, true);
  const fields = ['write', model, 'kernel.session.create', '1',
    hex(prefix) + hex(name) + encoded(value)];
  // An explicit empty reference field selects root in the service profile.
  if (value.known.length) fields.push(value.known.join(','));
  return fields;
}
export function update(observed: Observation, value: Value): string[] {
  return ['write', observed.session, 'editing.context.update', '1', encoded(value),
    [observed.session, observed.snapshot, ...value.known].join(',')];
}
export function observation(line: string): Observation {
  const reply = message(line);
  if (reply.contract !== 'editing.context' || reply.schema !== 1
      || reply.data.length !== 8 || reply.resources.length < 2) {
    throw new Error('invalid context observation');
  }
  return { session: reply.resources[0]!, snapshot: reply.resources[1]!,
    ...decoded(reply.data, reply.resources.slice(2)) };
}
function decoded(data: Uint8Array, known: ResourceId[]): Value {
  if (data.length !== 8) throw new Error('invalid context value');
  const index = new DataView(data.buffer, data.byteOffset, data.byteLength).getBigUint64(0, true);
  if (index !== 0xffffffffffffffffn && index >= BigInt(known.length)) {
    throw new Error('invalid active context index');
  }
  const value = { known, active: index === 0xffffffffffffffffn ? undefined : known[Number(index)] };
  encoded(value);
  return value;
}
export function watching(line: string): Watching {
  return watchReply(line, 'kernel.session.observe');
}
export function followed(line: string): Watching {
  return watchReply(line, 'kernel.session.follow');
}
function watchReply(line: string, contract: string): Watching {
  const reply = message(line);
  if (reply.contract !== contract || reply.schema !== 1
      || reply.data.length < 16 || reply.resources.length < 3 || reply.resources.length > 256) {
    throw new Error('invalid context watch');
  }
  const data = new DataView(reply.data.buffer);
  const end = 16 + data.getUint32(12, true);
  const name = new TextDecoder('utf-8', { fatal: true }).decode(reply.data.subarray(16, end));
  if (end + 8 !== reply.data.length || data.getUint32(8, true) !== 1 || name !== 'editing.context') {
    throw new Error('invalid watched context schema');
  }
  return { watch: reply.resources[2]!, revision: data.getBigUint64(0, true).toString(),
    observation: { session: reply.resources[0]!, snapshot: reply.resources[1]!,
      ...decoded(reply.data.subarray(end), reply.resources.slice(3)) } };
}
export function change(line: string): Change {
  const reply = message(line), bytes = reply.data;
  if (reply.contract !== 'kernel.session.next' || reply.schema !== 1 || reply.resources.length) {
    throw new Error('invalid context delivery');
  }
  if (bytes.length === 1 && bytes[0] === 1) return { kind: 'gap' };
  if (bytes.length === 1 && bytes[0] === 3) return { kind: 'pending' };
  if (bytes.length === 2 && bytes[0] === 2 && bytes[1]! <= 1) return { kind: 'end', lost: bytes[1] === 1 };
  if (bytes.length !== 25 || bytes[0] !== 0) throw new Error('invalid context event');
  const data = new DataView(bytes.buffer);
  return { kind: 'item', request: data.getBigUint64(1, true).toString(),
    before: data.getBigUint64(9, true).toString(), after: data.getBigUint64(17, true).toString() };
}
