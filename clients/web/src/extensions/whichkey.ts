/**
 * WhichKeyExtension — operator hint popup rendering.
 *
 * Mirrors `clients/tui/extensions/which-key/src/lib.rs`.
 * Shows available key continuations when an operator (d, y, c) is pressed.
 *
 * The popup appears after a configurable delay (default 500ms).
 * If the user completes the key sequence before the delay expires,
 * no popup is shown at all.
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
  category?: string;
}

/** Default category display order. Unlisted categories sort after these. */
const CATEGORY_ORDER = ["motion", "operator", "textobject", "window", "buffer"];

interface WhichKeyState {
  active: boolean;
  prefix: string;
  hints: WhichKeyHint[];
}

/** Default delay in milliseconds before showing the popup. */
const DEFAULT_SHOW_DELAY_MS = 500;

export class WhichKeyExtension implements WebExtension {
  private state: WhichKeyState = {
    active: false,
    prefix: "",
    hints: [],
  };

  /** Whether the server says a prefix is pending. */
  private serverActive: boolean = false;

  /** Whether the popup is visible to the user. */
  private visible: boolean = false;

  /** Timer ID for the show-delay. */
  private timerId: ReturnType<typeof setTimeout> | null = null;

  /** Delay before showing the popup (milliseconds). */
  private readonly showDelayMs: number;

  private popupElement: HTMLElement | null = null;

  constructor(showDelayMs: number = DEFAULT_SHOW_DELAY_MS) {
    this.showDelayMs = showDelayMs;
  }

  kind(): string {
    return "whichkey";
  }

  isActive(): boolean {
    return this.visible;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Partial<WhichKeyState>;
      const active = parsed.active ?? false;

      // Always update prefix and hints
      this.state = {
        active,
        prefix: parsed.prefix ?? this.state.prefix,
        hints: parsed.hints ?? this.state.hints,
      };

      if (active) {
        // Start (or restart) the delay timer
        if (!this.serverActive) {
          this.clearTimer();
          this.timerId = setTimeout(() => {
            this.visible = true;
          }, this.showDelayMs);
        }
        this.serverActive = true;
      } else {
        // Deactivate: clear timer and hide
        this.clearTimer();
        this.serverActive = false;
        this.visible = false;
      }
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

    // Group hints by category
    const groups = new Map<string, WhichKeyHint[]>();
    for (const hint of this.state.hints) {
      const cat = hint.category || "";
      if (!groups.has(cat)) groups.set(cat, []);
      groups.get(cat)!.push(hint);
    }

    // Sort groups by CATEGORY_ORDER
    const sortedGroups = [...groups.entries()].sort(([a], [b]) => {
      const ai = CATEGORY_ORDER.indexOf(a);
      const bi = CATEGORY_ORDER.indexOf(b);
      const aIdx = ai >= 0 ? ai : CATEGORY_ORDER.length;
      const bIdx = bi >= 0 ? bi : CATEGORY_ORDER.length;
      if (aIdx !== bIdx) return aIdx - bIdx;
      return a.localeCompare(b);
    });

    // Check if we have meaningful categories (not just empty strings)
    const hasCategories =
      sortedGroups.length > 1 ||
      (sortedGroups.length === 1 && sortedGroups[0][0] !== "");

    for (const [cat, catHints] of sortedGroups) {
      if (hasCategories) {
        const groupHeader = document.createElement("div");
        groupHeader.className = "whichkey-group-header";
        groupHeader.textContent =
          cat === ""
            ? "Other"
            : cat.charAt(0).toUpperCase() + cat.slice(1);
        hintsContainer.appendChild(groupHeader);
      }

      for (const hint of catHints) {
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
    if (!this.visible) return null;
    return {
      active: this.state.active,
      prefix: this.state.prefix,
      hints: this.state.hints,
    };
  }

  private clearTimer(): void {
    if (this.timerId !== null) {
      clearTimeout(this.timerId);
      this.timerId = null;
    }
  }
}
