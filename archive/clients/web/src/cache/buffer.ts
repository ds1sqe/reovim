// Buffer content cache with version tracking.
//
// This module caches buffer content to avoid re-fetching unchanged
// buffers. Uses version tracking to detect stale cache entries.

/**
 * Cached buffer entry with content and version.
 */
interface BufferEntry {
  /** The buffer content as an array of lines. */
  lines: string[];
  /** Version number from server (for staleness detection). */
  version: number;
  /** Timestamp of last update (for debugging/metrics). */
  lastUpdated: number;
}

/**
 * Caches buffer content with version tracking.
 *
 * This cache reduces RPC calls by storing buffer content locally and
 * only fetching when the server indicates content has changed (via
 * bufferModified notification with new version).
 *
 * @example
 * ```typescript
 * const cache = new BufferCache();
 *
 * // Cache buffer content from server
 * cache.set(100, ["line 1", "line 2", "line 3"], 1);
 *
 * // Check if we need to refresh
 * if (cache.needsRefresh(100, 2)) {
 *   // Fetch from server, version 2 is newer than cached version 1
 * }
 *
 * // Get visible portion for a viewport
 * const visible = cache.getVisibleLines(100, 0, 2); // ["line 1", "line 2"]
 * ```
 */
export class BufferCache {
  private buffers: Map<number, BufferEntry> = new Map();

  /**
   * Get buffer content by ID.
   *
   * @param bufferId - The buffer identifier
   * @returns The cached lines array, or undefined if not cached
   */
  get(bufferId: number): string[] | undefined {
    return this.buffers.get(bufferId)?.lines;
  }

  /**
   * Get full buffer entry including metadata.
   *
   * @param bufferId - The buffer identifier
   * @returns The cached buffer entry, or undefined if not cached
   */
  getEntry(bufferId: number): Readonly<BufferEntry> | undefined {
    return this.buffers.get(bufferId);
  }

  /**
   * Set buffer content with version.
   *
   * @param bufferId - The buffer identifier
   * @param lines - The buffer content as array of lines
   * @param version - The version number from server
   */
  set(bufferId: number, lines: string[], version: number): void {
    this.buffers.set(bufferId, {
      lines: [...lines], // Defensive copy
      version,
      lastUpdated: Date.now(),
    });
  }

  /**
   * Check if we need to refresh from server.
   *
   * Returns true if:
   * - Buffer is not in cache
   * - Server version is newer than cached version
   *
   * @param bufferId - The buffer identifier
   * @param serverVersion - The version number from server notification
   * @returns true if cache is stale or missing
   */
  needsRefresh(bufferId: number, serverVersion: number): boolean {
    const cached = this.buffers.get(bufferId);
    return !cached || cached.version < serverVersion;
  }

  /**
   * Get the cached version number for a buffer.
   *
   * @param bufferId - The buffer identifier
   * @returns The cached version, or -1 if not cached
   */
  getVersion(bufferId: number): number {
    return this.buffers.get(bufferId)?.version ?? -1;
  }

  /**
   * Invalidate a buffer (mark for refresh).
   *
   * Call this when receiving a bufferModified notification.
   * The next render will trigger a fresh fetch.
   *
   * @param bufferId - The buffer identifier
   * @returns true if buffer was invalidated, false if not found
   */
  invalidate(bufferId: number): boolean {
    return this.buffers.delete(bufferId);
  }

  /**
   * Invalidate all buffers.
   *
   * Use sparingly - prefer targeted invalidation.
   */
  invalidateAll(): void {
    this.buffers.clear();
  }

  /**
   * Check if a buffer exists in the cache.
   *
   * @param bufferId - The buffer identifier
   * @returns true if buffer is cached
   */
  has(bufferId: number): boolean {
    return this.buffers.has(bufferId);
  }

  /**
   * Get visible lines for a viewport.
   *
   * Slices the buffer content to return only the lines visible
   * in the viewport, based on scroll position.
   *
   * @param bufferId - The buffer identifier
   * @param topLine - First visible line (0-indexed)
   * @param viewportHeight - Number of visible lines
   * @returns Array of visible lines, or empty array if buffer not cached
   */
  getVisibleLines(
    bufferId: number,
    topLine: number,
    viewportHeight: number
  ): string[] {
    const lines = this.get(bufferId);
    if (!lines) return [];

    // Clamp to valid range
    const start = Math.max(0, topLine);
    const end = Math.min(lines.length, start + viewportHeight);

    return lines.slice(start, end);
  }

  /**
   * Get line count for a buffer.
   *
   * @param bufferId - The buffer identifier
   * @returns Number of lines, or 0 if not cached
   */
  getLineCount(bufferId: number): number {
    return this.get(bufferId)?.length ?? 0;
  }

  /**
   * Get a specific line from a buffer.
   *
   * @param bufferId - The buffer identifier
   * @param lineNumber - Line number (0-indexed)
   * @returns The line content, or undefined if not found
   */
  getLine(bufferId: number, lineNumber: number): string | undefined {
    const lines = this.get(bufferId);
    if (!lines || lineNumber < 0 || lineNumber >= lines.length) {
      return undefined;
    }
    return lines[lineNumber];
  }

  /**
   * Get all cached buffer IDs.
   *
   * @returns Array of buffer IDs currently in cache
   */
  keys(): number[] {
    return Array.from(this.buffers.keys());
  }

  /**
   * Get number of cached buffers.
   *
   * @returns Count of cached buffers
   */
  get size(): number {
    return this.buffers.size;
  }

  /**
   * Clear all cached buffers.
   *
   * Use sparingly - prefer targeted invalidation.
   */
  clear(): void {
    this.buffers.clear();
  }

  /**
   * Get cache statistics for debugging/metrics.
   *
   * @returns Object with cache statistics
   */
  getStats(): {
    bufferCount: number;
    totalLines: number;
    oldestUpdate: number | null;
    newestUpdate: number | null;
  } {
    let totalLines = 0;
    let oldestUpdate: number | null = null;
    let newestUpdate: number | null = null;

    for (const entry of this.buffers.values()) {
      totalLines += entry.lines.length;
      if (oldestUpdate === null || entry.lastUpdated < oldestUpdate) {
        oldestUpdate = entry.lastUpdated;
      }
      if (newestUpdate === null || entry.lastUpdated > newestUpdate) {
        newestUpdate = entry.lastUpdated;
      }
    }

    return {
      bufferCount: this.buffers.size,
      totalLines,
      oldestUpdate,
      newestUpdate,
    };
  }
}
