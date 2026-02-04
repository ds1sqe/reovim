/**
 * Layout Renderer Tests
 *
 * Tests for the DOM layout renderer.
 * Uses JSDOM-compatible DOM operations (vitest default environment).
 */

import { describe, it, expect, beforeEach } from "vitest";
import { LayoutRenderer } from "../src/render/layout.js";
import type { WindowTree, Window, Rect, SplitDirection } from "../src/wasm/bindings.js";

// Test fixtures
function createWindow(id: number, bufferId: number, bounds: Rect): Window {
  return {
    id,
    viewport_id: id,
    buffer_id: bufferId,
    bounds,
    focused: false,
  };
}

function createLeaf(window: Window): WindowTree {
  return { Leaf: window };
}

function createSplit(
  direction: SplitDirection,
  children: WindowTree[],
  bounds: Rect
): WindowTree {
  return { Split: { direction, children, bounds } };
}

function createTabs(tabs: WindowTree[], active: number, bounds: Rect): WindowTree {
  return { Tabs: { tabs, active, bounds } };
}

const testBounds: Rect = { x: 0, y: 0, width: 80, height: 24 };
const leftBounds: Rect = { x: 0, y: 0, width: 40, height: 24 };
const rightBounds: Rect = { x: 40, y: 0, width: 40, height: 24 };

