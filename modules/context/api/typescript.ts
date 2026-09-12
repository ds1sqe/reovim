import { hex, message, u64, type ResourceId } from '../../../sdk/typescript/resource.js';

export interface Value { known: ResourceId[]; active?: ResourceId }
export interface Observation extends Value { session: ResourceId; snapshot: ResourceId }

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
  const known = reply.resources.slice(2);
  const index = new DataView(reply.data.buffer).getBigUint64(0, true);
  if (index !== 0xffffffffffffffffn && index >= BigInt(known.length)) {
    throw new Error('invalid active context index');
  }
  const value = { known, active: index === 0xffffffffffffffffn ? undefined : known[Number(index)] };
  encoded(value);
  return { session: reply.resources[0]!, snapshot: reply.resources[1]!, ...value };
}
