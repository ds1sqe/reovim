/**
 * WASM Helpers Tests
 *
 * Tests for the TypeScript-only tree traversal utilities.
 * These tests don't require actual WASM - they test pure TypeScript functions.
 */

import { describe, it, expect } from "vitest";
import {
  isLeaf,
  isSplit,
  isTabs,
  allWindows,
  findWindow,
  findWindowByViewport,
  findWindowByBuffer,
  windowCount,
  getBounds,
  windowAtPosition,
  mapWindows,
} from "../src/wasm/helpers.js";
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
const topBounds: Rect = { x: 0, y: 0, width: 80, height: 12 };
const bottomBounds: Rect = { x: 0, y: 12, width: 80, height: 12 };

describe("WindowTree type guards", () => {
  it("isLeaf identifies leaf nodes", () => {
    const leaf = createLeaf(createWindow(1, 1, testBounds));
    const split = createSplit("Vertical", [], testBounds);
    const tabs = createTabs([], 0, testBounds);

    expect(isLeaf(leaf)).toBe(true);
    expect(isLeaf(split)).toBe(false);
    expect(isLeaf(tabs)).toBe(false);
  });

  it("isSplit identifies split nodes", () => {
    const leaf = createLeaf(createWindow(1, 1, testBounds));
    const split = createSplit("Vertical", [], testBounds);
    const tabs = createTabs([], 0, testBounds);

    expect(isSplit(leaf)).toBe(false);
    expect(isSplit(split)).toBe(true);
    expect(isSplit(tabs)).toBe(false);
  });

  it("isTabs identifies tab nodes", () => {
    const leaf = createLeaf(createWindow(1, 1, testBounds));
    const split = createSplit("Vertical", [], testBounds);
    const tabs = createTabs([], 0, testBounds);

    expect(isTabs(leaf)).toBe(false);
    expect(isTabs(split)).toBe(false);
    expect(isTabs(tabs)).toBe(true);
  });
});

