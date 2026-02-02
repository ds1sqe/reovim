/**
 * Exit Protection
 *
 * Provides a safety net for accidental tab closure (Ctrl+W, etc.)
 * by warning users about unsaved changes.
 *
 * ## Browser Keyboard Policy
 *
 * Since browsers reserve Ctrl+W for tab closing and we cannot prevent it,
 * we use beforeunload as a last-resort warning for unsaved work.
 *
 * Note: This doesn't capture Ctrl+W - it just warns the user if they
 * have unsaved changes before the tab closes.
 */

import type { Editor } from "./editor.js";

/**
 * Setup exit protection with beforeunload warning.
 *
 * When the user tries to close the tab (Ctrl+W, close button, etc.)
 * and there are unsaved changes, shows a browser confirmation dialog.
 *
 * @param editor - Editor instance to check for unsaved changes
 */
export function setupExitProtection(editor: Editor): void {
  window.addEventListener("beforeunload", (event: BeforeUnloadEvent) => {
    // Check if editor has unsaved changes
    // Note: Editor.hasUnsavedChanges() may not be implemented yet
    if (editor.hasUnsavedChanges?.()) {
      // Trigger browser's "Leave site?" dialog
      event.preventDefault();
      // Modern browsers ignore custom messages but this is still required
      event.returnValue = "";
    }
  });

  console.log("Exit protection initialized (beforeunload warning for unsaved changes)");
}
