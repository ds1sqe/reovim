// Overlay rendering for floating UI elements.
//
// This module renders overlays (command-line, completion menus, hover info)
// as floating DOM elements positioned relative to the editor.

import type { LogicalOverlay, Anchor, OverlayState } from "../wasm/index.js";

/**
 * Configuration for overlay positioning.
 */
interface OverlayConfig {
  /** Approximate character width in pixels. */
  charWidth: number;
  /** Line height in pixels. */
  lineHeight: number;
  /** Padding from container edges. */
  padding: number;
}

const DEFAULT_CONFIG: OverlayConfig = {
  charWidth: 8.4,
  lineHeight: 21,
  padding: 8,
};

/**
 * Completion item structure (from server data).
 */
interface CompletionItem {
  label: string;
  kind?: string;
  detail?: string;
  documentation?: string;
}

/**
 * Renders overlays as floating DOM elements.
 *
 * Supports multiple overlay types:
 * - `completion` - Completion menu with selectable items
 * - `cmdline` - Command-line input (e.g., for `:` commands)
 * - `hover` - Hover information popup
 * - `signature` - Function signature help
 *
 * @example
 * ```typescript
 * const renderer = new OverlayRenderer();
 * renderer.setContainer(document.getElementById("editor")!);
 *
 * // Show completion menu
 * renderer.show({
 *   id: "completion-1",
 *   kind: "completion",
 *   anchor: "Cursor",
 *   data: { items: [{ label: "foo" }, { label: "bar" }] },
 *   state: { selected_index: 0 },
 *   priority: 10,
 * });
 *
 * // Hide it
 * renderer.hide("completion-1");
 * ```
 */
export class OverlayRenderer {
  private container: HTMLElement | null = null;
  private overlayElements: Map<string, HTMLElement> = new Map();
  private config: OverlayConfig;

  // Current cursor position (for anchor calculations)
  private cursorX: number = 0;
  private cursorY: number = 0;

