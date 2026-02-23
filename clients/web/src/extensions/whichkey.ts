/**
 * WhichKeyExtension — operator hint popup rendering.
 *
 * Mirrors `clients/tui/extensions/which-key/src/lib.rs`.
 * Shows available key continuations when an operator (d, y, c) is pressed.
 *
 * JSON payload (from `server/modules/whichkey/src/bridge.rs`):
 * ```json
 * {"active": true, "prefix": "d",
 *  "hints": [{"key": "d", "command": "motions:whole-line"}, ...]}
 * ```
 */

import type { WebExtension } from "./interface.js";

interface WhichKeyHint {
  key: string;
  command: string;
}

interface WhichKeyState {
  active: boolean;
  prefix: string;
  hints: WhichKeyHint[];
}

export class WhichKeyExtension implements WebExtension {
  private state: WhichKeyState = {
    active: false,
    prefix: "",
    hints: [],
  };

  private popupElement: HTMLElement | null = null;

  kind(): string {
    return "whichkey";
  }

  isActive(): boolean {
    return this.state.active;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Partial<WhichKeyState>;
      this.state = {
        active: parsed.active ?? this.state.active,
        prefix: parsed.prefix ?? this.state.prefix,
        hints: parsed.hints ?? this.state.hints,
      };
    } catch {
      // Invalid JSON — retain previous state
    }
  }

  render(container: HTMLElement): void {
    // Remove existing popup if present
    if (this.popupElement) {
      this.popupElement.remove();
      this.popupElement = null;
    }

    const popup = document.createElement("div");
    popup.className = "whichkey-popup overlay";

    const header = document.createElement("div");
    header.className = "whichkey-header";
    header.textContent = this.state.prefix || "?";
    popup.appendChild(header);

    const hintsContainer = document.createElement("div");
    hintsContainer.className = "whichkey-hints";

    for (const hint of this.state.hints) {
      const hintEl = document.createElement("div");
      hintEl.className = "whichkey-hint";

      const keySpan = document.createElement("span");
      keySpan.className = "whichkey-key";
      keySpan.textContent = hint.key;
      hintEl.appendChild(keySpan);

      const commandSpan = document.createElement("span");
      commandSpan.className = "whichkey-command";
      commandSpan.textContent = hint.command;
      hintEl.appendChild(commandSpan);

      hintsContainer.appendChild(hintEl);
    }

    popup.appendChild(hintsContainer);
    container.appendChild(popup);
    this.popupElement = popup;
  }

  hide(): void {
    if (this.popupElement) {
      this.popupElement.remove();
      this.popupElement = null;
    }
  }

  getState(): Record<string, unknown> | null {
    if (!this.state.active) return null;
    return {
      active: this.state.active,
      prefix: this.state.prefix,
      hints: this.state.hints,
    };
  }
}
