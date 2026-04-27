/**
 * Chrome Dispatcher Tests (#651, Phase 4)
 *
 * Verifies container lifecycle, reuse, destroy, module updates,
 * and viewport rect computation.
 *
 * @vitest-environment jsdom
 */

import { describe, it, expect, beforeEach, vi, type Mock } from "vitest";
import { ChromeDispatcher } from "../src/core/chrome-dispatcher.js";
import type { ClientModule, RenderSurface } from "../src/core/contracts.js";
import type { ChromePosition, Rect } from "../src/core/types.js";
import { probeSuccess } from "../src/core/types.js";
import { WebExtensionAdapter } from "../src/core/extension-adapter.js";
import type { WebExtension } from "../src/extensions/interface.js";

// ---- Helpers ----

function mockChromeModule(opts: {
  id: string;
  position: ChromePosition;
  size: number;
  priority?: number;
  active?: boolean;
}): ClientModule {
  return {
    id: () => opts.id,
    kind: () => opts.id,
    name: () => opts.id,
    version: () => ({ major: 0, minor: 1, patch: 0 }),
    init: () => probeSuccess(),
    exit: () => {},
    hasChrome: () => true,
    hasBufferContrib: () => false,
    hasAnnotations: () => false,
    chromePosition: () => opts.position,
    chromeRequestedSize: () => opts.size,
    chromePriority: () => opts.priority ?? 0,
    chromeZOrder: () => 0,
  };
}

interface MockWebExtension extends WebExtension {
  render: Mock;
  hide: Mock;
  applyNotification: Mock;
}

function mockWebExtension(opts: {
  kind: string;
  active: boolean;
}): MockWebExtension {
  return {
    kind: () => opts.kind,
    isActive: () => opts.active,
    applyNotification: vi.fn(),
    render: vi.fn(),
    hide: vi.fn(),
    getState: () => null,
  };
}

describe("ChromeDispatcher", () => {
  let parent: HTMLElement;

  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("creates positioned containers for chrome modules", () => {
    const dispatcher = new ChromeDispatcher(parent);
    const modules = [
      mockChromeModule({ id: "statusline", position: "bottom", size: 1 }),
    ];

    dispatcher.dispatch(modules, 80, 24);

    const container = parent.querySelector(
      '[data-chrome-module="statusline"]',
    ) as HTMLElement;
    expect(container).not.toBeNull();
    expect(container.style.position).toBe("absolute");
    expect(container.style.top).toBe("23px");
    expect(container.style.width).toBe("80px");
    expect(container.style.height).toBe("1px");
  });

  it("reuses containers across frames", () => {
    const dispatcher = new ChromeDispatcher(parent);
    const modules = [
      mockChromeModule({ id: "statusline", position: "bottom", size: 1 }),
    ];

    dispatcher.dispatch(modules, 80, 24);
    const containerBefore = parent.querySelector(
      '[data-chrome-module="statusline"]',
    );

    dispatcher.dispatch(modules, 80, 24);
    const containerAfter = parent.querySelector(
      '[data-chrome-module="statusline"]',
    );

    // Same DOM element reused
    expect(containerBefore).toBe(containerAfter);
    // Only one container created
    expect(
      parent.querySelectorAll('[data-chrome-module="statusline"]'),
    ).toHaveLength(1);
  });

  it("removes containers on destroy", () => {
    const dispatcher = new ChromeDispatcher(parent);
    const modules = [
      mockChromeModule({ id: "statusline", position: "bottom", size: 1 }),
      mockChromeModule({ id: "tabline", position: "top", size: 1 }),
    ];

    dispatcher.dispatch(modules, 80, 24);
    expect(parent.querySelectorAll("[data-chrome-module]")).toHaveLength(2);

    dispatcher.destroy();
    expect(parent.querySelectorAll("[data-chrome-module]")).toHaveLength(0);
  });

  it("handleModuleUpdate only re-renders the targeted module", () => {
    const dispatcher = new ChromeDispatcher(parent);

    const ext1 = mockWebExtension({ kind: "whichkey", active: true });
    const ext2 = mockWebExtension({ kind: "notification", active: true });
    const mod1 = new WebExtensionAdapter(ext1);
    const mod2 = new WebExtensionAdapter(ext2);
    const modules = [mod1, mod2];

    // Initial dispatch
    dispatcher.dispatch(modules, 80, 24);
    ext1.render.mockClear();
    ext2.render.mockClear();

    // Update only whichkey
    dispatcher.handleModuleUpdate("whichkey", modules, 80, 24);

    expect(ext1.render).toHaveBeenCalled();
    expect(ext2.render).not.toHaveBeenCalled();
  });

  it("returns correct viewport rect after allocation", () => {
    const dispatcher = new ChromeDispatcher(parent);
    const modules = [
      mockChromeModule({
        id: "statusline",
        position: "bottom",
        size: 1,
        priority: 100,
      }),
      mockChromeModule({
        id: "tabline",
        position: "top",
        size: 1,
        priority: 100,
      }),
      mockChromeModule({
        id: "explorer",
        position: "left",
        size: 30,
        priority: 50,
      }),
    ];

    const viewport = dispatcher.dispatch(modules, 120, 40);

    expect(viewport).toEqual({ x: 30, y: 1, width: 90, height: 38 });
  });

  it("hides containers for inactive WebExtensionAdapter modules", () => {
    const dispatcher = new ChromeDispatcher(parent);

    const ext = mockWebExtension({ kind: "whichkey", active: false });
    const mod = new WebExtensionAdapter(ext);

    dispatcher.dispatch([mod], 80, 24);

    const container = parent.querySelector(
      '[data-chrome-module="whichkey"]',
    ) as HTMLElement;
    expect(container.style.display).toBe("none");
    expect(ext.hide).toHaveBeenCalled();
  });

  it("renders active WebExtensionAdapter modules into containers", () => {
    const dispatcher = new ChromeDispatcher(parent);

    const ext = mockWebExtension({ kind: "whichkey", active: true });
    const mod = new WebExtensionAdapter(ext);

    dispatcher.dispatch([mod], 80, 24);

    const container = parent.querySelector(
      '[data-chrome-module="whichkey"]',
    ) as HTMLElement;
    expect(container.style.display).not.toBe("none");
    expect(ext.render).toHaveBeenCalledWith(container);
  });

  it("calls chromeRender for native CLM modules", () => {
    const dispatcher = new ChromeDispatcher(parent);

    const renderFn = vi.fn((_surface: RenderSurface, _rect: Rect) => {});
    const mod: ClientModule = {
      ...mockChromeModule({ id: "native-status", position: "bottom", size: 1 }),
      chromeRender: renderFn,
    };

    dispatcher.dispatch([mod], 80, 24);

    expect(renderFn).toHaveBeenCalled();
    // Surface and rect should be passed
    const call = renderFn.mock.calls[0];
    expect(call).toBeDefined();
    const [surface, rect] = call!;
    expect(surface.size()).toEqual({ width: 80, height: 1 });
    expect(rect).toEqual({ x: 0, y: 23, width: 80, height: 1 });
  });
});