  constructor(config: Partial<OverlayConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Set the container element for overlays.
   *
   * All overlays are positioned relative to this container.
   *
   * @param container - The editor container element
   */
  setContainer(container: HTMLElement): void {
    this.container = container;
  }

  /**
   * Update cursor position for anchor calculations.
   *
   * Call this when cursor moves to ensure correct overlay positioning.
   *
   * @param line - Cursor line (0-indexed)
   * @param col - Cursor column (0-indexed)
   */
  updateCursorPosition(line: number, col: number): void {
    this.cursorX = col * this.config.charWidth + this.config.padding;
    this.cursorY = line * this.config.lineHeight + this.config.padding;
  }

  /**
   * Show an overlay.
   *
   * Creates the DOM element if it doesn't exist, then updates
   * content and position.
   *
   * @param overlay - The logical overlay from the server
   */
  show(overlay: LogicalOverlay): void {
    if (!this.container) {
      console.warn("OverlayRenderer: No container set");
      return;
    }

    let el = this.overlayElements.get(overlay.id);
    if (!el) {
      el = this.createOverlayElement(overlay);
      this.overlayElements.set(overlay.id, el);
      this.container.appendChild(el);
    }

    this.updateContent(el, overlay);
    this.positionOverlay(el, overlay.anchor);
    el.style.display = "block";
    el.setAttribute("aria-hidden", "false");
  }

  /**
   * Hide an overlay by ID.
   *
   * The element is hidden but not removed, allowing efficient re-show.
   *
   * @param overlayId - The overlay identifier
   */
  hide(overlayId: string): void {
    const el = this.overlayElements.get(overlayId);
    if (el) {
      el.style.display = "none";
      el.setAttribute("aria-hidden", "true");
    }
  }

  /**
   * Remove an overlay completely.
   *
   * @param overlayId - The overlay identifier
   */
  remove(overlayId: string): void {
    const el = this.overlayElements.get(overlayId);
    if (el) {
      el.remove();
      this.overlayElements.delete(overlayId);
    }
  }

  /**
   * Hide all overlays.
   */
  hideAll(): void {
    for (const el of this.overlayElements.values()) {
      el.style.display = "none";
      el.setAttribute("aria-hidden", "true");
    }
  }

  /**
   * Remove all overlays.
   */
  clear(): void {
    for (const el of this.overlayElements.values()) {
      el.remove();
    }
    this.overlayElements.clear();
  }

  /**
   * Check if an overlay is currently visible.
   *
   * @param overlayId - The overlay identifier
   * @returns true if visible
   */
  isVisible(overlayId: string): boolean {
    const el = this.overlayElements.get(overlayId);
    return el ? el.style.display !== "none" : false;
  }

  /**
   * Get all visible overlay IDs.
   *
   * @returns Array of visible overlay IDs
   */
  getVisibleOverlays(): string[] {
    const visible: string[] = [];
    for (const [id, el] of this.overlayElements) {
      if (el.style.display !== "none") {
        visible.push(id);
      }
    }
    return visible;
  }

  /**
   * Update overlay state (e.g., selection index).
   *
   * @param overlayId - The overlay identifier
   * @param state - The new state
   */
  updateState(overlayId: string, state: Partial<OverlayState>): void {
    const el = this.overlayElements.get(overlayId);
    if (!el) return;

    // Update selection in completion menus
    if (state.selected_index !== undefined) {
      const items = el.querySelectorAll(".completion-item");
      items.forEach((item, i) => {
        if (i === state.selected_index) {
          item.classList.add("selected");
          item.setAttribute("aria-selected", "true");
          // Scroll into view if needed
          (item as HTMLElement).scrollIntoView?.({ block: "nearest" });
        } else {
          item.classList.remove("selected");
          item.setAttribute("aria-selected", "false");
        }
      });
    }
  }

  // ============ Private Methods ============

  /**
   * Create the DOM element for an overlay.
   */
  private createOverlayElement(overlay: LogicalOverlay): HTMLElement {
    const el = document.createElement("div");
    el.className = `overlay overlay-${overlay.kind}`;
    el.dataset.overlayId = overlay.id;
    el.style.position = "absolute";
    el.style.zIndex = String(100 + overlay.priority);
    el.style.display = "none";

    // Accessibility attributes
    el.setAttribute("role", this.getAriaRole(overlay.kind));
    el.setAttribute("aria-hidden", "true");

    return el;
  }

  /**
   * Get ARIA role based on overlay kind.
   */
  private getAriaRole(kind: string): string {
    switch (kind) {
      case "completion":
        return "listbox";
      case "cmdline":
        return "textbox";
      case "hover":
      case "signature":
        return "tooltip";
      default:
        return "dialog";
    }
  }

  /**
   * Update overlay content based on kind.
   */
  private updateContent(el: HTMLElement, overlay: LogicalOverlay): void {
    switch (overlay.kind) {
      case "completion":
        this.renderCompletion(el, overlay);
        break;
      case "cmdline":
        this.renderCommandLine(el, overlay);
        break;
      case "hover":
        this.renderHover(el, overlay);
        break;
      case "signature":
        this.renderSignature(el, overlay);
        break;
      default:
        this.renderDefault(el, overlay);
    }
  }

  /**
   * Render completion menu.
   */
  private renderCompletion(el: HTMLElement, overlay: LogicalOverlay): void {
    el.innerHTML = "";

    const data = overlay.data as { items?: CompletionItem[] };
    const items = data?.items || [];
    const selectedIndex = overlay.state?.selected_index ?? 0;

    if (items.length === 0) {
      el.innerHTML = '<div class="completion-empty">No completions</div>';
      return;
    }

    const list = document.createElement("ul");
    list.className = "completion-list";
    list.setAttribute("role", "listbox");

    items.forEach((item, i) => {
      const li = document.createElement("li");
      li.className = "completion-item";
      if (i === selectedIndex) {
        li.classList.add("selected");
        li.setAttribute("aria-selected", "true");
      } else {
        li.setAttribute("aria-selected", "false");
      }
      li.setAttribute("role", "option");
      li.dataset.index = String(i);

      // Label
      const label = document.createElement("span");
      label.className = "completion-label";
      label.textContent = item.label;
      li.appendChild(label);

      // Kind badge (if present)
      if (item.kind) {
        const kind = document.createElement("span");
        kind.className = `completion-kind completion-kind-${item.kind.toLowerCase()}`;
        kind.textContent = item.kind;
        li.appendChild(kind);
      }

      // Detail (if present)
      if (item.detail) {
        const detail = document.createElement("span");
        detail.className = "completion-detail";
        detail.textContent = item.detail;
        li.appendChild(detail);
      }

      list.appendChild(li);
    });

    el.appendChild(list);
  }

  /**
   * Render command-line input.
   */
  private renderCommandLine(el: HTMLElement, overlay: LogicalOverlay): void {
    el.innerHTML = "";

    const data = overlay.data as { prefix?: string; content?: string; cursor?: number };
    const prefix = data?.prefix ?? ":";
    const content = data?.content ?? "";
    const cursor = data?.cursor ?? content.length;

    const container = document.createElement("div");
    container.className = "cmdline-container";

    // Prefix (e.g., ":" or "/")
    const prefixSpan = document.createElement("span");
    prefixSpan.className = "cmdline-prefix";
    prefixSpan.textContent = prefix;
    container.appendChild(prefixSpan);

    // Content with cursor
    const contentSpan = document.createElement("span");
    contentSpan.className = "cmdline-content";

    // Split content around cursor
    const beforeCursor = content.slice(0, cursor);
    const afterCursor = content.slice(cursor);

    const beforeSpan = document.createElement("span");
    beforeSpan.textContent = beforeCursor;
    contentSpan.appendChild(beforeSpan);

    const cursorSpan = document.createElement("span");
    cursorSpan.className = "cmdline-cursor";
    cursorSpan.textContent = afterCursor.charAt(0) || " ";
    contentSpan.appendChild(cursorSpan);

    if (afterCursor.length > 1) {
      const afterSpan = document.createElement("span");
      afterSpan.textContent = afterCursor.slice(1);
      contentSpan.appendChild(afterSpan);
    }

    container.appendChild(contentSpan);
    el.appendChild(container);
  }

  /**
   * Render hover information.
   */
  private renderHover(el: HTMLElement, overlay: LogicalOverlay): void {
    el.innerHTML = "";

    const data = overlay.data as { text?: string; markdown?: string };
    const text = data?.text || data?.markdown || "";

    const content = document.createElement("div");
    content.className = "hover-content";

    // Simple text rendering (markdown support could be added)
    content.textContent = text;

    el.appendChild(content);
  }

  /**
   * Render function signature help.
   */
  private renderSignature(el: HTMLElement, overlay: LogicalOverlay): void {
    el.innerHTML = "";

    const data = overlay.data as {
      signatures?: Array<{ label: string; documentation?: string }>;
      activeSignature?: number;
      activeParameter?: number;
    };

    const signatures = data?.signatures || [];
    const activeSignature = data?.activeSignature ?? 0;

    if (signatures.length === 0) return;

    const sig = signatures[activeSignature];
    if (!sig) return;

    const container = document.createElement("div");
    container.className = "signature-container";

    const label = document.createElement("div");
    label.className = "signature-label";
    label.textContent = sig.label;
    container.appendChild(label);

    if (sig.documentation) {
      const doc = document.createElement("div");
      doc.className = "signature-documentation";
      doc.textContent = sig.documentation;
      container.appendChild(doc);
    }

    el.appendChild(container);
  }

  /**
   * Render default fallback content.
   */
  private renderDefault(el: HTMLElement, overlay: LogicalOverlay): void {
    el.innerHTML = "";

    const pre = document.createElement("pre");
    pre.className = "overlay-fallback";
    pre.textContent = JSON.stringify(overlay.data, null, 2);
    el.appendChild(pre);
  }

  /**
   * Position overlay based on anchor.
   *
   * Anchor types from reovim-client-model:
   * - "Cursor" - Position below the cursor
   * - "Center" - Center on screen
   * - { Buffer: { buffer_id, line, col } } - Position at buffer location
   * - { Screen: { x, y } } - Normalized screen position (0.0-1.0)
   * - { Below: "overlay_id" } - Position below another overlay
   */
  private positionOverlay(el: HTMLElement, anchor: Anchor): void {
    if (!this.container) return;

    const containerRect = this.container.getBoundingClientRect();

    // Default position based on anchor
    let x: number;
    let y: number;

    // Handle different anchor types
    if (anchor === "Cursor") {
      x = this.cursorX;
      y = this.cursorY + this.config.lineHeight; // Below cursor
    } else if (anchor === "Center") {
      x = (containerRect.width - el.offsetWidth) / 2;
      y = (containerRect.height - el.offsetHeight) / 2;
    } else if (typeof anchor === "object") {
      // Handle object anchor types
      if ("Buffer" in anchor) {
        // Buffer position anchor
        const buf = anchor.Buffer as { buffer_id: number; line: number; col: number };
        x = buf.col * this.config.charWidth + this.config.padding;
        y = buf.line * this.config.lineHeight + this.config.padding;
      } else if ("Screen" in anchor) {
        // Normalized screen position (0.0-1.0)
        const scr = anchor.Screen as { x: number; y: number };
        x = scr.x * containerRect.width;
        y = scr.y * containerRect.height;
      } else if ("Below" in anchor) {
        // Position below another overlay
        const belowId = anchor.Below as string;
        const belowEl = this.overlayElements.get(belowId);
        if (belowEl) {
          const belowRect = belowEl.getBoundingClientRect();
          x = belowRect.left - containerRect.left;
          y = belowRect.bottom - containerRect.top;
        } else {
          // Fallback to cursor position
          x = this.cursorX;
          y = this.cursorY + this.config.lineHeight;
        }
      } else {
        // Unknown object anchor, default to cursor
        x = this.cursorX;
        y = this.cursorY + this.config.lineHeight;
      }
    } else {
      // Fallback for any other anchor type
      x = this.cursorX;
      y = this.cursorY + this.config.lineHeight;
    }

    // Clamp to container bounds
    x = Math.max(this.config.padding, Math.min(x, containerRect.width - el.offsetWidth - this.config.padding));
    y = Math.max(this.config.padding, Math.min(y, containerRect.height - el.offsetHeight - this.config.padding));

    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
  }
}
