/**
 * Chrome Compositor
 *
 * Allocates non-overlapping screen regions for chrome modules
 * (statusline, sidebars, etc.) using a priority-sorted, edge-inward
 * algorithm. Mirrors the TUI `render_chrome` in `render_engine.rs`.
 *
 * @module core/compositor
 */

import type { ClientModule } from "./contracts.js";
import type { ChromePosition, ColumnWidth, Rect } from "./types.js";
import { rect } from "./types.js";

/** A computed chrome region assigned to a module. */
export interface ChromeRegion {
  readonly moduleId: string;
  readonly position: ChromePosition;
  readonly rect: Rect;
  readonly zOrder: number;
}

/** Result of layout computation: chrome regions + remaining viewport. */
export interface CompositorLayout {
  readonly regions: readonly ChromeRegion[];
  readonly viewport: Rect;
}

/** Resolve a ColumnWidth | number to a numeric pixel/cell value. */
function resolveSize(size: ColumnWidth | number): number {
  if (typeof size === "number") return size;
  return size.width;
}

/**
 * Chrome compositor — collects chrome declarations from CLM modules
 * and computes non-overlapping Rect regions.
 *
 * Algorithm (matching TUI `render_chrome`):
 * - Collect modules where `hasChrome() === true`
 * - Sort by `chromePriority()` descending (highest priority first)
 * - Four independent edge counters: top, bottom, left, right
 *   - Top/Bottom use full screenWidth
 *   - Left/Right use full screenHeight
 * - Overlay modules get full-screen bounds (don't consume space)
 * - Viewport = remaining space after all four edges are subtracted
 */
export class ChromeCompositor {
  computeLayout(
    modules: readonly ClientModule[],
    screenWidth: number,
    screenHeight: number,
  ): CompositorLayout {
    // Collect chrome modules
    const chromeModules = modules.filter((m) => m.hasChrome());

    // Sort by priority descending (highest allocated first)
    const sorted = [...chromeModules].sort((a, b) => {
      const pa = a.chromePriority?.() ?? 0;
      const pb = b.chromePriority?.() ?? 0;
      return pb - pa;
    });

    let allocatedTop = 0;
    let allocatedBottom = 0;
    let allocatedLeft = 0;
    let allocatedRight = 0;

    const regions: ChromeRegion[] = [];
    let edgeIndex = 0;

    for (const mod of sorted) {
      const position = mod.chromePosition?.() ?? "overlay";
      const rawSize = mod.chromeRequestedSize?.() ?? 0;
      const size = resolveSize(rawSize);
      const zOrder = mod.chromeZOrder?.() ?? 0;

      let region: Rect;

      switch (position) {
        case "top": {
          const y = allocatedTop;
          allocatedTop += size;
          region = rect(0, y, screenWidth, size);
          break;
        }
        case "bottom": {
          const y = screenHeight - allocatedBottom - size;
          allocatedBottom += size;
          region = rect(0, Math.max(y, 0), screenWidth, size);
          break;
        }
        case "left": {
          const x = allocatedLeft;
          allocatedLeft += size;
          region = rect(x, 0, size, screenHeight);
          break;
        }
        case "right": {
          const x = screenWidth - allocatedRight - size;
          allocatedRight += size;
          region = rect(Math.max(x, 0), 0, size, screenHeight);
          break;
        }
        case "overlay": {
          region = rect(0, 0, screenWidth, screenHeight);
          break;
        }
      }

      regions.push({
        moduleId: mod.id(),
        position,
        rect: region,
        zOrder: position === "overlay" ? 1000 + zOrder : edgeIndex,
      });

      if (position !== "overlay") {
        edgeIndex++;
      }
    }

    const viewport = rect(
      allocatedLeft,
      allocatedTop,
      Math.max(0, screenWidth - allocatedLeft - allocatedRight),
      Math.max(0, screenHeight - allocatedTop - allocatedBottom),
    );

    return { regions, viewport };
  }
}
