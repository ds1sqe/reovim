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
 * Client identity is provided via the x-reovim-token header (#483),
 * so no clientId parameter is needed in the request body.
 *
 * @param client - gRPC client for server communication
 * @param _editor - Editor instance (unused - state updates via notifications)
 */
export function setupKeyboardHandler(
  client: ReovimClient,
  _editor: Editor,
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
      // Send key to server (#483: identity via x-reovim-token header)
      console.log(`[input] Sending key: ${vimKey}`);
      const response = await client.input.sendKeys({
        keys: vimKey,
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
