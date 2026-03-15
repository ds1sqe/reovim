/**
 * YankFlashExtension — yank highlight flash rendering (#657).
 *
 * Mirrors `clients/tui/modules/yank-flash/src/lib.rs`.
 * Renders a brief highlight on yanked text via DOM overlay.
 *
 * JSON payload (from `server/modules/vim/src/operators/yank_flash.rs`):
 * ```json
 * {"bufferId":1,"startLine":5,"endLine":10,"startCol":0,"endCol":0,"isLinewise":true,"sequence":1}
 * ```
 */

import type { WebExtension } from "./interface.js";

const KIND = "yank-flash";

/** Flash highlight duration in milliseconds. */
const FLASH_DURATION_MS = 200;

interface FlashState {
  bufferId: number;
  startLine: number;
  endLine: number;
  startCol: number;
  endCol: number;
  isLinewise: boolean;
  sequence: number;
}

export class YankFlashExtension implements WebExtension {
  private flashState: FlashState | null = null;
  private lastSequence: number = 0;
  private flashTimeout: ReturnType<typeof setTimeout> | null = null;
  private styleElement: HTMLStyleElement | null = null;

  kind(): string {
    return KIND;
  }

  serverKinds(): string[] {
    return [KIND];
  }

  isActive(): boolean {
    return this.flashState !== null;
  }

  applyNotification(data: string): void {
    let parsed: Record<string, unknown>;
    try {
      parsed = JSON.parse(data) as Record<string, unknown>;
    } catch {
      return;
    }

    const sequence = parsed.sequence as number | undefined;
    if (sequence === undefined || sequence === null) return;
    if (sequence <= this.lastSequence) return;

    this.lastSequence = sequence;

    this.flashState = {
      bufferId: (parsed.bufferId as number) ?? 0,
      startLine: (parsed.startLine as number) ?? 0,
      endLine: (parsed.endLine as number) ?? 0,
      startCol: (parsed.startCol as number) ?? 0,
      endCol: (parsed.endCol as number) ?? 0,
      isLinewise: (parsed.isLinewise as boolean) ?? false,
      sequence,
    };

    // Clear any existing timeout
    if (this.flashTimeout !== null) {
      clearTimeout(this.flashTimeout);
    }

    // Auto-dismiss after duration
    this.flashTimeout = setTimeout(() => {
      this.flashState = null;
      this.flashTimeout = null;
    }, FLASH_DURATION_MS);
  }

  render(container: HTMLElement): void {
    if (!this.flashState) return;

    // Inject CSS if not already present
    if (!this.styleElement) {
      this.styleElement = document.createElement("style");
      this.styleElement.textContent = `
        .yank-flash-highlight {
          background-color: rgba(80, 80, 120, 0.4) !important;
          transition: background-color 150ms ease-out;
        }
      `;
      document.head.appendChild(this.styleElement);
    }

    // Find buffer lines in the container and apply flash class
    const lines = container.querySelectorAll("[data-line-idx]");
    for (const line of lines) {
      const lineIdx = parseInt(
        line.getAttribute("data-line-idx") ?? "-1",
        10,
      );
      if (
        lineIdx >= this.flashState.startLine &&
        lineIdx <= this.flashState.endLine
      ) {
        line.classList.add("yank-flash-highlight");
      }
    }
  }

  hide(): void {
    // Remove flash class from all lines
    const highlighted = document.querySelectorAll(".yank-flash-highlight");
    for (const el of highlighted) {
      el.classList.remove("yank-flash-highlight");
    }

    // Remove injected style
    if (this.styleElement) {
      this.styleElement.remove();
      this.styleElement = null;
    }
  }

  exit(): void {
    if (this.flashTimeout !== null) {
      clearTimeout(this.flashTimeout);
      this.flashTimeout = null;
    }
    this.flashState = null;
    this.hide();
  }

  getState(): Record<string, unknown> | null {
    if (!this.flashState) return null;
    return {
      active: true,
      bufferId: this.flashState.bufferId,
      startLine: this.flashState.startLine,
      endLine: this.flashState.endLine,
      startCol: this.flashState.startCol,
      endCol: this.flashState.endCol,
      isLinewise: this.flashState.isLinewise,
      sequence: this.flashState.sequence,
    };
  }
}
