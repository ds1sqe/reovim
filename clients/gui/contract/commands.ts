import type { ResourceId } from '../../../sdk/typescript/resource.js';
import type { Unit } from '../../../modules/text/api/typescript.js';

export interface Selection {
  snapshot: ResourceId; start: bigint; end: bigint; units: readonly Unit[];
}
export interface Editor {
  selection(): Selection;
  replace(selection: Selection, text: string): Promise<void>;
}
/** Busy close fences new calls; the caller keeps this owner and retries. */
export interface Registration { close(): void }
export type RegisterCommand = (name: string, run: () => Promise<void>) => Registration;
