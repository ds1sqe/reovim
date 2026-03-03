/**
 * MicroscopeExtension — fuzzy finder overlay rendering.
 *
 * Mirrors `clients/tui/extensions/microscope/src/lib.rs`.
 * Displays a Helix-style bottom-anchored picker with query input,
 * results list, and preview pane.
 *
 * JSON payload (from `server/modules/microscope/src/bridge.rs`):
 * ```json
 * {"active": true, "query": "main", "cursor": 4, "selected": 1,
 *  "scrollOffset": 0, "pickerName": "files", "pickerTitle": "Files",
 *  "prompt": "> ", "items": [{"display": "main.rs", "detail": "src/"}],
 *  "totalCount": 100, "matchedCount": 3,
 *  "preview": {"lines": ["fn main() {"], "highlightLine": 0, "filePath": "main.rs"}}
 * ```
 */

import type { WebExtension } from "./interface.js";

/** Maximum visible items in the results list. */
const MAX_VISIBLE_ITEMS = 20;

interface MicroscopeItem {
  display: string;
  detail?: string;
  icon?: string;
}

interface MicroscopePreview {
  lines: string[];
  highlightLine?: number;
  filePath?: string;
}

interface MicroscopeState {
  active: boolean;
  query: string;
  cursor: number;
  selected: number;
  scrollOffset: number;
  pickerName: string;
  pickerTitle: string;
  prompt: string;
  items: MicroscopeItem[];
  totalCount: number;
  matchedCount: number;
  preview?: MicroscopePreview;
}

function defaultState(): MicroscopeState {
  return {
    active: false,
    query: "",
    cursor: 0,
    selected: 0,
    scrollOffset: 0,
    pickerName: "",
    pickerTitle: "",
    prompt: "> ",
    items: [],
    totalCount: 0,
    matchedCount: 0,
  };
}

export class MicroscopeExtension implements WebExtension {
  private state: MicroscopeState = defaultState();
  private containerElement: HTMLElement | null = null;

  kind(): string {
    return "microscope";
  }

  isActive(): boolean {
    return this.state.active;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Partial<MicroscopeState>;
      const active = parsed.active ?? false;

      if (!active) {
        this.state = defaultState();
        return;
      }

      this.state = {
        active: true,
        query: parsed.query ?? "",
        cursor: parsed.cursor ?? 0,
        selected: parsed.selected ?? 0,
        scrollOffset: parsed.scrollOffset ?? 0,
        pickerName: parsed.pickerName ?? "",
        pickerTitle: parsed.pickerTitle ?? "",
        prompt: parsed.prompt ?? "> ",
        items: Array.isArray(parsed.items) ? parsed.items : [],
        totalCount: parsed.totalCount ?? 0,
        matchedCount: parsed.matchedCount ?? 0,
        preview: parsed.preview,
      };
    } catch {
      // Invalid JSON — retain previous state
    }
  }

  render(container: HTMLElement): void {
    // Remove existing overlay
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }

    if (!this.state.active) return;

    const overlay = document.createElement("div");
    overlay.className = "microscope-overlay overlay";

    // Query row
    const queryRow = document.createElement("div");
    queryRow.className = "microscope-query-row";

    const prompt = document.createElement("span");
    prompt.className = "microscope-prompt";
    prompt.textContent = this.state.prompt;
    queryRow.appendChild(prompt);

    const queryInput = document.createElement("span");
    queryInput.className = "microscope-query";
    queryInput.textContent = this.state.query;
    queryRow.appendChild(queryInput);

    const counter = document.createElement("span");
    counter.className = "microscope-counter";
    counter.textContent = `${this.state.matchedCount}/${this.state.totalCount}`;
    queryRow.appendChild(counter);

    overlay.appendChild(queryRow);

    // Content area (results + preview)
    const content = document.createElement("div");
    content.className = "microscope-content";

    // Results list
    const results = document.createElement("div");
    results.className = "microscope-results";

    const visible = this.state.items.slice(0, MAX_VISIBLE_ITEMS);
    for (const [i, item] of visible.entries()) {
      const itemEl = document.createElement("div");
      itemEl.className = "microscope-item";
      if (i === this.state.selected) {
        itemEl.classList.add("selected");
      }

      if (item.icon) {
        const iconEl = document.createElement("span");
        iconEl.className = "microscope-item-icon";
        iconEl.textContent = item.icon;
        itemEl.appendChild(iconEl);
      }

      const displayEl = document.createElement("span");
      displayEl.className = "microscope-item-display";
      displayEl.textContent = item.display;
      itemEl.appendChild(displayEl);

      if (item.detail) {
        const detailEl = document.createElement("span");
        detailEl.className = "microscope-item-detail";
        detailEl.textContent = item.detail;
        itemEl.appendChild(detailEl);
      }

      results.appendChild(itemEl);
    }

    content.appendChild(results);

    // Preview pane
    if (this.state.preview) {
      const preview = document.createElement("div");
      preview.className = "microscope-preview";

      if (this.state.preview.filePath) {
        const pathEl = document.createElement("div");
        pathEl.className = "microscope-preview-path";
        pathEl.textContent = this.state.preview.filePath;
        preview.appendChild(pathEl);
      }

      const lines = document.createElement("div");
      lines.className = "microscope-preview-lines";

      for (const [i, text] of this.state.preview.lines.entries()) {
        const lineEl = document.createElement("div");
        lineEl.className = "microscope-preview-line";
        if (this.state.preview.highlightLine === i) {
          lineEl.classList.add("highlighted");
        }

        const lineNum = document.createElement("span");
        lineNum.className = "microscope-line-number";
        lineNum.textContent = String(i + 1);
        lineEl.appendChild(lineNum);

        const lineText = document.createElement("span");
        lineText.className = "microscope-line-text";
        lineText.textContent = text;
        lineEl.appendChild(lineText);

        lines.appendChild(lineEl);
      }

      preview.appendChild(lines);
      content.appendChild(preview);
    }

    overlay.appendChild(content);
    container.appendChild(overlay);
    this.containerElement = overlay;
  }

  hide(): void {
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }
  }

  getState(): Record<string, unknown> | null {
    if (!this.state.active) return null;
    return {
      active: this.state.active,
      query: this.state.query,
      cursor: this.state.cursor,
      selected: this.state.selected,
      pickerName: this.state.pickerName,
      pickerTitle: this.state.pickerTitle,
      items: this.state.items,
      matchedCount: this.state.matchedCount,
      totalCount: this.state.totalCount,
      preview: this.state.preview ?? null,
    };
  }
}
