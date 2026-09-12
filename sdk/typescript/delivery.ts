import { Client, outcome, type Receipt } from './resource.js';

export interface Journal { receipt?: Receipt }

/** Adoption must be idempotent: a journal may resume after receipt retirement. */
export async function deliver(client: Client, journal: Journal,
  persist: () => void, fields: string[],
  adopt: (fields: string[], line: string) => void,
  correlate?: (receipt: Receipt) => void): Promise<void> {
  if (!journal.receipt) {
    journal.receipt = await client.reserve(fields);
  }
  const receipt = journal.receipt;
  correlate?.(receipt);
  persist();
  receipt.response = await client.send(receipt);
  persist();
  if (receipt.response.startsWith('error\t')) {
    const result = outcome(receipt.response);
    if (result.kind === 'unknown') throw new Error('Unknown outcome; receipt retained');
    await client.release(receipt.id);
    delete journal.receipt; persist();
    throw new Error(receipt.response);
  }
  adopt(receipt.fields, receipt.response);
  persist();
  await client.release(receipt.id);
  delete journal.receipt; persist();
}
