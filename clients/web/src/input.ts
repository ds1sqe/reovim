/**
 * Keyboard Input Handler
 *
 * Captures keyboard events and sends them to the server via gRPC-Web.
 *
 * ## Browser Keyboard Policy
 *
 * Browsers reserve Ctrl+W, Ctrl+T, Ctrl+N, etc. for security.
 * We use a leader key (\) to access these combinations:
 *   - `\w` → `<C-w>` (window commands)
 *   - `\t` → `<C-t>` (tag jump)
 *   - etc.
 *
 * The server receives standard vim notation and doesn't know
 * about the web leader key.
 */

import type { ReovimClient } from "./client.js";
import type { Editor } from "./editor.js";
import { WebKeymapper } from "./keymapper.js";

/**
 * Setup keyboard event handler for the editor.
 *
 * Uses WebKeymapper to handle browser-reserved keys via leader sequences.
 *
 * @param client - gRPC client for server communication
 * @param editor - Editor instance (used for leader indicator)
 * @param myClientId - This client's unique ID (Phase 11.2)
 */
export function setupKeyboardHandler(
  client: ReovimClient,
  editor: Editor,
  myClientId: bigint
): void {
  // Create stateful keymapper with leader key support
  const keymapper = new WebKeymapper({
    onLeaderStart: () => {
      // Show leader indicator in command line
      editor.showLeaderIndicator?.();
    },
    onLeaderEnd: () => {
      // Hide leader indicator
      editor.hideLeaderIndicator?.();
    },
  });

  document.addEventListener("keydown", async (event: KeyboardEvent) => {
    // Use WebKeymapper - it handles preventDefault internally
    const vimKey = keymapper.handleKeyEvent(event);

    if (!vimKey) {
      // Leader pending, modifier-only press, or cancelled
      return;
    }

    try {
      // Send key to server WITH client ID (Phase 11.2)
      // CRITICAL: Without clientId, all clients share state as ClientId(0)
      console.log(`[input] Sending key: ${vimKey} (client ${myClientId})`);
      const response = await client.input.sendKeys({
        keys: vimKey,
        clientId: myClientId,
      });
      console.log(`[input] Response: ok=${response.ok}, status=${response.status}`);

      if (!response.ok) {
        console.warn(`Key not handled: ${vimKey}`, response.status);
      }

      // NOTE: We rely on notifications for state updates instead of
      // calling editor.refresh() after every key. This prevents state
      // overwrites and reduces unnecessary round-trips.
    } catch (error) {
      console.error(`[input] Failed to send key: ${vimKey}`, error);
    }
  });

  // Prevent context menu on right-click (for future mouse support)
  document.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });

  console.log("Keyboard handler initialized (with leader key support: \\ for Ctrl+)");
}
