/**
 * Core CLM Contracts Tests (#650)
 *
 * Verifies that mock implementations satisfy the CLM interfaces and
 * that ModuleContext assembly works correctly.
 */

import { describe, it, expect } from "vitest";
import type {
  ClientModule,
  PlatformCapabilities,
  ServerHandle,
  ClmThemeProvider,
  ClientModuleRegistry,
  ModuleContext,
  RenderSurface,
} from "../src/core/contracts.js";
import type { Version, ProbeResult, Rect, Style } from "../src/core/types.js";
import { INSETS_ZERO } from "../src/core/types.js";
import { WebClientServiceRegistry } from "../src/core/service-registry.js";

// ---- Helpers: minimal mocks ----

function mockCapabilities(): PlatformCapabilities {
  return {
    renderingModel: () => "Canvas",
    gridSize: () => null,
    colorDepth: () => "TrueColor",
    pixelSize: () => ({ width: 800, height: 600 }),
    reliableUnicodeWidth: () => true,
    darkMode: () => true,
    smoothScroll: () => true,
    pointerEvents: () => true,
    touchInput: () => false,
    haptic: () => false,
    safeArea: () => INSETS_ZERO,
    hasFocus: () => true,
    clipboardAvailable: () => true,
    screenReaderActive: () => false,
  };
}

function mockServerHandle(): ServerHandle {
  return {
    getOptions: async () => new Map(),
    executeCommand: async () => {},
    listCommands: async () => [],
    getOptionMetadata: async () => null,
  };
}

function mockTheme(): ClmThemeProvider {
  return {
    highlight: () => null,
    highlightWithFallback: () => null,
    foreground: () => ({ fg: "#abb2bf" }),
    background: () => ({ bg: "#282c34" }),
    isDark: () => true,
  };
}

function minimalModule(kind: string): ClientModule {
  return {
    id: () => kind,
    kind: () => kind,
    name: () => kind,
    version: (): Version => ({ major: 1, minor: 0, patch: 0 }),
    init: (): ProbeResult => ({ status: "success" }),
    exit: () => {},
    hasChrome: () => false,
    hasBufferContrib: () => false,
    hasAnnotations: () => false,
  };
}

// ---- Tests ----

describe("PlatformCapabilities mock", () => {
  it("provides all 14 methods", () => {
    const caps = mockCapabilities();
    expect(caps.renderingModel()).toBe("Canvas");
    expect(caps.gridSize()).toBeNull();
    expect(caps.colorDepth()).toBe("TrueColor");
    expect(caps.pixelSize()).toEqual({ width: 800, height: 600 });
    expect(caps.reliableUnicodeWidth()).toBe(true);
    expect(caps.darkMode()).toBe(true);
    expect(caps.smoothScroll()).toBe(true);
    expect(caps.pointerEvents()).toBe(true);
    expect(caps.touchInput()).toBe(false);
    expect(caps.haptic()).toBe(false);
    expect(caps.safeArea()).toEqual(INSETS_ZERO);
    expect(caps.hasFocus()).toBe(true);
    expect(caps.clipboardAvailable()).toBe(true);
    expect(caps.screenReaderActive()).toBe(false);
  });
});

describe("ClientModule minimal mock", () => {
  it("satisfies the interface", () => {
    const mod = minimalModule("test");
    expect(mod.id()).toBe("test");
    expect(mod.kind()).toBe("test");
    expect(mod.name()).toBe("test");
    expect(mod.version()).toEqual({ major: 1, minor: 0, patch: 0 });
    expect(mod.init({} as ModuleContext).status).toBe("success");
    expect(mod.hasChrome()).toBe(false);
    expect(mod.hasBufferContrib()).toBe(false);
    expect(mod.hasAnnotations()).toBe(false);
  });

  it("optional methods are undefined", () => {
    const mod = minimalModule("test");
    expect(mod.onNotification).toBeUndefined();
    expect(mod.onModeChange).toBeUndefined();
    expect(mod.chromePosition).toBeUndefined();
    expect(mod.dependencies).toBeUndefined();
    expect(mod.getState).toBeUndefined();
  });
});

describe("ModuleContext assembly", () => {
  it("composes all required fields", () => {
    const ctx: ModuleContext = {
      capabilities: mockCapabilities(),
      server: mockServerHandle(),
      theme: mockTheme(),
    };

    expect(ctx.capabilities.renderingModel()).toBe("Canvas");
    expect(ctx.theme.isDark()).toBe(true);
    expect(ctx.services).toBeUndefined();
    expect(ctx.moduleRegistry).toBeUndefined();
  });

  it("composes with optional fields", () => {
    const services = new WebClientServiceRegistry();
    const registry: ClientModuleRegistry = {
      isRunning: () => false,
      moduleState: () => null,
      loadedKinds: () => [],
      runningCount: () => 0,
    };

    const ctx: ModuleContext = {
      capabilities: mockCapabilities(),
      server: mockServerHandle(),
      theme: mockTheme(),
      services,
      moduleRegistry: registry,
    };

    expect(ctx.services).toBeDefined();
    expect(ctx.moduleRegistry).toBeDefined();
    expect(ctx.moduleRegistry!.runningCount()).toBe(0);
  });
});

describe("RenderSurface interface shape", () => {
  it("can be mocked with all 6 methods", () => {
    const surface: RenderSurface = {
      writeStyled: (_x: number, _y: number, _text: string, _style: Style) => {},
      applyStyle: (_rect: Rect, _style: Style) => {},
      overlayBg: (_rect: Rect, _color: string) => {},
      fill: (_rect: Rect, _ch: string, _style: Style) => {},
      clear: () => {},
      size: () => ({ width: 80, height: 24 }),
    };

    expect(surface.size()).toEqual({ width: 80, height: 24 });
  });
});

describe("ClientServiceRegistry", () => {
  it("register and get", () => {
    const reg = new WebClientServiceRegistry();
    reg.register("my-service", { value: 42 });
    expect(reg.get<{ value: number }>("my-service")?.value).toBe(42);
  });

  it("contains check", () => {
    const reg = new WebClientServiceRegistry();
    expect(reg.contains("x")).toBe(false);
    reg.register("x", "hello");
    expect(reg.contains("x")).toBe(true);
  });

  it("size tracks registrations", () => {
    const reg = new WebClientServiceRegistry();
    expect(reg.size).toBe(0);
    reg.register("a", 1);
    reg.register("b", 2);
    expect(reg.size).toBe(2);
  });

  it("overwrite on duplicate key", () => {
    const reg = new WebClientServiceRegistry();
    reg.register("key", "first");
    reg.register("key", "second");
    expect(reg.get("key")).toBe("second");
    expect(reg.size).toBe(1);
  });

  it("get returns undefined for missing key", () => {
    const reg = new WebClientServiceRegistry();
    expect(reg.get("missing")).toBeUndefined();
  });
});
