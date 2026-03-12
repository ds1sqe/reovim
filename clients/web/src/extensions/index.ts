/**
 * Extension registry — factory for all web client extensions.
 *
 * Mirrors `clients/tui/extensions/defaults/src/lib.rs`.
 * Editor imports ONLY `createExtensions`, never individual extension modules.
 */

export type { WebExtension } from "./interface.js";
export { CmdlineExtension } from "./cmdline.js";
export { WhichKeyExtension } from "./whichkey.js";
export { NotificationExtension } from "./notification.js";
export { MicroscopeExtension } from "./microscope.js";
export { CompletionExtension } from "./completion.js";
export { ExplorerExtension } from "./explorer.js";
export { RangeFinderJumpExtension } from "./range-finder-jump.js";
export { RangeFinderFoldExtension } from "./range-finder-fold.js";
export { LandingExtension } from "./landing.js";

import type { WebExtension } from "./interface.js";
import { CmdlineExtension } from "./cmdline.js";
import { WhichKeyExtension } from "./whichkey.js";
import { NotificationExtension } from "./notification.js";
import { MicroscopeExtension } from "./microscope.js";
import { CompletionExtension } from "./completion.js";
import { ExplorerExtension } from "./explorer.js";
import { RangeFinderJumpExtension } from "./range-finder-jump.js";
import { RangeFinderFoldExtension } from "./range-finder-fold.js";
import { LandingExtension } from "./landing.js";

export function createExtensions(): WebExtension[] {
  return [new CmdlineExtension(), new WhichKeyExtension(), new NotificationExtension(), new MicroscopeExtension(), new CompletionExtension(), new ExplorerExtension(), new RangeFinderJumpExtension(), new RangeFinderFoldExtension(), new LandingExtension()];
}
