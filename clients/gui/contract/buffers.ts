import type { ResourceId } from '../../../sdk/typescript/resource.js';

/** Views retain Buffer ownership independently of context acceptance. */
export interface BufferViews {
  known(): ResourceId[];
  show(target: ResourceId | undefined): Promise<void>;
  recover(): Promise<void>;
}
