// Viewport state cache for incremental updates.
//
// This module caches viewport state from the server and applies
// incremental updates to avoid full re-fetches on every change.

import type { ViewportState, ViewportUpdate } from "../wasm/index.js";

/**
 * Caches viewport state and applies incremental updates.
 *
 * Uses the wire types from `reovim-client-model` to track viewport state
 * (scroll position, cursor position) and apply partial updates efficiently.
 *
 * @example
 * ```typescript
 * const cache = new ViewportCache();
 *
 * // Initial population from server
 * cache.set({ id: 1, buffer_id: 100, top_line: 0, left_col: 0, cursor_line: 5, cursor_col: 10 });
 *
 * // Apply incremental update (e.g., from viewportUpdated notification)
 * cache.applyUpdate({ viewport_id: 1, cursor_line: 6 });
 * ```
 */
export class ViewportCache {
  private viewports: Map<number, ViewportState> = new Map();

  /**
   * Get viewport state by ID.
   *
   * @param viewportId - The viewport identifier
   * @returns The cached viewport state, or undefined if not cached
   */
  get(viewportId: number): ViewportState | undefined {
    return this.viewports.get(viewportId);
  }

  /**
   * Set viewport state (from full refresh or initial load).
   *
   * @param state - The complete viewport state from the server
   */
  set(state: ViewportState): void {
    // Create a copy to avoid mutation issues
    this.viewports.set(Number(state.id), { ...state });
  }

  /**
   * Apply incremental update to cached state.
   *
   * Only updates fields that are present in the update object.
   * This enables efficient partial updates without full re-fetch.
   *
   * @param update - The partial update from a viewportUpdated notification
   * @returns true if state was updated, false if viewport not found in cache
   */
  applyUpdate(update: ViewportUpdate): boolean {
    const viewportId = Number(update.viewport_id);
    const existing = this.viewports.get(viewportId);
    if (!existing) return false;

    // Apply changes only for defined fields
    if (update.top_line !== undefined && update.top_line !== null) {
      existing.top_line = update.top_line;
    }
    if (update.left_col !== undefined && update.left_col !== null) {
      existing.left_col = update.left_col;
    }
    if (update.cursor_line !== undefined && update.cursor_line !== null) {
      existing.cursor_line = update.cursor_line;
    }
    if (update.cursor_col !== undefined && update.cursor_col !== null) {
      existing.cursor_col = update.cursor_col;
    }

    return true;
  }

  /**
   * Check if a viewport exists in the cache.
   *
   * @param viewportId - The viewport identifier
   * @returns true if viewport is cached
   */
  has(viewportId: number): boolean {
    return this.viewports.has(viewportId);
  }

  /**
   * Remove a viewport from the cache.
   *
   * Call this when a viewport is closed or layout changes.
   *
   * @param viewportId - The viewport identifier
   * @returns true if viewport was removed, false if not found
   */
  delete(viewportId: number): boolean {
    return this.viewports.delete(viewportId);
  }

  /**
   * Clear all cached viewports.
   *
   * Use sparingly - prefer incremental updates.
   */
  clear(): void {
    this.viewports.clear();
  }

  /**
   * Get all cached viewport IDs.
   *
   * @returns Array of viewport IDs currently in cache
   */
  keys(): number[] {
    return Array.from(this.viewports.keys());
  }

  /**
   * Get number of cached viewports.
   *
   * @returns Count of cached viewports
   */
  get size(): number {
    return this.viewports.size;
  }

  /**
   * Get all cached viewport states.
   *
   * @returns Iterator over all cached viewport states
   */
  values(): IterableIterator<ViewportState> {
    return this.viewports.values();
  }

  /**
   * Iterate over all cached viewports.
   *
   * @param callback - Function to call for each viewport
   */
  forEach(callback: (state: ViewportState, id: number) => void): void {
    this.viewports.forEach(callback);
  }
}
