/**
 * Chrome Compositor Tests (#651, Phase 1)
 *
 * Verifies the edge-inward allocation algorithm, priority sorting,
 * overlay handling, and viewport computation.
 */

import { describe, it, expect } from "vitest";
import { ChromeCompositor } from "../src/core/compositor.js";
import type { ClientModule } from "../src/core/contracts.js";
import type { ChromePosition, ColumnWidth } from "../src/core/types.js";
import { probeSuccess } from "../src/core/types.js";

// ---- Helpers ----

/** Create a minimal mock ClientModule for compositor testing. */
function mockChromeModule(opts: {
  id: string;
  position: ChromePosition;
  size: ColumnWidth | number;
  priority?: number;
  zOrder?: number;
  hasChrome?: boolean;
}): ClientModule {
  return {
    id: () => opts.id,
    kind: () => opts.id,
    name: () => opts.id,
    version: () => ({ major: 0, minor: 1, patch: 0 }),
    init: () => probeSuccess(),
    exit: () => {},
    hasChrome: () => opts.hasChrome ?? true,
    hasBufferContrib: () => false,
    hasAnnotations: () => false,
    chromePosition: () => opts.position,
    chromeRequestedSize: () => opts.size,
    chromePriority: () => opts.priority ?? 0,
    chromeZOrder: () => opts.zOrder ?? 0,
  };
}

