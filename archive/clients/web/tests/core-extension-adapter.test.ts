/**
 * WebExtension Adapter Tests (#650)
 *
 * Tests that the adapter correctly wraps WebExtension into ClientModule.
 */

import { describe, it, expect, vi } from "vitest";
import {
  WebExtensionAdapter,
  adaptExtension,
  adaptExtensions,
} from "../src/core/extension-adapter.js";
import type { WebExtension } from "../src/extensions/interface.js";
import type { ModuleContext } from "../src/core/contracts.js";
import { INSETS_ZERO } from "../src/core/types.js";

// ---- Helper: create a mock WebExtension ----

function mockExtension(kind: string): WebExtension {
  return {
    kind: () => kind,
    isActive: () => false,
    applyNotification: vi.fn(),
    render: vi.fn(),
    hide: vi.fn(),
    getState: () => ({ kind, active: false }),
    dependencies: () => ["dep-a"],
    init: vi.fn(),
    exit: vi.fn(),
    serverKinds: () => ["server-kind-1"],
  };
}

function stubContext(): ModuleContext {
  return {
    capabilities: {
      renderingModel: () => "Canvas",
      gridSize: () => null,
      colorDepth: () => "TrueColor",
      pixelSize: () => null,
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
    },
    server: {
      getOptions: async () => new Map(),
      executeCommand: async () => {},
      listCommands: async () => [],
      getOptionMetadata: async () => null,
    },
    theme: {
      highlight: () => null,
      highlightWithFallback: () => null,
      foreground: () => ({}),
      background: () => ({}),
      isDark: () => true,
    },
  };
}

// ---- Tests ----

describe("WebExtensionAdapter", () => {
  describe("identity", () => {
    it("id returns ext.kind()", () => {
      const ext = mockExtension("cmdline");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.id()).toBe("cmdline");
    });

    it("kind returns ext.kind()", () => {
      const ext = mockExtension("whichkey");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.kind()).toBe("whichkey");
    });

    it("name returns ext.kind()", () => {
      const ext = mockExtension("explorer");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.name()).toBe("explorer");
    });

    it("version returns 0.1.0", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.version()).toEqual({ major: 0, minor: 1, patch: 0 });
    });
  });

  describe("lifecycle", () => {
    it("init calls ext.init and returns success", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      const ctx = stubContext();

      const result = adapter.init(ctx);
      expect(result.status).toBe("success");
      expect(ext.init).toHaveBeenCalled();
    });

    it("exit calls ext.exit", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);

      adapter.exit();
      expect(ext.exit).toHaveBeenCalled();
    });
  });

  describe("roles", () => {
    it("hasChrome returns true", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.hasChrome()).toBe(true);
    });

    it("hasBufferContrib returns false", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.hasBufferContrib()).toBe(false);
    });

    it("hasAnnotations returns false", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.hasAnnotations()).toBe(false);
    });
  });

  describe("events", () => {
    it("onNotification delegates to applyNotification", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);

      adapter.onNotification('{"active":true}');
      expect(ext.applyNotification).toHaveBeenCalledWith('{"active":true}');
    });
  });

  describe("chrome", () => {
    it("chromePosition returns overlay", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.chromePosition()).toBe("overlay");
    });
  });

  describe("dependencies", () => {
    it("dependencies delegates to ext.dependencies", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.dependencies()).toEqual(["dep-a"]);
    });

    it("serverKinds delegates to ext.serverKinds", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.serverKinds()).toEqual(["server-kind-1"]);
    });

    it("dependencies defaults to empty when ext has none", () => {
      const ext: WebExtension = {
        kind: () => "plain",
        isActive: () => false,
        applyNotification: vi.fn(),
        render: vi.fn(),
        hide: vi.fn(),
        getState: () => null,
      };
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.dependencies()).toEqual([]);
    });

    it("serverKinds defaults to [kind] when ext has none", () => {
      const ext: WebExtension = {
        kind: () => "plain",
        isActive: () => false,
        applyNotification: vi.fn(),
        render: vi.fn(),
        hide: vi.fn(),
        getState: () => null,
      };
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.serverKinds()).toEqual(["plain"]);
    });
  });

  describe("test support", () => {
    it("getState delegates to ext.getState", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.getState()).toEqual({ kind: "test", active: false });
    });
  });

  describe("wrappedExtension", () => {
    it("exposes the original WebExtension", () => {
      const ext = mockExtension("test");
      const adapter = new WebExtensionAdapter(ext);
      expect(adapter.wrappedExtension).toBe(ext);
    });
  });
});

describe("adaptExtension", () => {
  it("wraps a single extension", () => {
    const ext = mockExtension("single");
    const adapted = adaptExtension(ext);
    expect(adapted.kind()).toBe("single");
    expect(adapted).toBeInstanceOf(WebExtensionAdapter);
  });
});

describe("adaptExtensions", () => {
  it("wraps multiple extensions", () => {
    const exts = [mockExtension("a"), mockExtension("b"), mockExtension("c")];
    const adapted = adaptExtensions(exts);
    expect(adapted).toHaveLength(3);
    expect(adapted[0]!.kind()).toBe("a");
    expect(adapted[1]!.kind()).toBe("b");
    expect(adapted[2]!.kind()).toBe("c");
    expect(adapted.every((a) => a instanceof WebExtensionAdapter)).toBe(true);
  });
});
