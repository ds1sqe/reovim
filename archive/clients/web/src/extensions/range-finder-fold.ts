/**
 * RangeFinderFoldExtension -- fold indicator rendering for web client.
 *
 * Mirrors `clients/tui/extensions/range-finder/src/fold.rs`.
 * Parses FoldBridge JSON and renders fold markers at collapsed line positions.
 *
 * JSON payload (from `ext/server/modules/range-finder/src/fold/bridge.rs`):
 * ```json
 * {"folds": {"1": [{"start_line": 2, "hidden_count": 6, "preview": "fn foo() {"}]}}
 * ```
 */

import type { WebExtension } from "./interface.js";

const KIND = "range-finder-fold";

interface CollapsedFold {
  start_line: number;
  hidden_count: number;
  preview: string;
}

interface FoldState {
  folds: Record<string, CollapsedFold[]>;
}

export class RangeFinderFoldExtension implements WebExtension {
  private state: FoldState = { folds: {} };
  private containerElement: HTMLElement | null = null;

  kind(): string {
    return KIND;
  }

  isActive(): boolean {
    return Object.keys(this.state.folds).length > 0;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Record<string, unknown>;
      this.state.folds = {};

      const foldsObj = parsed.folds as Record<string, unknown> | undefined;
      if (foldsObj === undefined || typeof foldsObj !== "object" || foldsObj === null) return;

      for (const [bufId, entries] of Object.entries(foldsObj)) {
        if (!Array.isArray(entries)) continue;

        const folds: CollapsedFold[] = [];
        for (const entry of entries) {
          const e = entry as Record<string, unknown>;
          const start_line = e.start_line as number | undefined;
          const hidden_count = e.hidden_count as number | undefined;
          if (start_line === undefined || hidden_count === undefined) continue;
          folds.push({
            start_line,
            hidden_count,
            preview: (e.preview as string) ?? "",
          });
        }

        if (folds.length > 0) {
          this.state.folds[bufId] = folds;
        }
      }
    } catch {
      // Invalid JSON -- retain previous state
    }
  }

  render(container: HTMLElement): void {
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }

    if (!this.isActive()) return;

    const wrapper = document.createElement("div");
    wrapper.className = "range-finder-fold-overlay overlay";

    for (const [bufId, folds] of Object.entries(this.state.folds)) {
      for (const fold of folds) {
        const markerEl = document.createElement("div");
        markerEl.className = "fold-marker";
        markerEl.textContent = `--- ${fold.hidden_count} lines: ${fold.preview} ---`;
        markerEl.dataset.bufferId = bufId;
        markerEl.dataset.startLine = String(fold.start_line);
        wrapper.appendChild(markerEl);
      }
    }

    container.appendChild(wrapper);
    this.containerElement = wrapper;
  }

  hide(): void {
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }
  }

  getState(): Record<string, unknown> | null {
    if (!this.isActive()) return null;
    return { folds: this.state.folds };
  }
}
