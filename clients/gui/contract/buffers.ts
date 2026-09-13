import type { ResourceId } from '../../../sdk/typescript/resource.js';

/** Views retain Buffer ownership independently of context acceptance. */
export interface BufferViews {
  current(): ResourceId | undefined;
  /** Whether an outcome or cleanup still requires recovery. */
  pending(): boolean;
  known(): ResourceId[];
  show(target: ResourceId | undefined): Promise<void>;
  recover(): Promise<void>;
  /** Complete the original close/detach; retry after recovery is idempotent. */
  close(target: ResourceId): Promise<void>;
}