describe("ChromeCompositor", () => {
  const compositor = new ChromeCompositor();

  it("empty module list returns full viewport", () => {
    const layout = compositor.computeLayout([], 80, 24);
    expect(layout.regions).toHaveLength(0);
    expect(layout.viewport).toEqual({ x: 0, y: 0, width: 80, height: 24 });
  });

  it("single bottom module reduces viewport height", () => {
    const modules = [
      mockChromeModule({ id: "statusline", position: "bottom", size: 1 }),
    ];
    const layout = compositor.computeLayout(modules, 80, 24);

    expect(layout.regions).toHaveLength(1);
    expect(layout.regions[0]!.rect).toEqual({
      x: 0,
      y: 23,
      width: 80,
      height: 1,
    });
    expect(layout.viewport).toEqual({ x: 0, y: 0, width: 80, height: 23 });
  });

  it("single top module reduces viewport from top", () => {
    const modules = [
      mockChromeModule({ id: "tabline", position: "top", size: 2 }),
    ];
    const layout = compositor.computeLayout(modules, 80, 24);

    expect(layout.regions).toHaveLength(1);
    expect(layout.regions[0]!.rect).toEqual({
      x: 0,
      y: 0,
      width: 80,
      height: 2,
    });
    expect(layout.viewport).toEqual({ x: 0, y: 2, width: 80, height: 22 });
  });

  it("left and right panels reduce viewport width with full screen height", () => {
    const modules = [
      mockChromeModule({ id: "explorer", position: "left", size: 30 }),
      mockChromeModule({ id: "outline", position: "right", size: 20 }),
    ];
    const layout = compositor.computeLayout(modules, 120, 40);

    // Left panel: full screen height
    const left = layout.regions.find((r) => r.moduleId === "explorer")!;
    expect(left.rect).toEqual({ x: 0, y: 0, width: 30, height: 40 });

    // Right panel: full screen height
    const right = layout.regions.find((r) => r.moduleId === "outline")!;
    expect(right.rect).toEqual({ x: 100, y: 0, width: 20, height: 40 });

    // Viewport reduced from both sides
    expect(layout.viewport).toEqual({ x: 30, y: 0, width: 70, height: 40 });
  });

  it("priority ordering: higher priority allocated first", () => {
    // Two bottom modules — higher priority gets the bottom row
    const modules = [
      mockChromeModule({
        id: "cmdline",
        position: "bottom",
        size: 1,
        priority: 10,
      }),
      mockChromeModule({
        id: "statusline",
        position: "bottom",
        size: 1,
        priority: 100,
      }),
    ];
    const layout = compositor.computeLayout(modules, 80, 24);

    // statusline (priority 100) allocated first → row 23
    const statusline = layout.regions.find(
      (r) => r.moduleId === "statusline",
    )!;
    expect(statusline.rect.y).toBe(23);

    // cmdline (priority 10) allocated second → row 22
    const cmdline = layout.regions.find((r) => r.moduleId === "cmdline")!;
    expect(cmdline.rect.y).toBe(22);

    expect(layout.viewport.height).toBe(22);
  });

  it("overlay modules get full-screen bounds without reducing viewport", () => {
    const modules = [
      mockChromeModule({ id: "statusline", position: "bottom", size: 1 }),
      mockChromeModule({
        id: "whichkey",
        position: "overlay",
        size: 0,
        zOrder: 5,
      }),
    ];
    const layout = compositor.computeLayout(modules, 80, 24);

    const overlay = layout.regions.find((r) => r.moduleId === "whichkey")!;
    expect(overlay.rect).toEqual({ x: 0, y: 0, width: 80, height: 24 });
    expect(overlay.position).toBe("overlay");
    expect(overlay.zOrder).toBe(1005); // 1000 + 5

    // Viewport only reduced by statusline, not overlay
    expect(layout.viewport.height).toBe(23);
  });

  it("two bottom modules stack correctly", () => {
    const modules = [
      mockChromeModule({
        id: "statusline",
        position: "bottom",
        size: 1,
        priority: 100,
      }),
      mockChromeModule({
        id: "cmdline",
        position: "bottom",
        size: 1,
        priority: 50,
      }),
    ];
    const layout = compositor.computeLayout(modules, 80, 24);

    const statusline = layout.regions.find(
      (r) => r.moduleId === "statusline",
    )!;
    const cmdline = layout.regions.find((r) => r.moduleId === "cmdline")!;

    // statusline at bottom row, cmdline above it
    expect(statusline.rect.y).toBe(23);
    expect(cmdline.rect.y).toBe(22);
    expect(layout.viewport.height).toBe(22);
  });

  it("chromeRequestedSize determines region dimensions", () => {
    const modules = [
      mockChromeModule({
        id: "sidebar",
        position: "left",
        size: { kind: "fixed", width: 40 },
      }),
    ];
    const layout = compositor.computeLayout(modules, 120, 40);

    expect(layout.regions[0]!.rect.width).toBe(40);
    expect(layout.viewport.x).toBe(40);
    expect(layout.viewport.width).toBe(80);
  });

  it("modules with hasChrome false are excluded", () => {
    const modules = [
      mockChromeModule({
        id: "hidden",
        position: "bottom",
        size: 1,
        hasChrome: false,
      }),
      mockChromeModule({ id: "statusline", position: "bottom", size: 1 }),
    ];
    const layout = compositor.computeLayout(modules, 80, 24);

    expect(layout.regions).toHaveLength(1);
    expect(layout.regions[0]!.moduleId).toBe("statusline");
  });

  it("zero-size screen returns zero-area viewport", () => {
    const modules = [
      mockChromeModule({ id: "statusline", position: "bottom", size: 1 }),
    ];
    const layout = compositor.computeLayout(modules, 0, 0);

    expect(layout.viewport.width).toBe(0);
    expect(layout.viewport.height).toBe(0);
  });

  it("all four edges with independent counters", () => {
    const modules = [
      mockChromeModule({
        id: "top",
        position: "top",
        size: 2,
        priority: 100,
      }),
      mockChromeModule({
        id: "bottom",
        position: "bottom",
        size: 1,
        priority: 100,
      }),
      mockChromeModule({
        id: "left",
        position: "left",
        size: 10,
        priority: 50,
      }),
      mockChromeModule({
        id: "right",
        position: "right",
        size: 10,
        priority: 50,
      }),
    ];
    const layout = compositor.computeLayout(modules, 100, 40);

    // Top: full width at y=0, height=2
    const top = layout.regions.find((r) => r.moduleId === "top")!;
    expect(top.rect).toEqual({ x: 0, y: 0, width: 100, height: 2 });

    // Bottom: full width at y=39, height=1
    const bottom = layout.regions.find((r) => r.moduleId === "bottom")!;
    expect(bottom.rect).toEqual({ x: 0, y: 39, width: 100, height: 1 });

    // Left: full HEIGHT (40), not remaining height
    const left = layout.regions.find((r) => r.moduleId === "left")!;
    expect(left.rect).toEqual({ x: 0, y: 0, width: 10, height: 40 });

    // Right: full HEIGHT (40), not remaining height
    const right = layout.regions.find((r) => r.moduleId === "right")!;
    expect(right.rect).toEqual({ x: 90, y: 0, width: 10, height: 40 });

    // Viewport: reduced by all four edges
    expect(layout.viewport).toEqual({ x: 10, y: 2, width: 80, height: 37 });
  });

  it("edge chrome z-order follows allocation order", () => {
    const modules = [
      mockChromeModule({
        id: "statusline",
        position: "bottom",
        size: 1,
        priority: 100,
      }),
      mockChromeModule({
        id: "cmdline",
        position: "bottom",
        size: 1,
        priority: 50,
      }),
    ];
    const layout = compositor.computeLayout(modules, 80, 24);

    const statusline = layout.regions.find(
      (r) => r.moduleId === "statusline",
    )!;
    const cmdline = layout.regions.find((r) => r.moduleId === "cmdline")!;

    // statusline allocated first (higher priority) → lower z-order
    expect(statusline.zOrder).toBeLessThan(cmdline.zOrder);
  });
});
