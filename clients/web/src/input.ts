/**
 * Keyboard Input Handler
 *
 * Captures keyboard events and sends them to the server via gRPC-Web.
 */

import type { ReovimClient } from "./client.js";
import type { Editor } from "./editor.js";
import { browserKeyToVim, shouldPreventDefault } from "./keymapper.js";

/**
 * Setup keyboard event handler for the editor.
 *
 * Captures keydown events on the document and sends them to the server.
 *
 * @param client - gRPC client for server communication
 * @param _editor - Editor instance (unused - state updates via notifications)
 * @param myClientId - This client's unique ID (Phase 11.2)
 */
export function setupKeyboardHandler(
  client: ReovimClient,
  _editor: Editor,
  myClientId: bigint
): void {
  document.addEventListener("keydown", async (event: KeyboardEvent) => {
    // Check if we should prevent browser default behavior
    if (shouldPreventDefault(event)) {
      event.preventDefault();
    }

    // Convert to vim notation
    const vimKey = browserKeyToVim(event);
    if (!vimKey) {
      return; // Ignore modifier-only or unknown keys
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

      // NOTE: We now rely on notifications for state updates instead of
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

  console.log("Keyboard handler initialized");
}
