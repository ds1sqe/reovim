/**
 * TokenCache Tests
 *
 * Tests for the syntax token cache with byte-to-position conversion.
 * Part of Phase 13.1 - Web Theme Engine.
 */

import { describe, it, expect, beforeEach } from "vitest";
import {
  TokenCache,
  TokenCacheManager,
} from "../src/syntax/index.js";
import type {
  TokenSpan,
  TokenUpdate,
  CachedToken,
} from "../src/syntax/index.js";

// ============ Test Fixtures ============

/**
 * Create a TokenUpdate from spans.
 */
function createUpdate(
  tokens: TokenSpan[],
  options: Partial<TokenUpdate> = {}
): TokenUpdate {
  return {
    bufferId: options.bufferId ?? 1n,
    tokens,
    startLine: options.startLine ?? 0,
    endLine: options.endLine ?? 0,
    fullRefresh: options.fullRefresh ?? false,
  };
}

/**
 * Simple ASCII content for basic tests.
 */
const ASCII_CONTENT = "fn main() {\n    println!(\"Hello\");\n}";
//                     0123456789 10 11...

/**
 * UTF-8 content with multi-byte characters.
 */
const UTF8_CONTENT = "let café = 42;\nlet 日本語 = true;";
//                   0123456789...

// ============ TokenCache Tests ============

