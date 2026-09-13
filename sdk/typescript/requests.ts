import { message, type Outcome } from './resource.js';

/** Decodes the original outcome; retrieval failures remain outside this value. */
export function result(line: string): Outcome {
  const reply = message(line), bytes = reply.data;
  if (reply.contract !== 'kernel.request.result' || reply.schema !== 1 || !bytes.length) {
    throw new Error('invalid retained result');
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const decode = (data: Uint8Array) => new TextDecoder('utf-8', { fatal: true }).decode(data);
  if (bytes[0] === 0) {
    if (bytes.length < 13) throw new Error('truncated retained success');
    const size = view.getBigUint64(5, true);
    if (size === 0n || size > BigInt(bytes.length - 13)) throw new Error('invalid result contract');
    const end = 13 + Number(size);
    return { kind: 'ok', value: { contract: decode(bytes.slice(13, end)),
      schema: view.getUint32(1, true), data: bytes.slice(end), resources: reply.resources } };
  }
  if (![1, 2].includes(bytes[0]!) || bytes.length < 9 || reply.resources.length > 1
      || (bytes[0] === 1 && bytes.length !== 9) || (bytes[0] === 2 && bytes.length === 9)) {
    throw new Error('invalid retained failure');
  }
  return { kind: bytes[0] === 1 ? 'failed' : 'unknown', code: view.getBigUint64(1, true),
    resource: reply.resources[0], recovery: bytes[0] === 2 ? decode(bytes.slice(9)) : '' };
}
