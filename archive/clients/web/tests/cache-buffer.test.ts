/**
 * BufferCache Tests
 *
 * Tests for buffer content caching with version tracking.
 * Part of Phase 11.3 - Web Client Cache & Overlay Unit Tests.
 */

import { describe, it, expect, beforeEach } from "vitest";
import { BufferCache } from "../src/cache/buffer.js";

// ============ Test Fixtures ============

/**
 * Create an array of test lines.
 */
function createBufferLines(count: number): string[] {
  return Array.from({ length: count }, (_, i) => `Line ${i + 1}`);
}

// ============ Tests ============

describe("BufferCache", () => {
  let cache: BufferCache;

  beforeEach(() => {
    cache = new BufferCache();
  });

  describe("get/set", () => {
    it("returns undefined for missing buffer", () => {
      expect(cache.get(999)).toBeUndefined();
    });

    it("stores and retrieves buffer lines", () => {
      const lines = ["hello", "world"];
      cache.set(1, lines, 1);

      const retrieved = cache.get(1);
      expect(retrieved).toEqual(["hello", "world"]);
    });

    it("returns copy of lines array (defensive copy on set)", () => {
      const lines = ["original"];
      cache.set(1, lines, 1);

      // Mutate original
      lines.push("added");

      // Cache should be unaffected
      expect(cache.get(1)).toEqual(["original"]);
    });

    it("stores version with content", () => {
      cache.set(1, ["line"], 42);
      expect(cache.getVersion(1)).toBe(42);
    });

    it("overwrites existing buffer with same id", () => {
      cache.set(1, ["old"], 1);
      cache.set(1, ["new"], 2);

      expect(cache.get(1)).toEqual(["new"]);
      expect(cache.getVersion(1)).toBe(2);
    });
  });

  describe("has", () => {
    it("returns false for missing buffer", () => {
      expect(cache.has(999)).toBe(false);
    });

    it("returns true for cached buffer", () => {
      cache.set(1, ["line"], 1);
      expect(cache.has(1)).toBe(true);
    });
  });

  describe("getVersion", () => {
    it("returns -1 for missing buffer", () => {
      expect(cache.getVersion(999)).toBe(-1);
    });

    it("returns stored version number", () => {
      cache.set(1, ["line"], 5);
      expect(cache.getVersion(1)).toBe(5);
    });
  });

  describe("needsRefresh", () => {
    it("returns true when buffer not cached", () => {
      expect(cache.needsRefresh(999, 1)).toBe(true);
    });

    it("returns true when server version is higher", () => {
      cache.set(1, ["line"], 5);
      expect(cache.needsRefresh(1, 10)).toBe(true);
    });

    it("returns false when versions match", () => {
      cache.set(1, ["line"], 5);
      expect(cache.needsRefresh(1, 5)).toBe(false);
    });

    it("returns false when cached version is higher", () => {
      cache.set(1, ["line"], 10);
      expect(cache.needsRefresh(1, 5)).toBe(false);
    });
  });

  describe("invalidate", () => {
    it("removes buffer from cache", () => {
      cache.set(1, ["line"], 1);
      cache.invalidate(1);

      expect(cache.has(1)).toBe(false);
    });

    it("returns true when buffer existed", () => {
      cache.set(1, ["line"], 1);
      expect(cache.invalidate(1)).toBe(true);
    });

    it("returns false when buffer did not exist", () => {
      expect(cache.invalidate(999)).toBe(false);
    });

    it("get() returns undefined after invalidate", () => {
      cache.set(1, ["line"], 1);
      cache.invalidate(1);

      expect(cache.get(1)).toBeUndefined();
    });
  });

  describe("invalidateAll", () => {
    it("removes all buffers", () => {
      cache.set(1, ["a"], 1);
      cache.set(2, ["b"], 1);
      cache.set(3, ["c"], 1);

      cache.invalidateAll();

      expect(cache.size).toBe(0);
      expect(cache.has(1)).toBe(false);
      expect(cache.has(2)).toBe(false);
      expect(cache.has(3)).toBe(false);
    });
  });

  describe("getVisibleLines", () => {
    it("returns empty array for missing buffer", () => {
      expect(cache.getVisibleLines(999, 0, 10)).toEqual([]);
    });

    it("returns lines from topLine to topLine+height", () => {
      cache.set(1, createBufferLines(10), 1);

      const visible = cache.getVisibleLines(1, 2, 3);
      expect(visible).toEqual(["Line 3", "Line 4", "Line 5"]);
    });

    it("clamps to buffer length when height exceeds", () => {
      cache.set(1, createBufferLines(5), 1);

      const visible = cache.getVisibleLines(1, 3, 10);
      expect(visible).toEqual(["Line 4", "Line 5"]);
    });

    it("returns empty when topLine beyond buffer", () => {
      cache.set(1, createBufferLines(5), 1);

      const visible = cache.getVisibleLines(1, 100, 10);
      expect(visible).toEqual([]);
    });

    it("handles zero topLine correctly", () => {
      cache.set(1, createBufferLines(10), 1);

      const visible = cache.getVisibleLines(1, 0, 3);
      expect(visible).toEqual(["Line 1", "Line 2", "Line 3"]);
    });

    it("handles negative topLine by clamping to 0", () => {
      cache.set(1, createBufferLines(5), 1);

      const visible = cache.getVisibleLines(1, -5, 3);
      expect(visible).toEqual(["Line 1", "Line 2", "Line 3"]);
    });
  });

  describe("getLineCount", () => {
    it("returns 0 for missing buffer", () => {
      expect(cache.getLineCount(999)).toBe(0);
    });

    it("returns correct line count", () => {
      cache.set(1, createBufferLines(42), 1);
      expect(cache.getLineCount(1)).toBe(42);
    });

    it("returns 0 for empty buffer", () => {
      cache.set(1, [], 1);
      expect(cache.getLineCount(1)).toBe(0);
    });
  });

  describe("getLine", () => {
    it("returns undefined for missing buffer", () => {
      expect(cache.getLine(999, 0)).toBeUndefined();
    });

    it("returns correct line content", () => {
      cache.set(1, ["zero", "one", "two"], 1);
      expect(cache.getLine(1, 1)).toBe("one");
    });

    it("returns undefined for negative line number", () => {
      cache.set(1, ["line"], 1);
      expect(cache.getLine(1, -1)).toBeUndefined();
    });

    it("returns undefined for line beyond buffer", () => {
      cache.set(1, ["line"], 1);
      expect(cache.getLine(1, 100)).toBeUndefined();
    });
  });

  describe("clear", () => {
    it("removes all buffers", () => {
      cache.set(1, ["a"], 1);
      cache.set(2, ["b"], 1);

      cache.clear();

      expect(cache.size).toBe(0);
      expect(cache.get(1)).toBeUndefined();
      expect(cache.get(2)).toBeUndefined();
    });
  });

  describe("keys", () => {
    it("returns empty array when cache is empty", () => {
      expect(cache.keys()).toEqual([]);
    });

    it("returns all cached buffer IDs", () => {
      cache.set(1, ["a"], 1);
      cache.set(5, ["b"], 1);
      cache.set(10, ["c"], 1);

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
      cache.set(1, ["a"], 1);
      expect(cache.size).toBe(1);

      cache.set(2, ["b"], 1);
      expect(cache.size).toBe(2);

      cache.invalidate(1);
      expect(cache.size).toBe(1);

      cache.clear();
      expect(cache.size).toBe(0);
    });
  });

  describe("getEntry", () => {
    it("returns undefined for missing buffer", () => {
      expect(cache.getEntry(999)).toBeUndefined();
    });

    it("returns full entry with metadata", () => {
      cache.set(1, ["line"], 5);

      const entry = cache.getEntry(1);
      expect(entry).toBeDefined();
      expect(entry?.lines).toEqual(["line"]);
      expect(entry?.version).toBe(5);
      expect(entry?.lastUpdated).toBeGreaterThan(0);
    });
  });

  describe("getStats", () => {
    it("returns zero stats for empty cache", () => {
      const stats = cache.getStats();
      expect(stats.bufferCount).toBe(0);
      expect(stats.totalLines).toBe(0);
      expect(stats.oldestUpdate).toBeNull();
      expect(stats.newestUpdate).toBeNull();
    });

    it("returns correct stats for populated cache", () => {
      cache.set(1, createBufferLines(10), 1);
      cache.set(2, createBufferLines(20), 1);

      const stats = cache.getStats();
      expect(stats.bufferCount).toBe(2);
      expect(stats.totalLines).toBe(30);
      expect(stats.oldestUpdate).toBeGreaterThan(0);
      expect(stats.newestUpdate).toBeGreaterThan(0);
    });
  });
});