describe("allWindows", () => {
  it("returns single window from leaf", () => {
    const window = createWindow(1, 1, testBounds);
    const tree = createLeaf(window);

    const windows = allWindows(tree);
    expect(windows).toHaveLength(1);
    expect(windows[0]).toEqual(window);
  });

  it("returns all windows from vertical split", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const right = createLeaf(createWindow(2, 2, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    const windows = allWindows(tree);
    expect(windows).toHaveLength(2);
    expect(windows.map((w) => w.id)).toEqual([1, 2]);
  });

  it("returns all windows from nested splits", () => {
    // Left | (Top / Bottom)
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const top = createLeaf(createWindow(2, 2, topBounds));
    const bottom = createLeaf(createWindow(3, 3, bottomBounds));
    const rightSplit = createSplit("Horizontal", [top, bottom], rightBounds);
    const tree = createSplit("Vertical", [left, rightSplit], testBounds);

    const windows = allWindows(tree);
    expect(windows).toHaveLength(3);
    expect(windows.map((w) => w.id)).toEqual([1, 2, 3]);
  });

  it("returns all windows from tabs", () => {
    const tab1 = createLeaf(createWindow(1, 1, testBounds));
    const tab2 = createLeaf(createWindow(2, 2, testBounds));
    const tree = createTabs([tab1, tab2], 0, testBounds);

    const windows = allWindows(tree);
    expect(windows).toHaveLength(2);
  });
});

describe("findWindow", () => {
  it("finds window by id in leaf", () => {
    const window = createWindow(42, 1, testBounds);
    const tree = createLeaf(window);

    expect(findWindow(tree, 42)).toEqual(window);
    expect(findWindow(tree, 99)).toBeNull();
  });

  it("finds window by id in split", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const right = createLeaf(createWindow(2, 2, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    expect(findWindow(tree, 1)?.id).toBe(1);
    expect(findWindow(tree, 2)?.id).toBe(2);
    expect(findWindow(tree, 99)).toBeNull();
  });
});

describe("findWindowByViewport", () => {
  it("finds window by viewport_id", () => {
    const window = createWindow(1, 100, testBounds);
    window.viewport_id = 42;
    const tree = createLeaf(window);

    expect(findWindowByViewport(tree, 42)?.id).toBe(1);
    expect(findWindowByViewport(tree, 99)).toBeNull();
  });
});

describe("findWindowByBuffer", () => {
  it("finds window displaying buffer", () => {
    const left = createLeaf(createWindow(1, 100, leftBounds));
    const right = createLeaf(createWindow(2, 200, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    expect(findWindowByBuffer(tree, 100)?.id).toBe(1);
    expect(findWindowByBuffer(tree, 200)?.id).toBe(2);
    expect(findWindowByBuffer(tree, 999)).toBeNull();
  });
});

describe("windowCount", () => {
  it("returns 1 for leaf", () => {
    const tree = createLeaf(createWindow(1, 1, testBounds));
    expect(windowCount(tree)).toBe(1);
  });

  it("counts windows in split", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const right = createLeaf(createWindow(2, 2, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    expect(windowCount(tree)).toBe(2);
  });

  it("counts windows in nested tree", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const top = createLeaf(createWindow(2, 2, topBounds));
    const bottom = createLeaf(createWindow(3, 3, bottomBounds));
    const rightSplit = createSplit("Horizontal", [top, bottom], rightBounds);
    const tree = createSplit("Vertical", [left, rightSplit], testBounds);

    expect(windowCount(tree)).toBe(3);
  });

  it("counts windows in tabs (all tabs)", () => {
    const tab1 = createLeaf(createWindow(1, 1, testBounds));
    const tab2 = createLeaf(createWindow(2, 2, testBounds));
    const tree = createTabs([tab1, tab2], 0, testBounds);

    expect(windowCount(tree)).toBe(2);
  });
});

describe("getBounds", () => {
  it("returns bounds for leaf", () => {
    const tree = createLeaf(createWindow(1, 1, testBounds));
    expect(getBounds(tree)).toEqual(testBounds);
  });

  it("returns bounds for split", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const right = createLeaf(createWindow(2, 2, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    expect(getBounds(tree)).toEqual(testBounds);
  });

  it("returns bounds for tabs", () => {
    const tab1 = createLeaf(createWindow(1, 1, testBounds));
    const tree = createTabs([tab1], 0, testBounds);

    expect(getBounds(tree)).toEqual(testBounds);
  });
});

describe("windowAtPosition", () => {
  it("finds window at position in leaf", () => {
    const window = createWindow(1, 1, testBounds);
    const tree = createLeaf(window);

    expect(windowAtPosition(tree, 40, 12)?.id).toBe(1);
    expect(windowAtPosition(tree, 0, 0)?.id).toBe(1);
  });

  it("returns null for position outside bounds", () => {
    const window = createWindow(1, 1, { x: 10, y: 10, width: 20, height: 10 });
    const tree = createLeaf(window);

    expect(windowAtPosition(tree, 0, 0)).toBeNull();
    expect(windowAtPosition(tree, 50, 50)).toBeNull();
  });

  it("finds correct window in split", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const right = createLeaf(createWindow(2, 2, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    expect(windowAtPosition(tree, 10, 10)?.id).toBe(1);
    expect(windowAtPosition(tree, 50, 10)?.id).toBe(2);
  });

  it("only checks active tab in tabs node", () => {
    const tab1Window = createWindow(1, 1, testBounds);
    const tab2Window = createWindow(2, 2, testBounds);
    const tab1 = createLeaf(tab1Window);
    const tab2 = createLeaf(tab2Window);

    // Active tab is index 1 (second tab)
    const tree = createTabs([tab1, tab2], 1, testBounds);

    // Position should find window 2 (active tab)
    expect(windowAtPosition(tree, 40, 12)?.id).toBe(2);
  });
});

describe("mapWindows", () => {
  it("transforms window in leaf", () => {
    const window = createWindow(1, 1, testBounds);
    const tree = createLeaf(window);

    const mapped = mapWindows(tree, (w) => ({ ...w, focused: true }));

    expect(isLeaf(mapped)).toBe(true);
    if (isLeaf(mapped)) {
      expect(mapped.Leaf.focused).toBe(true);
    }
  });

  it("transforms all windows in split", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const right = createLeaf(createWindow(2, 2, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    const mapped = mapWindows(tree, (w) => ({ ...w, buffer_id: 999 }));
    const windows = allWindows(mapped);

    expect(windows.every((w) => w.buffer_id === 999)).toBe(true);
  });

  it("preserves tree structure", () => {
    const left = createLeaf(createWindow(1, 1, leftBounds));
    const right = createLeaf(createWindow(2, 2, rightBounds));
    const tree = createSplit("Vertical", [left, right], testBounds);

    const mapped = mapWindows(tree, (w) => w);

    expect(isSplit(mapped)).toBe(true);
    if (isSplit(mapped)) {
      expect(mapped.Split.children).toHaveLength(2);
      expect(mapped.Split.direction).toBe("Vertical");
    }
  });
});
