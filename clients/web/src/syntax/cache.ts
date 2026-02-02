/**
 * Token cache for syntax highlighting.
 *
 * Matches TUI TokenCache design:
 * - Stores tokens with (line, startCol, endCol, category)
 * - Byte-to-position conversion
 * - Incremental updates via applyUpdate()
 *
 * @module syntax/cache
 */

/**
 * A cached token with position info.
 *
 * Converted from byte-based TokenSpan from the protocol.
 */
export interface CachedToken {
  /** Line number (0-indexed) */
  line: number;
  /** Start column (0-indexed) */
  startCol: number;
  /** End column (0-indexed, exclusive) */
  endCol: number;
  /** Token category (e.g., "keyword", "function.builtin") */
  category: string;
}

/**
 * TokenSpan from the protocol (byte-based).
 */
export interface TokenSpan {
  /** Start byte offset */
  startByte: number;
  /** End byte offset */
  endByte: number;
  /** Token category */
  category: string;
}

/**
 * Token update from StreamTokens RPC.
 */
export interface TokenUpdate {
  /** Buffer ID */
  bufferId: bigint;
  /** Token spans */
  tokens: TokenSpan[];
  /** First affected line (0-indexed) */
  startLine: number;
  /** Last affected line (0-indexed) */
  endLine: number;
  /** Whether this is a full refresh (clear all tokens first) */
  fullRefresh: boolean;
}

/**
 * Cache for syntax tokens per buffer.
 *
 * Converts byte-based tokens from the protocol to position-based
 * tokens for rendering.
 *
 * @example
 * ```typescript
 * const cache = new TokenCache();
 *
 * // Apply update from StreamTokens
 * cache.applyUpdate(update, bufferContent);
 *
 * // Get tokens for rendering a line
 * for (const token of cache.tokensForLine(10)) {
 *   // Apply highlighting
 * }
 * ```
 */
export class TokenCache {
  /** Tokens sorted by (line, startCol) */
  private tokens: CachedToken[] = [];

  /** Line start byte offsets for byte→position conversion */
  private lineOffsets: number[] = [];

  /** Current buffer content length (for validation) */
  private contentLen: number = 0;

  /**
   * Apply a token update from StreamTokens.
   *
   * @param update - Token update message
   * @param content - Current buffer content (for byte→position conversion)
   */
  applyUpdate(update: TokenUpdate, content: string): void {
    // Rebuild line offsets if content changed
    this.rebuildLineOffsets(content);

    if (update.fullRefresh) {
      // Full refresh - clear all tokens
      this.tokens = [];
    } else {
      // Incremental - remove tokens in affected range
      this.tokens = this.tokens.filter(
        (t) => t.line < update.startLine || t.line > update.endLine
      );
    }

    // Convert and add new tokens
    for (const span of update.tokens) {
      const token = this.byteSpanToCached(span);
      if (token) {
        this.tokens.push(token);
      }
    }

    // Re-sort by (line, startCol)
    this.tokens.sort((a, b) => {
      if (a.line !== b.line) return a.line - b.line;
      return a.startCol - b.startCol;
    });
  }

  /**
   * Get all tokens for a specific line.
   *
   * @param line - Line number (0-indexed)
   * @returns Tokens on this line, sorted by startCol
   */
  tokensForLine(line: number): CachedToken[] {
    return this.tokens.filter((t) => t.line === line);
  }

  /**
   * Get the token at a specific position.
   *
   * @param line - Line number (0-indexed)
   * @param col - Column number (0-indexed)
   * @returns Token at position, or null if none
   */
  tokenAt(line: number, col: number): CachedToken | null {
    return (
      this.tokens.find(
        (t) => t.line === line && col >= t.startCol && col < t.endCol
      ) ?? null
    );
  }

  /**
   * Clear all cached tokens.
   */
  clear(): void {
    this.tokens = [];
    this.lineOffsets = [];
    this.contentLen = 0;
  }

  /**
   * Get the total number of cached tokens.
   */
  get size(): number {
    return this.tokens.length;
  }