describe("TokenCache", () => {
  let cache: TokenCache;

  beforeEach(() => {
    cache = new TokenCache();
  });

  describe("empty cache", () => {
    it("has size 0 initially", () => {
      expect(cache.size).toBe(0);
    });

    it("tokensForLine returns empty array", () => {
      expect(cache.tokensForLine(0)).toEqual([]);
      expect(cache.tokensForLine(100)).toEqual([]);
    });

    it("tokenAt returns null", () => {
      expect(cache.tokenAt(0, 0)).toBeNull();
    });
  });

  describe("applyUpdate - full refresh", () => {
    it("adds tokens from full refresh", () => {
      const update = createUpdate(
        [
          { startByte: 0, endByte: 2, category: "keyword" },
          { startByte: 3, endByte: 7, category: "function" },
        ],
        { fullRefresh: true }
      );

      cache.applyUpdate(update, "fn main");
      expect(cache.size).toBe(2);
    });

    it("clears existing tokens on full refresh", () => {
      // First update
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "keyword" }], {
          fullRefresh: true,
        }),
        "fn"
      );
      expect(cache.size).toBe(1);

      // Second full refresh replaces
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 1, category: "a" },
            { startByte: 2, endByte: 3, category: "b" },
            { startByte: 4, endByte: 5, category: "c" },
          ],
          { fullRefresh: true }
        ),
        "a b c"
      );
      expect(cache.size).toBe(3);
    });

    it("converts byte offsets to positions correctly", () => {
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "keyword" }], {
          fullRefresh: true,
        }),
        "fn main"
      );

      const tokens = cache.tokensForLine(0);
      expect(tokens.length).toBe(1);
      expect(tokens[0]).toEqual({
        line: 0,
        startCol: 0,
        endCol: 2,
        category: "keyword",
      });
    });
  });

  describe("applyUpdate - incremental", () => {
    it("adds tokens without clearing in incremental mode", () => {
      // Initial full refresh with tokens on line 0
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "keyword" }], {
          fullRefresh: true,
          startLine: 0,
          endLine: 0,
        }),
        "fn main()\n// comment\n"
      );

      // Incremental update adds tokens for line 1 only
      // The byte offset 11 is where "// comment" starts after "fn main()\n"
      cache.applyUpdate(
        createUpdate([{ startByte: 11, endByte: 21, category: "comment" }], {
          fullRefresh: false,
          startLine: 1,
          endLine: 1,
        }),
        "fn main()\n// comment\n"
      );

      // Line 0 token preserved, line 1 token added
      expect(cache.tokensForLine(0).length).toBe(1);
      expect(cache.tokensForLine(0)[0].category).toBe("keyword");
      expect(cache.tokensForLine(1).length).toBe(1);
      expect(cache.tokensForLine(1)[0].category).toBe("comment");
    });

    it("removes tokens in affected range during incremental update", () => {
      // Initial: tokens on lines 0 and 1
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 2, category: "a" }, // line 0
            { startByte: 3, endByte: 5, category: "b" }, // line 1
          ],
          { fullRefresh: true, startLine: 0, endLine: 1 }
        ),
        "aa\nbb"
      );

      expect(cache.tokensForLine(0).length).toBe(1);
      expect(cache.tokensForLine(1).length).toBe(1);

      // Incremental update replaces line 0 only
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "new" }], {
          fullRefresh: false,
          startLine: 0,
          endLine: 0,
        }),
        "cc\nbb"
      );

      // Line 0 replaced, line 1 preserved
      const line0 = cache.tokensForLine(0);
      const line1 = cache.tokensForLine(1);

      expect(line0.length).toBe(1);
      expect(line0[0].category).toBe("new");
      expect(line1.length).toBe(1);
      expect(line1[0].category).toBe("b");
    });
  });

  describe("byte-to-position conversion", () => {
    it("handles single-line ASCII content", () => {
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 2, category: "keyword" }, // fn
            { startByte: 3, endByte: 7, category: "function" }, // main
          ],
          { fullRefresh: true }
        ),
        "fn main()"
      );

      const tokens = cache.tokensForLine(0);
      expect(tokens[0]).toMatchObject({ startCol: 0, endCol: 2 });
      expect(tokens[1]).toMatchObject({ startCol: 3, endCol: 7 });
    });

    it("handles multi-line content", () => {
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 2, category: "keyword" }, // fn (line 0)
            { startByte: 12, endByte: 19, category: "macro" }, // println (line 1)
          ],
          { fullRefresh: true }
        ),
        ASCII_CONTENT
      );

      const line0 = cache.tokensForLine(0);
      const line1 = cache.tokensForLine(1);

      expect(line0.length).toBe(1);
      expect(line0[0].category).toBe("keyword");
      expect(line0[0].line).toBe(0);

      expect(line1.length).toBe(1);
      expect(line1[0].category).toBe("macro");
      expect(line1[0].line).toBe(1);
    });

    it("handles empty lines correctly", () => {
      const content = "a\n\nb";
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 1, category: "a" },
            { startByte: 3, endByte: 4, category: "b" },
          ],
          { fullRefresh: true }
        ),
        content
      );

      expect(cache.tokensForLine(0).length).toBe(1);
      expect(cache.tokensForLine(1).length).toBe(0); // Empty line
      expect(cache.tokensForLine(2).length).toBe(1);
    });
  });

  describe("UTF-8 handling", () => {
    it("handles ASCII correctly (1 byte per char)", () => {
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 5, category: "test" }], {
          fullRefresh: true,
        }),
        "hello"
      );

      const token = cache.tokensForLine(0)[0];
      expect(token.startCol).toBe(0);
      expect(token.endCol).toBe(5);
    });

    it("handles multi-byte UTF-8 characters", () => {
      // "café" = c(1) + a(1) + f(1) + é(2) = 5 bytes, 4 chars
      const content = "café";
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 5, category: "word" }], {
          fullRefresh: true,
        }),
        content
      );

      const token = cache.tokensForLine(0)[0];
      expect(token.startCol).toBe(0);
      expect(token.endCol).toBe(5); // Byte-based in this implementation
    });

    it("handles emoji (4-byte chars)", () => {
      // "👋" = 4 bytes
      const content = "hi 👋";
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "word" }], {
          fullRefresh: true,
        }),
        content
      );

      const token = cache.tokensForLine(0)[0];
      expect(token.startCol).toBe(0);
      expect(token.endCol).toBe(2);
    });

    it("handles CJK characters (3-byte chars)", () => {
      // Each Japanese char is 3 bytes
      const content = "日本";
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 6, category: "cjk" }], {
          fullRefresh: true,
        }),
        content
      );

      const token = cache.tokensForLine(0)[0];
      expect(token.startCol).toBe(0);
      expect(token.endCol).toBe(6); // 2 chars × 3 bytes
    });
  });

  describe("multi-line token handling", () => {
    it("truncates multi-line tokens to first line", () => {
      // Token spans across newline
      const content = "hello\nworld";
      cache.applyUpdate(
        createUpdate(
          [{ startByte: 0, endByte: 11, category: "multiline" }], // spans both lines
          { fullRefresh: true }
        ),
        content
      );

      // Should only appear on line 0, truncated
      const line0 = cache.tokensForLine(0);
      const line1 = cache.tokensForLine(1);

      expect(line0.length).toBe(1);
      expect(line0[0].line).toBe(0);
      expect(line0[0].endCol).toBe(5); // Truncated at newline

      // Line 1 gets the rest as separate token (if implementation does that)
      // Current implementation only keeps first line portion
      expect(line1.length).toBe(0);
    });
  });

  describe("tokensForLine", () => {
    beforeEach(() => {
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 2, category: "keyword" },
            { startByte: 3, endByte: 7, category: "function" },
            { startByte: 7, endByte: 8, category: "punctuation" },
            { startByte: 8, endByte: 9, category: "punctuation" },
          ],
          { fullRefresh: true }
        ),
        "fn main()"
      );
    });

    it("returns tokens for specified line", () => {
      const tokens = cache.tokensForLine(0);
      expect(tokens.length).toBe(4);
    });

    it("returns empty array for line with no tokens", () => {
      expect(cache.tokensForLine(1)).toEqual([]);
      expect(cache.tokensForLine(999)).toEqual([]);
    });

    it("returns tokens sorted by startCol", () => {
      const tokens = cache.tokensForLine(0);

      for (let i = 0; i < tokens.length - 1; i++) {
        expect(tokens[i].startCol).toBeLessThanOrEqual(tokens[i + 1].startCol);
      }
    });
  });

  describe("tokenAt", () => {
    beforeEach(() => {
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 2, category: "keyword" }, // "fn"
            { startByte: 3, endByte: 7, category: "function" }, // "main"
          ],
          { fullRefresh: true }
        ),
        "fn main()"
      );
    });

    it("returns token at position", () => {
      const token = cache.tokenAt(0, 0);
      expect(token).not.toBeNull();
      expect(token!.category).toBe("keyword");
    });

    it("returns token when position is within range", () => {
      // Position 1 is within "fn" (0-2)
      const token = cache.tokenAt(0, 1);
      expect(token).not.toBeNull();
      expect(token!.category).toBe("keyword");
    });

    it("returns null for position between tokens", () => {
      // Position 2 is after "fn" but before "main"
      const token = cache.tokenAt(0, 2);
      expect(token).toBeNull();
    });

    it("returns null for position after all tokens", () => {
      const token = cache.tokenAt(0, 8);
      expect(token).toBeNull();
    });

    it("returns null for empty line", () => {
      expect(cache.tokenAt(1, 0)).toBeNull();
    });

    it("finds second token", () => {
      const token = cache.tokenAt(0, 4);
      expect(token).not.toBeNull();
      expect(token!.category).toBe("function");
    });
  });

  describe("clear", () => {
    it("removes all tokens", () => {
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "test" }], {
          fullRefresh: true,
        }),
        "ab"
      );
      expect(cache.size).toBe(1);

      cache.clear();
      expect(cache.size).toBe(0);
    });

    it("resets line offsets", () => {
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "test" }], {
          fullRefresh: true,
        }),
        "ab"
      );

      cache.clear();

      // After clear, new content should work correctly
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 1, category: "new" }], {
          fullRefresh: true,
        }),
        "x"
      );

      expect(cache.size).toBe(1);
    });
  });

  describe("content length caching", () => {
    it("rebuilds line offsets when content changes", () => {
      // First content
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 1, category: "a" }], {
          fullRefresh: true,
        }),
        "a"
      );

      // Different content length triggers rebuild
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 1, category: "a" },
            { startByte: 2, endByte: 3, category: "b" },
          ],
          { fullRefresh: true }
        ),
        "a\nb"
      );

      expect(cache.tokensForLine(0).length).toBe(1);
      expect(cache.tokensForLine(1).length).toBe(1);
    });

    it("skips rebuild for same length content", () => {
      // First update
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "aa" }], {
          fullRefresh: true,
        }),
        "aa"
      );

      // Same length content - line offsets cached
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 2, category: "bb" }], {
          fullRefresh: true,
        }),
        "bb"
      );

      expect(cache.size).toBe(1);
      expect(cache.tokensForLine(0)[0].category).toBe("bb");
    });
  });
});

