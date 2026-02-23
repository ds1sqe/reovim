/**
 * Extension registry — factory for all web client extensions.
 *
 * Mirrors `clients/tui/extensions/defaults/src/lib.rs`.
 * Editor imports ONLY `createExtensions`, never individual extension modules.
 */

export type { WebExtension } from "./interface.js";
export { CmdlineExtension } from "./cmdline.js";
export { WhichKeyExtension } from "./whichkey.js";

import type { WebExtension } from "./interface.js";
import { CmdlineExtension } from "./cmdline.js";
import { WhichKeyExtension } from "./whichkey.js";

export function createExtensions(): WebExtension[] {
  return [new CmdlineExtension(), new WhichKeyExtension()];
}