  /**
   * Rebuild line offset table from buffer content.
   */
  private rebuildLineOffsets(content: string): void {
    // Skip if content unchanged
    if (content.length === this.contentLen && this.lineOffsets.length > 0) {
      return;
    }

    this.contentLen = content.length;
    this.lineOffsets = [0]; // First line starts at byte 0

    for (let i = 0; i < content.length; i++) {
      if (content[i] === '\n') {
        this.lineOffsets.push(i + 1);
      }
    }
  }

  /**
   * Convert a byte offset to (line, col) position.
   *
   * Uses binary search on lineOffsets for efficiency.
   */
  private byteToPosition(byte: number): { line: number; col: number } {
    if (this.lineOffsets.length === 0) {
      return { line: 0, col: byte };
    }

    // Binary search to find the line
    let low = 0;
    let high = this.lineOffsets.length - 1;

    while (low < high) {
      const mid = Math.floor((low + high + 1) / 2);
      if (this.lineOffsets[mid]! <= byte) {
        low = mid;
      } else {
        high = mid - 1;
      }
    }

    const line = low;
    const col = byte - this.lineOffsets[line]!;
    return { line, col };
  }

  /**
   * Convert a byte-based TokenSpan to a CachedToken.
   *
   * Handles multi-line tokens by splitting at line boundaries.
   * Returns null if the token spans multiple lines (we handle
   * only the first line for simplicity).
   */
  private byteSpanToCached(span: TokenSpan): CachedToken | null {
    const start = this.byteToPosition(span.startByte);
    const end = this.byteToPosition(span.endByte);

    // For multi-line tokens, we only keep the first line portion
    // This matches the TUI behavior of line-based rendering
    if (start.line !== end.line) {
      // Find the end of the first line
      const lineEnd =
        start.line + 1 < this.lineOffsets.length
          ? this.lineOffsets[start.line + 1]! - 1 - this.lineOffsets[start.line]!
          : end.col;

      return {
        line: start.line,
        startCol: start.col,
        endCol: lineEnd,
        category: span.category,
      };
    }

    return {
      line: start.line,
      startCol: start.col,
      endCol: end.col,
      category: span.category,
    };
  }
}

/**
 * Manages token caches for multiple buffers.
 *
 * Uses LRU eviction when too many buffers are cached.
 */
export class TokenCacheManager {
  private caches: Map<bigint, TokenCache>;
  private accessOrder: bigint[];
  private maxBuffers: number;

  constructor(maxBuffers: number = 10) {
    this.caches = new Map();
    this.accessOrder = [];
    this.maxBuffers = maxBuffers;
  }

  /**
   * Get or create a cache for a buffer.
   */
  getOrCreate(bufferId: bigint): TokenCache {
    let cache = this.caches.get(bufferId);

    if (!cache) {
      // Evict oldest if at capacity
      if (this.caches.size >= this.maxBuffers) {
        const oldest = this.accessOrder.shift();
        if (oldest !== undefined) {
          this.caches.delete(oldest);
        }
      }

      cache = new TokenCache();
      this.caches.set(bufferId, cache);
    }

    // Update access order (move to end)
    const idx = this.accessOrder.indexOf(bufferId);
    if (idx !== -1) {
      this.accessOrder.splice(idx, 1);
    }
    this.accessOrder.push(bufferId);

    return cache;
  }

  /**
   * Get cache for a buffer, or null if not cached.
   */
  get(bufferId: bigint): TokenCache | null {
    return this.caches.get(bufferId) ?? null;
  }

  /**
   * Remove cache for a buffer.
   */
  remove(bufferId: bigint): void {
    this.caches.delete(bufferId);
    const idx = this.accessOrder.indexOf(bufferId);
    if (idx !== -1) {
      this.accessOrder.splice(idx, 1);
    }
  }

  /**
   * Clear all caches.
   */
  clear(): void {
    this.caches.clear();
    this.accessOrder = [];
  }

  /**
   * Number of cached buffers.
   */
  get size(): number {
    return this.caches.size;
  }
}
