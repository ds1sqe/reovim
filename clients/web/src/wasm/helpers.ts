// TypeScript-only helpers for WindowTree traversal.
//
// These are pure TypeScript functions (no WASM calls) for simpler usage
// and tree-shaking. They provide type guards and utility functions for
// working with the WindowTree discriminated union.

import type { WindowTree, Window, SplitDirection, Rect } from "./bindings.js";

// ============ Type Guards ============

/**
 * Type guard: check if tree is a leaf node (single window).
 */
export function isLeaf(tree: WindowTree): tree is { Leaf: Window } {
  return "Leaf" in tree;
}

/**
 * Type guard: check if tree is a split node (multiple windows).
 */
export function isSplit(tree: WindowTree): tree is {
  Split: {
    direction: SplitDirection;
    children: WindowTree[];
    bounds: Rect;
  };
} {
  return "Split" in tree;
}

/**
 * Type guard: check if tree is a tabs node (tabbed windows).
 */
export function isTabs(tree: WindowTree): tree is {
  Tabs: {
    tabs: WindowTree[];
    active: number;
    bounds: Rect;
  };
} {
  return "Tabs" in tree;
}

// ============ Tree Traversal ============

/**
 * Collect all windows from a tree (flattens the hierarchy).
 *
 * This is useful when you need to iterate over all windows
 * regardless of their layout structure.
 *
 * @param tree - The window tree to flatten
 * @returns Array of all windows in the tree
 */
export function allWindows(tree: WindowTree): Window[] {
  if (isLeaf(tree)) {
    return [tree.Leaf];
  } else if (isSplit(tree)) {
    return tree.Split.children.flatMap(allWindows);
  } else if (isTabs(tree)) {
    return tree.Tabs.tabs.flatMap(allWindows);
  }
  return [];
}

/**
 * Find a window by its ID.
 *
 * @param tree - The window tree to search
 * @param id - The window ID to find
 * @returns The window if found, null otherwise
 */
export function findWindow(tree: WindowTree, id: number): Window | null {
  for (const w of allWindows(tree)) {
    if (w.id === id) return w;
  }
  return null;
}

/**
 * Find a window by viewport ID.
 *
 * @param tree - The window tree to search
 * @param viewportId - The viewport ID to find
 * @returns The window if found, null otherwise
 */
export function findWindowByViewport(
  tree: WindowTree,
  viewportId: number
): Window | null {
  for (const w of allWindows(tree)) {
    if (w.viewport_id === viewportId) return w;
  }
  return null;
}

/**
 * Find a window by buffer ID.
 *
 * Note: Multiple windows may display the same buffer.
 * This returns the first match.
 *
 * @param tree - The window tree to search
 * @param bufferId - The buffer ID to find
 * @returns The first window displaying the buffer, null if none
 */
export function findWindowByBuffer(
  tree: WindowTree,
  bufferId: number
): Window | null {
  for (const w of allWindows(tree)) {
    if (w.buffer_id === bufferId) return w;
  }
  return null;
}

/**
 * Get the focused window from a tree.
 *
 * @param tree - The window tree
 * @param focusedId - The ID of the focused window
 * @returns The focused window if found, null otherwise
 */
export function focusedWindow(tree: WindowTree, focusedId: number): Window | null {
  return findWindow(tree, focusedId);
}

/**
 * Count total windows in a tree.
 *
 * @param tree - The window tree to count
 * @returns Number of windows in the tree
 */
export function windowCount(tree: WindowTree): number {
  if (isLeaf(tree)) {
    return 1;
  } else if (isSplit(tree)) {
    return tree.Split.children.reduce((sum, child) => sum + windowCount(child), 0);
  } else if (isTabs(tree)) {
    return tree.Tabs.tabs.reduce((sum, tab) => sum + windowCount(tab), 0);
  }
  return 0;
}

/**
 * Get the bounds of a window tree node.
 *
 * @param tree - The window tree node
 * @returns The bounding rectangle
 */
export function getBounds(tree: WindowTree): Rect {
  if (isLeaf(tree)) {
    return tree.Leaf.bounds;
  } else if (isSplit(tree)) {
    return tree.Split.bounds;
  } else if (isTabs(tree)) {
    return tree.Tabs.bounds;
  }
  // Fallback for exhaustiveness
  return { x: 0, y: 0, width: 0, height: 0 };
}

/**
 * Find the window at a screen position.
 *
 * @param tree - The window tree to search
 * @param x - X coordinate (column)
 * @param y - Y coordinate (row)
 * @returns The window at the position, null if none
 */
export function windowAtPosition(
  tree: WindowTree,
  x: number,
  y: number
): Window | null {
  if (isLeaf(tree)) {
    const bounds = tree.Leaf.bounds;
    if (
      x >= bounds.x &&
      x < bounds.x + bounds.width &&
      y >= bounds.y &&
      y < bounds.y + bounds.height
    ) {
      return tree.Leaf;
    }
    return null;
  } else if (isSplit(tree)) {
    for (const child of tree.Split.children) {
      const result = windowAtPosition(child, x, y);
      if (result) return result;
    }
    return null;
  } else if (isTabs(tree)) {
    // Only check the active tab
    const activeTab = tree.Tabs.tabs[tree.Tabs.active];
    if (activeTab) {
      return windowAtPosition(activeTab, x, y);
    }
    return null;
  }
  return null;
}

/**
 * Map over all windows in a tree, preserving structure.
 *
 * @param tree - The window tree
 * @param fn - Function to apply to each window
 * @returns New tree with transformed windows
 */
export function mapWindows(
  tree: WindowTree,
  fn: (window: Window) => Window
): WindowTree {
  if (isLeaf(tree)) {
    return { Leaf: fn(tree.Leaf) };
  } else if (isSplit(tree)) {
    return {
      Split: {
        ...tree.Split,
        children: tree.Split.children.map((child) => mapWindows(child, fn)),
      },
    };
  } else if (isTabs(tree)) {
    return {
      Tabs: {
        ...tree.Tabs,
        tabs: tree.Tabs.tabs.map((tab) => mapWindows(tab, fn)),
      },
    };
  }
  return tree;
}
