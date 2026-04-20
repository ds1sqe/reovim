/**
 * CompletionExtension — completion popup overlay rendering.
 *
 * Mirrors `clients/tui/extensions/completion/src/lib.rs`.
 * Displays a floating completion popup with kind badge, label,
 * optional detail, and source ID.
 *
 * JSON payload (from `ext/server/modules/completion/src/bridge.rs`):
 * ```json
 * {"active": true, "items": [{"label": "HashMap", "kindAbbrev": "cls",
 *   "sourceId": "lsp", "detail": "HashMap<K, V>"}],
 *  "selected": 0, "prefix": "Hash", "scrollOffset": 0}
 * ```
 */

import type { WebExtension } from "./interface.js";

const KIND = "completion";

/** Maximum visible items in the popup (matches TUI constant). */
const MAX_VISIBLE_ITEMS = 10;

/** Map `kindAbbrev` to CSS class suffix for kind-specific colors. */
const KIND_CSS: Record<string, string> = {
  fn: "function",
  met: "method",
  var: "variable",
  cls: "class",
  kw: "keyword",
  snp: "snippet",
  fld: "field",
};

interface CompletionItem {
  label: string;
  kindAbbrev: string;
  kindIcon?: string;
  sourceId: string;
  detail?: string;
}

interface CompletionState {
  active: boolean;
  items: CompletionItem[];
  selected: number;
  prefix: string;
  scrollOffset: number;
}

function defaultState(): CompletionState {
  return {
    active: false,
    items: [],
    selected: 0,
    prefix: "",
    scrollOffset: 0,
  };
}

export class CompletionExtension implements WebExtension {
  private state: CompletionState = defaultState();
  private containerElement: HTMLElement | null = null;

  kind(): string {
    return KIND;
  }

  isActive(): boolean {
    return this.state.active;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Partial<CompletionState>;
      const active = parsed.active ?? false;

      if (!active) {
        this.state = defaultState();
        return;
      }

      const rawItems = Array.isArray(parsed.items) ? parsed.items : [];
      const items: CompletionItem[] = rawItems
        .filter((item): item is CompletionItem => typeof item.label === "string")
        .map((item) => ({
          label: item.label,
          kindAbbrev: typeof item.kindAbbrev === "string" ? item.kindAbbrev : "txt",
          sourceId: typeof item.sourceId === "string" ? item.sourceId : "",
          detail: typeof item.detail === "string" ? item.detail : undefined,
        }));

      this.state = {
        active: true,
        items,
        selected: typeof parsed.selected === "number" ? parsed.selected : 0,
        prefix: typeof parsed.prefix === "string" ? parsed.prefix : "",
        scrollOffset: typeof parsed.scrollOffset === "number" ? parsed.scrollOffset : 0,
      };
    } catch {
      // Invalid JSON — retain previous state.
    }
  }

  render(container: HTMLElement): void {
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }

    if (!this.state.active || this.state.items.length === 0) return;

    const overlay = document.createElement("div");
    overlay.className = "overlay overlay-completion";

    const list = document.createElement("div");
    list.className = "completion-list";

    const start = this.state.scrollOffset;
    const visible = this.state.items.slice(start, start + MAX_VISIBLE_ITEMS);

    for (const [i, item] of visible.entries()) {
      const globalIndex = start + i;
      const itemEl = document.createElement("div");
      itemEl.className = "completion-item";
      if (globalIndex === this.state.selected) {
        itemEl.classList.add("selected");
      }

      // Kind badge (icon preferred, abbreviation fallback).
      const kindEl = document.createElement("span");
      kindEl.className = "completion-kind";
      const cssKind = KIND_CSS[item.kindAbbrev];
      if (cssKind) {
        kindEl.classList.add(`completion-kind-${cssKind}`);
      }
      kindEl.textContent = item.kindIcon || item.kindAbbrev;
      itemEl.appendChild(kindEl);

      // Label.
      const labelEl = document.createElement("span");
      labelEl.className = "completion-label";
      labelEl.textContent = item.label;
      itemEl.appendChild(labelEl);

      // Detail (optional).
      if (item.detail) {
        const detailEl = document.createElement("span");
        detailEl.className = "completion-detail";
        detailEl.textContent = item.detail;
        itemEl.appendChild(detailEl);
      }

      // Source ID.
      if (item.sourceId) {
        const sourceEl = document.createElement("span");
        sourceEl.className = "completion-source";
        sourceEl.textContent = item.sourceId;
        itemEl.appendChild(sourceEl);
      }

      list.appendChild(itemEl);
    }

    overlay.appendChild(list);
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
      items: this.state.items,
      selected: this.state.selected,
      prefix: this.state.prefix,
      scrollOffset: this.state.scrollOffset,
    };
  }
}