// ============ TokenCacheManager Tests ============

describe("TokenCacheManager", () => {
  let manager: TokenCacheManager;

  beforeEach(() => {
    manager = new TokenCacheManager(3); // Small limit for testing
  });

  describe("getOrCreate", () => {
    it("creates new cache for buffer", () => {
      const cache = manager.getOrCreate(1n);
      expect(cache).toBeInstanceOf(TokenCache);
    });

    it("returns same cache for same buffer id", () => {
      const cache1 = manager.getOrCreate(1n);
      const cache2 = manager.getOrCreate(1n);
      expect(cache1).toBe(cache2);
    });

    it("creates different caches for different buffer ids", () => {
      const cache1 = manager.getOrCreate(1n);
      const cache2 = manager.getOrCreate(2n);
      expect(cache1).not.toBe(cache2);
    });

    it("updates size correctly", () => {
      expect(manager.size).toBe(0);

      manager.getOrCreate(1n);
      expect(manager.size).toBe(1);

      manager.getOrCreate(2n);
      expect(manager.size).toBe(2);

      // Same buffer doesn't increase size
      manager.getOrCreate(1n);
      expect(manager.size).toBe(2);
    });
  });

  describe("get", () => {
    it("returns null for uncached buffer", () => {
      expect(manager.get(999n)).toBeNull();
    });

    it("returns cache for cached buffer", () => {
      const created = manager.getOrCreate(1n);
      const retrieved = manager.get(1n);
      expect(retrieved).toBe(created);
    });
  });

  describe("remove", () => {
    it("removes buffer from cache", () => {
      manager.getOrCreate(1n);
      expect(manager.size).toBe(1);

      manager.remove(1n);
      expect(manager.size).toBe(0);
      expect(manager.get(1n)).toBeNull();
    });

    it("does nothing for non-existent buffer", () => {
      manager.getOrCreate(1n);
      manager.remove(999n);
      expect(manager.size).toBe(1);
    });
  });

  describe("clear", () => {
    it("removes all caches", () => {
      manager.getOrCreate(1n);
      manager.getOrCreate(2n);
      manager.getOrCreate(3n);
      expect(manager.size).toBe(3);

      manager.clear();
      expect(manager.size).toBe(0);
    });
  });

  describe("LRU eviction", () => {
    it("evicts oldest when at capacity", () => {
      manager.getOrCreate(1n);
      manager.getOrCreate(2n);
      manager.getOrCreate(3n);
      expect(manager.size).toBe(3);

      // Adding 4th should evict 1
      manager.getOrCreate(4n);
      expect(manager.size).toBe(3);
      expect(manager.get(1n)).toBeNull(); // Evicted
      expect(manager.get(4n)).not.toBeNull();
    });

    it("access updates order (MRU)", () => {
      manager.getOrCreate(1n);
      manager.getOrCreate(2n);
      manager.getOrCreate(3n);

      // Access 1, making it most recently used
      manager.getOrCreate(1n);

      // Add 4 - should evict 2 (oldest)
      manager.getOrCreate(4n);

      expect(manager.get(1n)).not.toBeNull(); // Still present
      expect(manager.get(2n)).toBeNull(); // Evicted
      expect(manager.get(3n)).not.toBeNull();
      expect(manager.get(4n)).not.toBeNull();
    });

    it("handles repeated access patterns", () => {
      // Fill cache
      manager.getOrCreate(1n);
      manager.getOrCreate(2n);
      manager.getOrCreate(3n);

      // Access pattern: 1, 2, 1, 2, 1, 2
      for (let i = 0; i < 3; i++) {
        manager.getOrCreate(1n);
        manager.getOrCreate(2n);
      }

      // Add 4 - should evict 3 (least recently used)
      manager.getOrCreate(4n);

      expect(manager.get(3n)).toBeNull();
      expect(manager.get(1n)).not.toBeNull();
      expect(manager.get(2n)).not.toBeNull();
    });
  });

  describe("default capacity", () => {
    it("uses default capacity of 10", () => {
      const defaultManager = new TokenCacheManager();

      // Add 10 buffers
      for (let i = 1; i <= 10; i++) {
        defaultManager.getOrCreate(BigInt(i));
      }
      expect(defaultManager.size).toBe(10);

      // 11th should evict first
      defaultManager.getOrCreate(11n);
      expect(defaultManager.size).toBe(10);
      expect(defaultManager.get(1n)).toBeNull();
    });
  });
});

