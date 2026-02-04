// Layout rendering - converts WindowTree to DOM.
//
// This module handles rendering the window tree structure to the DOM,
// including splits, tabs, and individual windows.

import type { WindowTree, Window, Rect, SplitDirection } from "../wasm/index.js";
import { isLeaf, isSplit, isTabs } from "../wasm/index.js";

/**
 * Options for the layout renderer.
 */
export interface LayoutRendererOptions {
  /** Approximate character width in pixels (for CSS calculations). */
  charWidth: number;
  /** Line height in pixels. */
  lineHeight: number;
  /** Whether to render separators between windows. */
  showSeparators: boolean;
  /** Gap between windows in pixels (if not using separators). */
  windowGap: number;
}

const DEFAULT_OPTIONS: LayoutRendererOptions = {
  charWidth: 8.4,
  lineHeight: 21,
  showSeparators: true,
  windowGap: 1,
};

/**
 * Renders a WindowTree to the DOM.
 *
 * This renderer converts the WASM-computed WindowTree (with bounds in
 * character cells) into positioned DOM elements using CSS absolute positioning.
 */
export class LayoutRenderer {
  private options: LayoutRendererOptions;
  private windowElements: Map<number, HTMLElement> = new Map();

  constructor(options: Partial<LayoutRendererOptions> = {}) {
    this.options = { ...DEFAULT_OPTIONS, ...options };
  }

  /**
   * Update renderer options.
   */
  setOptions(options: Partial<LayoutRendererOptions>): void {
    this.options = { ...this.options, ...options };
  }

  /**
   * Render a window tree to the container.
   *
   * @param tree - The window tree to render
   * @param container - The DOM container element
   * @param focusedId - The ID of the focused window
   */
  render(tree: WindowTree, container: HTMLElement, focusedId: number): void {
    // Clear existing content
    container.innerHTML = "";
    this.windowElements.clear();

    // Add layout container class
    container.classList.add("layout-container");

    // Render the tree recursively
    this.renderNode(tree, container, focusedId);
  }

  /**
   * Get the DOM element for a window by ID.
   */
  getWindowElement(windowId: number): HTMLElement | undefined {
    return this.windowElements.get(windowId);
  }

  /**
   * Get all rendered window elements.
   */
  getAllWindowElements(): Map<number, HTMLElement> {
    return new Map(this.windowElements);
  }

  private renderNode(
    tree: WindowTree,
    container: HTMLElement,
    focusedId: number
  ): void {
    if (isLeaf(tree)) {
      this.renderWindow(tree.Leaf, container, tree.Leaf.id === focusedId);
    } else if (isSplit(tree)) {
      this.renderSplit(tree.Split, container, focusedId);
    } else if (isTabs(tree)) {
      this.renderTabs(tree.Tabs, container, focusedId);
    }
  }

  private renderWindow(
    window: Window,
    container: HTMLElement,
    focused: boolean
  ): void {
    const el = document.createElement("div");
    el.className = "window" + (focused ? " focused" : "");
    el.dataset.windowId = window.id.toString();
    el.dataset.viewportId = window.viewport_id.toString();
    el.dataset.bufferId = window.buffer_id.toString();

    // Position using character grid
    this.applyBounds(el, window.bounds);

    // Create buffer container (where content will be rendered)
    const bufferEl = document.createElement("div");
    bufferEl.className = "window-buffer";
    el.appendChild(bufferEl);

    // Create cursor element
    const cursorEl = document.createElement("div");
    cursorEl.className = "window-cursor";
    el.appendChild(cursorEl);

    // Create status line container
    const statusEl = document.createElement("div");
    statusEl.className = "window-status";
    el.appendChild(statusEl);

    container.appendChild(el);
    this.windowElements.set(window.id, el);
  }

  private renderSplit(
    split: {
      direction: SplitDirection;
      children: WindowTree[];
      bounds: Rect;
    },
    container: HTMLElement,
    focusedId: number
  ): void {
    const splitEl = document.createElement("div");
    splitEl.className = `split split-${split.direction.toLowerCase()}`;
    this.applyBounds(splitEl, split.bounds);

    // Render children
    split.children.forEach((child, i) => {
      this.renderNode(child, splitEl, focusedId);

      // Add separator between children
      if (this.options.showSeparators && i < split.children.length - 1) {
        const sep = this.createSeparator(split.direction, i);
        splitEl.appendChild(sep);
      }
    });

    container.appendChild(splitEl);
  }

  private createSeparator(direction: SplitDirection, index: number): HTMLElement {
    const sep = document.createElement("div");
    sep.className = `separator separator-${direction.toLowerCase()}`;
    sep.dataset.separatorIndex = index.toString();

    // Separators can be draggable for resize (future feature)
    sep.setAttribute("role", "separator");
    sep.setAttribute("aria-orientation", direction === "Horizontal" ? "horizontal" : "vertical");

    return sep;
  }

  private renderTabs(
    tabs: {
      tabs: WindowTree[];
      active: number;
      bounds: Rect;
    },
    container: HTMLElement,
    focusedId: number
  ): void {
    const tabsEl = document.createElement("div");
    tabsEl.className = "tabs";
    this.applyBounds(tabsEl, tabs.bounds);

    // Tab bar
    const tabBar = document.createElement("div");
    tabBar.className = "tab-bar";
    tabBar.setAttribute("role", "tablist");

    tabs.tabs.forEach((_, i) => {
      const tab = document.createElement("div");
      tab.className = "tab" + (i === tabs.active ? " active" : "");
      tab.textContent = `Tab ${i + 1}`;
      tab.dataset.tabIndex = i.toString();
      tab.setAttribute("role", "tab");
      tab.setAttribute("aria-selected", (i === tabs.active).toString());
      tab.tabIndex = i === tabs.active ? 0 : -1;
      tabBar.appendChild(tab);
    });
    tabsEl.appendChild(tabBar);

    // Active tab content
    const activeTab = tabs.tabs[tabs.active];
    if (activeTab) {
      const contentEl = document.createElement("div");
      contentEl.className = "tab-content";
      contentEl.setAttribute("role", "tabpanel");
      this.renderNode(activeTab, contentEl, focusedId);
      tabsEl.appendChild(contentEl);
    }

    container.appendChild(tabsEl);
  }

  private applyBounds(el: HTMLElement, bounds: Rect): void {
    const { charWidth, lineHeight } = this.options;
    Object.assign(el.style, {
      position: "absolute",
      left: `${bounds.x * charWidth}px`,
      top: `${bounds.y * lineHeight}px`,
      width: `${bounds.width * charWidth}px`,
      height: `${bounds.height * lineHeight}px`,
    });
  }

  /**
   * Update focus state without re-rendering the entire tree.
   *
   * @param newFocusedId - The ID of the newly focused window
   */
  updateFocus(newFocusedId: number): void {
    for (const [id, el] of this.windowElements) {
      if (id === newFocusedId) {
        el.classList.add("focused");
      } else {
        el.classList.remove("focused");
      }
    }
  }

  /**
   * Calculate screen dimensions in character cells.
   *
   * @param container - The container element
   * @returns Size in character cells { width, height }
   */
  getScreenSize(container: HTMLElement): { width: number; height: number } {
    const rect = container.getBoundingClientRect();
    return {
      width: Math.floor(rect.width / this.options.charWidth),
      height: Math.floor(rect.height / this.options.lineHeight),
    };
  }
}