describe("LayoutRenderer", () => {
  let container: HTMLElement;
  let renderer: LayoutRenderer;

  beforeEach(() => {
    container = document.createElement("div");
    container.style.width = "800px";
    container.style.height = "600px";
    document.body.appendChild(container);
    renderer = new LayoutRenderer();
  });

  describe("render single window", () => {
    it("creates window element", () => {
      const window = createWindow(1, 100, testBounds);
      const tree = createLeaf(window);

      renderer.render(tree, container, 1);

      const windowEl = container.querySelector(".window");
      expect(windowEl).not.toBeNull();
    });

    it("sets correct data attributes", () => {
      const window = createWindow(42, 100, testBounds);
      const tree = createLeaf(window);

      renderer.render(tree, container, 42);

      const windowEl = container.querySelector(".window") as HTMLElement;
      expect(windowEl.dataset.windowId).toBe("42");
      expect(windowEl.dataset.bufferId).toBe("100");
      expect(windowEl.dataset.viewportId).toBe("42");
    });

    it("marks focused window", () => {
      const window = createWindow(1, 100, testBounds);
      const tree = createLeaf(window);

      renderer.render(tree, container, 1);

      const windowEl = container.querySelector(".window");
      expect(windowEl?.classList.contains("focused")).toBe(true);
    });

    it("creates buffer and cursor elements", () => {
      const window = createWindow(1, 100, testBounds);
      const tree = createLeaf(window);

      renderer.render(tree, container, 1);

      expect(container.querySelector(".window-buffer")).not.toBeNull();
      expect(container.querySelector(".window-cursor")).not.toBeNull();
    });

    it("applies bounds as absolute positioning", () => {
      const bounds = { x: 10, y: 5, width: 60, height: 20 };
      const window = createWindow(1, 100, bounds);
      const tree = createLeaf(window);

      renderer.render(tree, container, 1);

      const windowEl = container.querySelector(".window") as HTMLElement;
      expect(windowEl.style.position).toBe("absolute");
      // Check left (x * charWidth where charWidth = 8.4)
      expect(windowEl.style.left).toBe("84px"); // 10 * 8.4
      // Check top (y * lineHeight where lineHeight = 21)
      expect(windowEl.style.top).toBe("105px"); // 5 * 21
    });
  });

  describe("render split", () => {
    it("creates multiple window elements for split", () => {
      const left = createLeaf(createWindow(1, 100, leftBounds));
      const right = createLeaf(createWindow(2, 200, rightBounds));
      const tree = createSplit("Vertical", [left, right], testBounds);

      renderer.render(tree, container, 1);

      const windows = container.querySelectorAll(".window");
      expect(windows).toHaveLength(2);
    });

    it("only marks one window as focused", () => {
      const left = createLeaf(createWindow(1, 100, leftBounds));
      const right = createLeaf(createWindow(2, 200, rightBounds));
      const tree = createSplit("Vertical", [left, right], testBounds);

      renderer.render(tree, container, 2);

      const focused = container.querySelectorAll(".window.focused");
      expect(focused).toHaveLength(1);
      expect((focused[0] as HTMLElement).dataset.windowId).toBe("2");
    });

    it("creates separator between windows", () => {
      const left = createLeaf(createWindow(1, 100, leftBounds));
      const right = createLeaf(createWindow(2, 200, rightBounds));
      const tree = createSplit("Vertical", [left, right], testBounds);

      renderer.render(tree, container, 1);

      const separator = container.querySelector(".separator-vertical");
      expect(separator).not.toBeNull();
    });

    it("creates split container with direction class", () => {
      const left = createLeaf(createWindow(1, 100, leftBounds));
      const right = createLeaf(createWindow(2, 200, rightBounds));
      const tree = createSplit("Vertical", [left, right], testBounds);

      renderer.render(tree, container, 1);

      const split = container.querySelector(".split-vertical");
      expect(split).not.toBeNull();
    });
  });

  describe("render tabs", () => {
    it("creates tab bar with tabs", () => {
      const tab1 = createLeaf(createWindow(1, 100, testBounds));
      const tab2 = createLeaf(createWindow(2, 200, testBounds));
      const tree = createTabs([tab1, tab2], 0, testBounds);

      renderer.render(tree, container, 1);

      const tabBar = container.querySelector(".tab-bar");
      expect(tabBar).not.toBeNull();
      expect(tabBar?.querySelectorAll(".tab")).toHaveLength(2);
    });

    it("marks active tab", () => {
      const tab1 = createLeaf(createWindow(1, 100, testBounds));
      const tab2 = createLeaf(createWindow(2, 200, testBounds));
      const tree = createTabs([tab1, tab2], 1, testBounds);

      renderer.render(tree, container, 2);

      const tabs = container.querySelectorAll(".tab");
      expect(tabs[0]?.classList.contains("active")).toBe(false);
      expect(tabs[1]?.classList.contains("active")).toBe(true);
    });

    it("renders only active tab content", () => {
      const tab1 = createLeaf(createWindow(1, 100, testBounds));
      const tab2 = createLeaf(createWindow(2, 200, testBounds));
      const tree = createTabs([tab1, tab2], 1, testBounds);

      renderer.render(tree, container, 2);

      // Only window 2 should be rendered (active tab)
      const windows = container.querySelectorAll(".window");
      expect(windows).toHaveLength(1);
      expect((windows[0] as HTMLElement).dataset.windowId).toBe("2");
    });
  });

  describe("getWindowElement", () => {
    it("retrieves window element by id", () => {
      const window = createWindow(42, 100, testBounds);
      const tree = createLeaf(window);

      renderer.render(tree, container, 42);

      const el = renderer.getWindowElement(42);
      expect(el).not.toBeNull();
      expect(el?.dataset.windowId).toBe("42");
    });

    it("returns undefined for non-existent window", () => {
      const window = createWindow(1, 100, testBounds);
      const tree = createLeaf(window);

      renderer.render(tree, container, 1);

      expect(renderer.getWindowElement(999)).toBeUndefined();
    });
  });

  describe("updateFocus", () => {
    it("moves focus to new window", () => {
      const left = createLeaf(createWindow(1, 100, leftBounds));
      const right = createLeaf(createWindow(2, 200, rightBounds));
      const tree = createSplit("Vertical", [left, right], testBounds);

      renderer.render(tree, container, 1);

      // Initially window 1 is focused
      let focused = container.querySelector(".window.focused") as HTMLElement;
      expect(focused.dataset.windowId).toBe("1");

      // Update focus to window 2
      renderer.updateFocus(2);

      focused = container.querySelector(".window.focused") as HTMLElement;
      expect(focused.dataset.windowId).toBe("2");
    });

    it("removes focus from previous window", () => {
      const left = createLeaf(createWindow(1, 100, leftBounds));
      const right = createLeaf(createWindow(2, 200, rightBounds));
      const tree = createSplit("Vertical", [left, right], testBounds);

      renderer.render(tree, container, 1);
      renderer.updateFocus(2);

      // Only one focused window
      const focused = container.querySelectorAll(".window.focused");
      expect(focused).toHaveLength(1);
    });
  });

  describe("options", () => {
    it("respects showSeparators option", () => {
      const left = createLeaf(createWindow(1, 100, leftBounds));
      const right = createLeaf(createWindow(2, 200, rightBounds));
      const tree = createSplit("Vertical", [left, right], testBounds);

      const noSepRenderer = new LayoutRenderer({ showSeparators: false });
      noSepRenderer.render(tree, container, 1);

      expect(container.querySelector(".separator")).toBeNull();
    });
  });
});
