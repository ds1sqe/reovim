/**
 * Core CLM Integration Tests (#650)
 *
 * Tests the full lifecycle: create modules -> init -> notification -> exit.
 * Verifies that adapter-wrapped extensions work through the loader.
 */

import { describe, it, expect, vi } from "vitest";
import { ClientModuleLoader } from "../src/core/loader.js";
import {
  WebExtensionAdapter,
  adaptExtensions,
} from "../src/core/extension-adapter.js";
import { WebClientServiceRegistry } from "../src/core/service-registry.js";
import type { WebExtension } from "../src/extensions/interface.js";
import type { ModuleContext } from "../src/core/contracts.js";
import { INSETS_ZERO } from "../src/core/types.js";

// ---- Helpers ----

function stubContext(
  loader: ClientModuleLoader,
): ModuleContext {
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
    services: new WebClientServiceRegistry(),
    moduleRegistry: loader,
  };
}

function createTestExtension(
  kind: string,
  deps: string[] = [],
): WebExtension {
  let state: Record<string, unknown> = { active: false, data: null };

  return {
    kind: () => kind,
    isActive: () => state["active"] === true,
    applyNotification: vi.fn((data: string) => {
      try {
        const parsed = JSON.parse(data) as Record<string, unknown>;
        state = { ...state, ...parsed };
      } catch {
        // ignore parse errors
      }
    }),
    render: vi.fn(),
    hide: vi.fn(),
    getState: () => ({ ...state }),
    dependencies: () => deps,
    init: vi.fn(),
    exit: vi.fn(),
  };
}

// ---- Tests ----

describe("CLM Integration", () => {
  it("full lifecycle: create -> init -> notification -> exit", () => {
    // Create extensions
    const ext1 = createTestExtension("cmdline");
    const ext2 = createTestExtension("whichkey");

    // Wrap as ClientModules
    const modules = adaptExtensions([ext1, ext2]);
    const loader = new ClientModuleLoader(modules);
    const ctx = stubContext(loader);

    // Init
    const count = loader.initAll(ctx);
    expect(count).toBe(2);
    expect(loader.runningCount()).toBe(2);

    // onAllLoaded
    loader.onAllLoaded(ctx);

    // Notification dispatch
    for (const mod of loader.modules()) {
      if (mod.kind() === "cmdline") {
        mod.onNotification?.('{"active":true,"prompt":":","input":"w"}');
      }
    }

    // Verify state reached the extension
    expect(ext1.applyNotification).toHaveBeenCalledWith(
      '{"active":true,"prompt":":","input":"w"}',
    );

    // Check getState through module
    const cmdlineMod = loader.modules().find((m) => m.kind() === "cmdline");
    const state = cmdlineMod?.getState?.();
    expect(state).toBeDefined();
    expect(state?.["active"]).toBe(true);

    // Exit
    loader.exitAll();
    expect(loader.runningCount()).toBe(0);
    expect(ext1.exit).toHaveBeenCalled();
    expect(ext2.exit).toHaveBeenCalled();
  });

  it("adapter-wrapped extensions are instanceof WebExtensionAdapter", () => {
    const ext = createTestExtension("test");
    const modules = adaptExtensions([ext]);
    const loader = new ClientModuleLoader(modules);

    for (const mod of loader.modules()) {
      expect(mod).toBeInstanceOf(WebExtensionAdapter);
      if (mod instanceof WebExtensionAdapter) {
        expect(mod.wrappedExtension).toBe(ext);
      }
    }
  });

  it("module introspection after init", () => {
    const ext1 = createTestExtension("a");
    const ext2 = createTestExtension("b", ["a"]);
    const modules = adaptExtensions([ext1, ext2]);
    const loader = new ClientModuleLoader(modules);
    const ctx = stubContext(loader);

    loader.initAll(ctx);

    // Check through ModuleContext.moduleRegistry
    expect(ctx.moduleRegistry!.isRunning("a")).toBe(true);
    expect(ctx.moduleRegistry!.isRunning("b")).toBe(true);
    expect(ctx.moduleRegistry!.runningCount()).toBe(2);
    expect(ctx.moduleRegistry!.loadedKinds()).toContain("a");
    expect(ctx.moduleRegistry!.loadedKinds()).toContain("b");
  });

  it("disabled modules are excluded", () => {
    const ext1 = createTestExtension("keep");
    const ext2 = createTestExtension("skip");
    const modules = adaptExtensions([ext1, ext2]);
    const loader = new ClientModuleLoader(modules, new Set(["skip"]));

    expect(loader.moduleCount()).toBe(1);
    expect(loader.loadedKinds()).toEqual(["keep"]);
  });

  it("service registry is shared across modules", () => {
    const ext1 = createTestExtension("provider");
    const ext2 = createTestExtension("consumer");
    const modules = adaptExtensions([ext1, ext2]);
    const loader = new ClientModuleLoader(modules);
    const ctx = stubContext(loader);

    loader.initAll(ctx);

    // Provider registers a service
    ctx.services!.register("search-api", { search: (q: string) => `found: ${q}` });

    // Consumer retrieves it
    const searchApi = ctx.services!.get<{ search: (q: string) => string }>("search-api");
    expect(searchApi).toBeDefined();
    expect(searchApi!.search("test")).toBe("found: test");
  });
});
