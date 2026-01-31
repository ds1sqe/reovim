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
 */
export function setupKeyboardHandler(client: ReovimClient, editor: Editor): void {
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
      // Send key to server
      const response = await client.input.sendKeys({ keys: vimKey });

      if (!response.ok) {
        console.warn(`Key not handled: ${vimKey}`, response.status);
      }

      // Refresh editor state after key processing
      // Note: In production, we'd rely on notifications instead of polling
      await editor.refresh();
    } catch (error) {
      console.error(`Failed to send key: ${vimKey}`, error);
    }
  });

  // Prevent context menu on right-click (for future mouse support)
  document.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });

  console.log("Keyboard handler initialized");
}
