/**
 * WASM Convert Tests
 *
 * Tests for proto-to-model conversion utilities.
 * Uses mocked proto types (no actual proto dependencies needed for pure conversion logic).
 */

import { describe, it, expect } from "vitest";
import {
  protoToLogicalLayout,
  layoutNotificationToLogical,
  isProtoLeaf,
  isProtoSplit,
  getProtoBufferId,
  getProtoWindowId,
  getAllProtoWindowIds,
  countProtoWindows,
} from "../src/wasm/convert.js";

// Mock proto types that match the generated structure
type MockWindowNode = {
  node:
    | { case: "leaf"; value: { windowId: bigint; bufferId: bigint } }
    | { case: "split"; value: { direction: number; children: MockWindowNode[] } }
    | { case: undefined };
};

// Proto enum values: HORIZONTAL = 1, VERTICAL = 2
const SPLIT_HORIZONTAL = 1;
const SPLIT_VERTICAL = 2;

function createMockLeaf(windowId: bigint, bufferId: bigint): MockWindowNode {
  return {
    node: {
      case: "leaf",
      value: { windowId, bufferId },
    },
  };
}

function createMockSplit(
  direction: number,
  children: MockWindowNode[]
): MockWindowNode {
  return {
    node: {
      case: "split",
      value: { direction, children },
    },
  };
}

function createMockEmpty(): MockWindowNode {
  return {
    node: { case: undefined },
  };
}

describe("protoToLogicalLayout", () => {
  it("converts leaf to Single layout", () => {
    const proto = createMockLeaf(1n, 100n);
    const layout = protoToLogicalLayout(proto as never);

    expect(layout).not.toBeNull();
    expect(layout).toHaveProperty("Single");
    if (layout && "Single" in layout) {
      expect(layout.Single.viewport_id).toBe(1);
      expect(layout.Single.buffer_id).toBe(100);
    }
  });

  it("converts vertical split", () => {
    const left = createMockLeaf(1n, 100n);
    const right = createMockLeaf(2n, 200n);
    const proto = createMockSplit(SPLIT_VERTICAL, [left, right]);

    const layout = protoToLogicalLayout(proto as never);

    expect(layout).not.toBeNull();
    expect(layout).toHaveProperty("Split");
    if (layout && "Split" in layout) {
      expect(layout.Split.direction).toBe("Vertical");
      expect(layout.Split.children).toHaveLength(2);
      expect(layout.Split.ratios).toEqual([0.5, 0.5]);
    }
  });

  it("converts horizontal split", () => {
    const top = createMockLeaf(1n, 100n);
    const bottom = createMockLeaf(2n, 200n);
    const proto = createMockSplit(SPLIT_HORIZONTAL, [top, bottom]);

    const layout = protoToLogicalLayout(proto as never);

    expect(layout).not.toBeNull();
    expect(layout).toHaveProperty("Split");
    if (layout && "Split" in layout) {
      expect(layout.Split.direction).toBe("Horizontal");
      expect(layout.Split.children).toHaveLength(2);
    }
  });

  it("converts nested splits", () => {
    // left | (top / bottom)
    const left = createMockLeaf(1n, 100n);
    const top = createMockLeaf(2n, 200n);
    const bottom = createMockLeaf(3n, 300n);
    const rightSplit = createMockSplit(SPLIT_HORIZONTAL, [top, bottom]);
    const proto = createMockSplit(SPLIT_VERTICAL, [left, rightSplit]);

    const layout = protoToLogicalLayout(proto as never);

    expect(layout).not.toBeNull();
    expect(layout).toHaveProperty("Split");
    if (layout && "Split" in layout) {
      expect(layout.Split.direction).toBe("Vertical");
      expect(layout.Split.children).toHaveLength(2);

      const rightChild = layout.Split.children[1];
      expect(rightChild).toHaveProperty("Split");
      if ("Split" in rightChild) {
        expect(rightChild.Split.direction).toBe("Horizontal");
        expect(rightChild.Split.children).toHaveLength(2);
      }
    }
  });

  it("returns null for empty node", () => {
    const proto = createMockEmpty();
    const layout = protoToLogicalLayout(proto as never);
    expect(layout).toBeNull();
  });

  it("returns null for split with no valid children", () => {
    const empty1 = createMockEmpty();
    const empty2 = createMockEmpty();
    const proto = createMockSplit(SPLIT_VERTICAL, [empty1, empty2]);

    const layout = protoToLogicalLayout(proto as never);
    expect(layout).toBeNull();
  });

  it("defaults to Vertical for unknown split direction", () => {
    const left = createMockLeaf(1n, 100n);
    const right = createMockLeaf(2n, 200n);
    const proto = createMockSplit(0, [left, right]); // 0 = UNSPECIFIED

    const layout = protoToLogicalLayout(proto as never);

    expect(layout).not.toBeNull();
    if (layout && "Split" in layout) {
      expect(layout.Split.direction).toBe("Vertical");
    }
  });

  it("computes equal ratios for children", () => {
    const children = [
      createMockLeaf(1n, 100n),
      createMockLeaf(2n, 200n),
      createMockLeaf(3n, 300n),
    ];
    const proto = createMockSplit(SPLIT_VERTICAL, children);

    const layout = protoToLogicalLayout(proto as never);

    if (layout && "Split" in layout) {
      expect(layout.Split.ratios).toHaveLength(3);
      expect(layout.Split.ratios[0]).toBeCloseTo(1 / 3);
      expect(layout.Split.ratios[1]).toBeCloseTo(1 / 3);
      expect(layout.Split.ratios[2]).toBeCloseTo(1 / 3);
    }
  });
});

