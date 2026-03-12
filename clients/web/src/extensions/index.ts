/**
 * Extension registry — factory for all web client extensions (#583).
 *
 * Mirrors `clients/tui/extensions/defaults/src/lib.rs`.
 * Editor imports ONLY `createExtensions`, never individual extension modules.
 *
 * Extensions are topologically sorted by declared dependencies via
 * `toposortExtensions()` and `init()` is called in dependency order.
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
import { toposortExtensions } from "./toposort.js";

/** Create all default web extensions, sorted by dependency order (#583). */
export function createExtensions(): WebExtension[] {
  const exts: WebExtension[] = [
    new CmdlineExtension(),
    new WhichKeyExtension(),
    new NotificationExtension(),
    new MicroscopeExtension(),
    new CompletionExtension(),
    new ExplorerExtension(),
    new RangeFinderJumpExtension(),
    new RangeFinderFoldExtension(),
    new LandingExtension(),
  ];

  const sorted = toposortExtensions(exts);

  for (const ext of sorted) {
    ext.init?.();
  }

  return sorted;
}

/** Validate extensions against server-available kinds (#584). */
export function validateExtensions(
  extensions: WebExtension[],
  serverAvailableKinds: string[],
): void {
  for (const ext of extensions) {
    const needed = ext.serverKinds?.() ?? [ext.kind()];
    for (const kind of needed) {
      if (!serverAvailableKinds.includes(kind)) {
        console.warn(
          `[reovim] Extension "${ext.kind()}" expects server kind "${kind}" but server does not provide it`,
        );
      }
    }
  }
}

/** Shutdown all extensions in reverse dependency order (#583). */
export function shutdownExtensions(extensions: WebExtension[]): void {
  for (let i = extensions.length - 1; i >= 0; i--) {
    extensions[i].exit?.();
  }
}