// ============ Edge Cases ============

describe("Edge Cases", () => {
  let cache: TokenCache;

  beforeEach(() => {
    cache = new TokenCache();
  });

  describe("empty content", () => {
    it("handles empty string content", () => {
      cache.applyUpdate(createUpdate([], { fullRefresh: true }), "");
      expect(cache.size).toBe(0);
    });

    it("handles content with only newlines", () => {
      cache.applyUpdate(createUpdate([], { fullRefresh: true }), "\n\n\n");
      expect(cache.tokensForLine(0)).toEqual([]);
      expect(cache.tokensForLine(1)).toEqual([]);
    });
  });

  describe("boundary conditions", () => {
    it("handles token at end of line", () => {
      cache.applyUpdate(
        createUpdate([{ startByte: 3, endByte: 5, category: "end" }], {
          fullRefresh: true,
        }),
        "abc12"
      );

      const token = cache.tokensForLine(0)[0];
      expect(token.startCol).toBe(3);
      expect(token.endCol).toBe(5);
    });

    it("handles adjacent tokens", () => {
      cache.applyUpdate(
        createUpdate(
          [
            { startByte: 0, endByte: 2, category: "a" },
            { startByte: 2, endByte: 4, category: "b" },
          ],
          { fullRefresh: true }
        ),
        "aabb"
      );

      const tokens = cache.tokensForLine(0);
      expect(tokens[0].endCol).toBe(tokens[1].startCol);
    });

    it("handles single-character tokens", () => {
      cache.applyUpdate(
        createUpdate([{ startByte: 0, endByte: 1, category: "char" }], {
          fullRefresh: true,
        }),
        "x"
      );

      const token = cache.tokensForLine(0)[0];
      expect(token.startCol).toBe(0);
      expect(token.endCol).toBe(1);
    });
  });

  describe("large content", () => {
    it("handles many tokens", () => {
      const tokens: TokenSpan[] = [];
      for (let i = 0; i < 100; i++) {
        tokens.push({
          startByte: i * 2,
          endByte: i * 2 + 1,
          category: `token${i}`,
        });
      }

      const content = "x ".repeat(100);
      cache.applyUpdate(createUpdate(tokens, { fullRefresh: true }), content);

      expect(cache.size).toBe(100);
    });

    it("handles many lines", () => {
      const lines = Array.from({ length: 100 }, () => "line").join("\n");
      const tokens: TokenSpan[] = [];

      // One token per line
      let offset = 0;
      for (let i = 0; i < 100; i++) {
        tokens.push({
          startByte: offset,
          endByte: offset + 4,
          category: "word",
        });
        offset += 5; // "line\n"
      }

      cache.applyUpdate(createUpdate(tokens, { fullRefresh: true }), lines);

      for (let i = 0; i < 100; i++) {
        expect(cache.tokensForLine(i).length).toBe(1);
      }
    });
  });
});
