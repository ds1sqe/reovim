/**
 * ViewportCache Tests
 *
 * Tests for viewport state caching with incremental updates.
 * Part of Phase 11.3 - Web Client Cache & Overlay Unit Tests.
 */

import { describe, it, expect, beforeEach } from "vitest";
import { ViewportCache } from "../src/cache/viewport.js";
import type { ViewportState, ViewportUpdate } from "../src/wasm/index.js";

// ============ Test Fixtures ============

/**
 * Create a viewport state for testing.
 */
function createViewportState(
  id: number,
  overrides: Partial<ViewportState> = {}
): ViewportState {
  return {
    id,
    buffer_id: 1,
    top_line: 0,
    left_col: 0,
    cursor_line: 0,
    cursor_col: 0,
    width: 80,
    height: 24,
    ...overrides,
  };
}

/**
 * Create a viewport update for testing.
 */
function createViewportUpdate(
  viewport_id: number,
  updates: Partial<Omit<ViewportUpdate, "viewport_id">> = {}
): ViewportUpdate {
  return { viewport_id, ...updates };
}

// ============ Tests ============

describe("ViewportCache", () => {
  let cache: ViewportCache;

  beforeEach(() => {
    cache = new ViewportCache();
  });

  describe("get/set", () => {
    it("returns undefined for missing viewport", () => {
      expect(cache.get(999)).toBeUndefined();
    });

    it("stores and retrieves viewport state", () => {
      const state = createViewportState(1, { cursor_line: 10, cursor_col: 5 });
      cache.set(state);

      const retrieved = cache.get(1);
      expect(retrieved).toBeDefined();
      expect(retrieved?.id).toBe(1);
      expect(retrieved?.cursor_line).toBe(10);
      expect(retrieved?.cursor_col).toBe(5);
    });

    it("stores copy to prevent external mutation", () => {
      const state = createViewportState(1);
      cache.set(state);

      // Mutate original
      state.cursor_line = 999;

      // Cache should be unaffected
      const retrieved = cache.get(1);
      expect(retrieved?.cursor_line).toBe(0);
    });

    it("overwrites existing viewport with same id", () => {
      const state1 = createViewportState(1, { cursor_line: 5 });
      const state2 = createViewportState(1, { cursor_line: 10 });

      cache.set(state1);
      cache.set(state2);

      expect(cache.get(1)?.cursor_line).toBe(10);
      expect(cache.size).toBe(1);
    });
  });

  describe("has", () => {
    it("returns false for missing viewport", () => {
      expect(cache.has(999)).toBe(false);
    });

    it("returns true for cached viewport", () => {
      cache.set(createViewportState(1));
      expect(cache.has(1)).toBe(true);
    });
  });

  describe("applyUpdate", () => {
    it("returns false when viewport not in cache", () => {
      const update = createViewportUpdate(999, { cursor_line: 10 });
      expect(cache.applyUpdate(update)).toBe(false);
    });

    it("updates top_line when provided", () => {
      cache.set(createViewportState(1, { top_line: 0 }));
      cache.applyUpdate(createViewportUpdate(1, { top_line: 50 }));

      expect(cache.get(1)?.top_line).toBe(50);
    });

    it("updates left_col when provided", () => {
      cache.set(createViewportState(1, { left_col: 0 }));
      cache.applyUpdate(createViewportUpdate(1, { left_col: 20 }));

      expect(cache.get(1)?.left_col).toBe(20);
    });

    it("updates cursor_line when provided", () => {
      cache.set(createViewportState(1, { cursor_line: 0 }));
      cache.applyUpdate(createViewportUpdate(1, { cursor_line: 15 }));

      expect(cache.get(1)?.cursor_line).toBe(15);
    });

    it("updates cursor_col when provided", () => {
      cache.set(createViewportState(1, { cursor_col: 0 }));
      cache.applyUpdate(createViewportUpdate(1, { cursor_col: 42 }));

      expect(cache.get(1)?.cursor_col).toBe(42);
    });

    it("applies partial update (only changed fields)", () => {
      cache.set(
        createViewportState(1, {
          top_line: 10,
          left_col: 5,
          cursor_line: 20,
          cursor_col: 15,
        })
      );

      // Only update cursor position
      cache.applyUpdate(createViewportUpdate(1, { cursor_line: 25 }));

      const state = cache.get(1);
      expect(state?.top_line).toBe(10); // unchanged
      expect(state?.left_col).toBe(5); // unchanged
      expect(state?.cursor_line).toBe(25); // updated
      expect(state?.cursor_col).toBe(15); // unchanged
    });

    it("leaves unchanged fields intact", () => {
      const original = createViewportState(1, {
        buffer_id: 100,
        width: 120,
        height: 40,
      });
      cache.set(original);

      cache.applyUpdate(createViewportUpdate(1, { top_line: 5 }));

      const state = cache.get(1);
      expect(state?.buffer_id).toBe(100);
      expect(state?.width).toBe(120);
      expect(state?.height).toBe(40);
    });

    it("returns true on successful update", () => {
      cache.set(createViewportState(1));
      const result = cache.applyUpdate(createViewportUpdate(1, { cursor_line: 10 }));

      expect(result).toBe(true);
    });

    it("ignores null values in update", () => {
      cache.set(createViewportState(1, { cursor_line: 10 }));

      // Update with explicit null should not change value
      cache.applyUpdate({ viewport_id: 1, cursor_line: null } as ViewportUpdate);

      expect(cache.get(1)?.cursor_line).toBe(10);
    });
  });

  describe("delete", () => {
    it("removes viewport from cache", () => {
      cache.set(createViewportState(1));
      cache.delete(1);

      expect(cache.get(1)).toBeUndefined();
      expect(cache.has(1)).toBe(false);
    });

    it("returns true when viewport existed", () => {
      cache.set(createViewportState(1));
      expect(cache.delete(1)).toBe(true);
    });

    it("returns false when viewport did not exist", () => {
      expect(cache.delete(999)).toBe(false);
    });
  });

  describe("clear", () => {
    it("removes all viewports", () => {
      cache.set(createViewportState(1));
      cache.set(createViewportState(2));
      cache.set(createViewportState(3));

      cache.clear();

      expect(cache.size).toBe(0);
      expect(cache.get(1)).toBeUndefined();
      expect(cache.get(2)).toBeUndefined();
      expect(cache.get(3)).toBeUndefined();
    });

    it("keys() returns empty after clear", () => {
      cache.set(createViewportState(1));
      cache.set(createViewportState(2));

      cache.clear();

      expect(cache.keys()).toEqual([]);
    });
  });

  describe("keys", () => {
    it("returns empty array when cache is empty", () => {
      expect(cache.keys()).toEqual([]);
    });

    it("returns all cached viewport IDs", () => {
      cache.set(createViewportState(1));
      cache.set(createViewportState(5));
      cache.set(createViewportState(10));

      const keys = cache.keys();
      expect(keys).toHaveLength(3);
      expect(keys).toContain(1);
      expect(keys).toContain(5);
      expect(keys).toContain(10);
    });
  });

  describe("size", () => {
    it("returns 0 for empty cache", () => {
      expect(cache.size).toBe(0);
    });

    it("returns correct count after operations", () => {
      cache.set(createViewportState(1));
      expect(cache.size).toBe(1);

      cache.set(createViewportState(2));
      expect(cache.size).toBe(2);

      cache.delete(1);
      expect(cache.size).toBe(1);

      cache.clear();
      expect(cache.size).toBe(0);
    });
  });

  describe("forEach", () => {
    it("iterates over all cached viewports", () => {
      cache.set(createViewportState(1, { cursor_line: 10 }));
      cache.set(createViewportState(2, { cursor_line: 20 }));

      const seen: Array<{ id: number; cursorLine: number }> = [];
      cache.forEach((state, id) => {
        seen.push({ id, cursorLine: state.cursor_line });
      });

      expect(seen).toHaveLength(2);
      expect(seen).toContainEqual({ id: 1, cursorLine: 10 });
      expect(seen).toContainEqual({ id: 2, cursorLine: 20 });
    });
  });
});