describe("layoutNotificationToLogical", () => {
  it("creates Single layout for focused window", () => {
    const windows = [
      { windowId: 1n, bufferId: 100n },
      { windowId: 2n, bufferId: 200n },
    ];

    const layout = layoutNotificationToLogical(windows, 2n);

    expect(layout).not.toBeNull();
    if (layout && "Single" in layout) {
      expect(layout.Single.viewport_id).toBe(2);
      expect(layout.Single.buffer_id).toBe(200);
    }
  });

  it("falls back to first window if focused not found", () => {
    const windows = [
      { windowId: 1n, bufferId: 100n },
      { windowId: 2n, bufferId: 200n },
    ];

    const layout = layoutNotificationToLogical(windows, 999n);

    expect(layout).not.toBeNull();
    if (layout && "Single" in layout) {
      expect(layout.Single.viewport_id).toBe(1);
      expect(layout.Single.buffer_id).toBe(100);
    }
  });

  it("returns null for empty window list", () => {
    const layout = layoutNotificationToLogical([], 1n);
    expect(layout).toBeNull();
  });
});

describe("Proto helper functions", () => {
  describe("isProtoLeaf", () => {
    it("returns true for leaf nodes", () => {
      const leaf = createMockLeaf(1n, 100n);
      expect(isProtoLeaf(leaf as never)).toBe(true);
    });

    it("returns false for split nodes", () => {
      const split = createMockSplit(SPLIT_VERTICAL, []);
      expect(isProtoLeaf(split as never)).toBe(false);
    });

    it("returns false for empty nodes", () => {
      const empty = createMockEmpty();
      expect(isProtoLeaf(empty as never)).toBe(false);
    });
  });

  describe("isProtoSplit", () => {
    it("returns true for split nodes", () => {
      const split = createMockSplit(SPLIT_VERTICAL, []);
      expect(isProtoSplit(split as never)).toBe(true);
    });

    it("returns false for leaf nodes", () => {
      const leaf = createMockLeaf(1n, 100n);
      expect(isProtoSplit(leaf as never)).toBe(false);
    });
  });

  describe("getProtoBufferId", () => {
    it("returns buffer id from leaf", () => {
      const leaf = createMockLeaf(1n, 100n);
      expect(getProtoBufferId(leaf as never)).toBe(100n);
    });

    it("returns null from split", () => {
      const split = createMockSplit(SPLIT_VERTICAL, []);
      expect(getProtoBufferId(split as never)).toBeNull();
    });
  });

  describe("getProtoWindowId", () => {
    it("returns window id from leaf", () => {
      const leaf = createMockLeaf(42n, 100n);
      expect(getProtoWindowId(leaf as never)).toBe(42n);
    });

    it("returns null from split", () => {
      const split = createMockSplit(SPLIT_VERTICAL, []);
      expect(getProtoWindowId(split as never)).toBeNull();
    });
  });

  describe("getAllProtoWindowIds", () => {
    it("returns single id from leaf", () => {
      const leaf = createMockLeaf(42n, 100n);
      expect(getAllProtoWindowIds(leaf as never)).toEqual([42n]);
    });

    it("returns all ids from split", () => {
      const left = createMockLeaf(1n, 100n);
      const right = createMockLeaf(2n, 200n);
      const split = createMockSplit(SPLIT_VERTICAL, [left, right]);

      expect(getAllProtoWindowIds(split as never)).toEqual([1n, 2n]);
    });

    it("returns all ids from nested splits", () => {
      const left = createMockLeaf(1n, 100n);
      const top = createMockLeaf(2n, 200n);
      const bottom = createMockLeaf(3n, 300n);
      const rightSplit = createMockSplit(SPLIT_HORIZONTAL, [top, bottom]);
      const root = createMockSplit(SPLIT_VERTICAL, [left, rightSplit]);

      expect(getAllProtoWindowIds(root as never)).toEqual([1n, 2n, 3n]);
    });
  });

  describe("countProtoWindows", () => {
    it("returns 1 for leaf", () => {
      const leaf = createMockLeaf(1n, 100n);
      expect(countProtoWindows(leaf as never)).toBe(1);
    });

    it("counts windows in split", () => {
      const left = createMockLeaf(1n, 100n);
      const right = createMockLeaf(2n, 200n);
      const split = createMockSplit(SPLIT_VERTICAL, [left, right]);

      expect(countProtoWindows(split as never)).toBe(2);
    });

    it("counts windows in nested structure", () => {
      const left = createMockLeaf(1n, 100n);
      const top = createMockLeaf(2n, 200n);
      const bottom = createMockLeaf(3n, 300n);
      const rightSplit = createMockSplit(SPLIT_HORIZONTAL, [top, bottom]);
      const root = createMockSplit(SPLIT_VERTICAL, [left, rightSplit]);

      expect(countProtoWindows(root as never)).toBe(3);
    });

    it("returns 0 for empty node", () => {
      const empty = createMockEmpty();
      expect(countProtoWindows(empty as never)).toBe(0);
    });
  });
});
