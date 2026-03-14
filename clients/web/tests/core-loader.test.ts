/**
 * Client Module Loader Tests (#650)
 *
 * Tests topological sort, multi-pass deferral, and lifecycle management.
 */

import { describe, it, expect, vi } from "vitest";
import { ClientModuleLoader } from "../src/core/loader.js";
import type { ClientModule, ModuleContext } from "../src/core/contracts.js";
import type { Version, ProbeResult } from "../src/core/types.js";
import { INSETS_ZERO } from "../src/core/types.js";

// ---- Helper: create a mock module ----

interface MockModuleOpts {
  kind: string;
  deps?: string[];
  initBehavior?:
    | "success"
    | "failed"
    | "defer-once"
    | "defer-always"
    | (() => ProbeResult);
}

function createMockModule(opts: MockModuleOpts): ClientModule {
  let deferCount = 0;
  const initFn = vi.fn((_ctx: ModuleContext): ProbeResult => {
    if (typeof opts.initBehavior === "function") {
      return opts.initBehavior();
    }
    switch (opts.initBehavior) {
      case "failed":
        return { status: "failed", error: `${opts.kind} failed` };
      case "defer-once":
        if (deferCount === 0) {
          deferCount++;
          return { status: "defer", reason: "not ready yet" };
        }
        return { status: "success" };
      case "defer-always":
        return { status: "defer", reason: "never ready" };
      default:
        return { status: "success" };
    }
  });

  const exitFn = vi.fn();
  const onAllLoadedFn = vi.fn();

  return {
    id: () => opts.kind,
    kind: () => opts.kind,
    name: () => opts.kind,
    version: (): Version => ({ major: 1, minor: 0, patch: 0 }),
    init: initFn,
    exit: exitFn,
    onAllLoaded: onAllLoadedFn,
    hasChrome: () => false,
    hasBufferContrib: () => false,
    hasAnnotations: () => false,
    dependencies: opts.deps ? () => opts.deps! : undefined,
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

describe("ClientModuleLoader", () => {
  describe("empty loader", () => {
    it("has zero modules", () => {
      const loader = new ClientModuleLoader([]);
      expect(loader.moduleCount()).toBe(0);
      expect(loader.runningCount()).toBe(0);
      expect(loader.modules()).toHaveLength(0);
    });

    it("initAll returns 0", () => {
      const loader = new ClientModuleLoader([]);
      expect(loader.initAll(stubContext())).toBe(0);
    });

    it("exitAll is safe on empty", () => {
      const loader = new ClientModuleLoader([]);
      loader.exitAll(); // should not throw
    });
  });

  describe("single module lifecycle", () => {
    it("init -> running -> exit -> loaded", () => {
      const mod = createMockModule({ kind: "test" });
      const loader = new ClientModuleLoader([mod]);
      const ctx = stubContext();

      expect(loader.moduleState("test")).toBe("loaded");

      const count = loader.initAll(ctx);
      expect(count).toBe(1);
      expect(loader.moduleState("test")).toBe("running");
      expect(loader.isRunning("test")).toBe(true);
      expect(loader.runningCount()).toBe(1);

      loader.exitAll();
      expect(loader.moduleState("test")).toBe("loaded");
      expect(loader.isRunning("test")).toBe(false);
      expect(loader.runningCount()).toBe(0);
    });
  });

  describe("dependency ordering", () => {
    it("sorts A depends on B -> B inits first", () => {
      const modA = createMockModule({ kind: "A", deps: ["B"] });
      const modB = createMockModule({ kind: "B" });

      // Provide out of order
      const loader = new ClientModuleLoader([modA, modB]);
      const ctx = stubContext();

      loader.initAll(ctx);

      // B should have been initialized before A
      const kinds = loader.modules().map((m) => m.kind());
      const bIdx = kinds.indexOf("B");
      const aIdx = kinds.indexOf("A");
      expect(bIdx).toBeLessThan(aIdx);
    });
  });

  describe("multi-pass deferral", () => {
    it("module defers once then succeeds on pass 2", () => {
      const mod = createMockModule({ kind: "lazy", initBehavior: "defer-once" });
      const loader = new ClientModuleLoader([mod]);
      const ctx = stubContext();

      const count = loader.initAll(ctx);
      expect(count).toBe(1);
      expect(loader.isRunning("lazy")).toBe(true);
      // init was called twice (first deferred, second succeeded)
      expect(mod.init).toHaveBeenCalledTimes(2);
    });

    it("module defers 3 times -> marked failed", () => {
      const mod = createMockModule({
        kind: "stuck",
        initBehavior: "defer-always",
      });
      const loader = new ClientModuleLoader([mod]);
      const ctx = stubContext();

      const count = loader.initAll(ctx);
      expect(count).toBe(0);
      expect(loader.isRunning("stuck")).toBe(false);

      const state = loader.moduleState("stuck");
      expect(typeof state).toBe("object");
      if (typeof state === "object" && state !== null) {
        expect(state.failed).toBe("exceeded maximum init passes");
      }
    });
  });

  describe("failed module", () => {
    it("does not block other modules", () => {
      const good = createMockModule({ kind: "good" });
      const bad = createMockModule({ kind: "bad", initBehavior: "failed" });
      const alsoGood = createMockModule({ kind: "also-good" });

      const loader = new ClientModuleLoader([good, bad, alsoGood]);
      const ctx = stubContext();

      const count = loader.initAll(ctx);
      expect(count).toBe(2);
      expect(loader.isRunning("good")).toBe(true);
      expect(loader.isRunning("bad")).toBe(false);
      expect(loader.isRunning("also-good")).toBe(true);
    });

    it("records failure message", () => {
      const bad = createMockModule({ kind: "bad", initBehavior: "failed" });
      const loader = new ClientModuleLoader([bad]);
      loader.initAll(stubContext());

      const state = loader.moduleState("bad");
      expect(typeof state).toBe("object");
      if (typeof state === "object" && state !== null) {
        expect(state.failed).toBe("bad failed");
      }
    });
  });

  describe("init exception handling", () => {
    it("catches thrown errors and marks as failed", () => {
      const throwing = createMockModule({
        kind: "throws",
        initBehavior: () => {
          throw new Error("kaboom");
        },
      });
      const loader = new ClientModuleLoader([throwing]);
      const count = loader.initAll(stubContext());

      expect(count).toBe(0);
      const state = loader.moduleState("throws");
      expect(typeof state).toBe("object");
      if (typeof state === "object" && state !== null) {
        expect(state.failed).toBe("kaboom");
      }
    });
  });

  describe("reverse-order shutdown", () => {
    it("exits modules in reverse dependency order", () => {
      const exitOrder: string[] = [];
      const modA = createMockModule({ kind: "A", deps: ["B"] });
      const modB = createMockModule({ kind: "B" });

      // Override exit to record order
      modA.exit = vi.fn(() => exitOrder.push("A"));
      modB.exit = vi.fn(() => exitOrder.push("B"));

      const loader = new ClientModuleLoader([modA, modB]);
      loader.initAll(stubContext());
      loader.exitAll();

      // A depends on B, so sorted order is [B, A].
      // Reverse = [A, B]. A exits first.
      expect(exitOrder).toEqual(["A", "B"]);
    });
  });

  describe("disabled kinds filtering", () => {
    it("excludes disabled modules", () => {
      const modA = createMockModule({ kind: "A" });
      const modB = createMockModule({ kind: "B" });
      const modC = createMockModule({ kind: "C" });

      const loader = new ClientModuleLoader(
        [modA, modB, modC],
        new Set(["B"]),
      );

      expect(loader.moduleCount()).toBe(2);
      const kinds = loader.loadedKinds();
      expect(kinds).toContain("A");
      expect(kinds).not.toContain("B");
      expect(kinds).toContain("C");
    });
  });

  describe("onAllLoaded", () => {
    it("calls onAllLoaded only on running modules", () => {
      const good = createMockModule({ kind: "good" });
      const bad = createMockModule({ kind: "bad", initBehavior: "failed" });

      const loader = new ClientModuleLoader([good, bad]);
      const ctx = stubContext();
      loader.initAll(ctx);
      loader.onAllLoaded(ctx);

      expect(good.onAllLoaded).toHaveBeenCalledTimes(1);
      expect(bad.onAllLoaded).not.toHaveBeenCalled();
    });
  });

  describe("ClientModuleRegistry interface", () => {
    it("isRunning returns correct status", () => {
      const mod = createMockModule({ kind: "test" });
      const loader = new ClientModuleLoader([mod]);

      expect(loader.isRunning("test")).toBe(false);
      loader.initAll(stubContext());
      expect(loader.isRunning("test")).toBe(true);
    });

    it("moduleState returns null for unknown kind", () => {
      const loader = new ClientModuleLoader([]);
      expect(loader.moduleState("unknown")).toBeNull();
    });

    it("loadedKinds returns all module kinds", () => {
      const mods = [
        createMockModule({ kind: "x" }),
        createMockModule({ kind: "y" }),
      ];
      const loader = new ClientModuleLoader(mods);
      expect(loader.loadedKinds()).toEqual(["x", "y"]);
    });

    it("runningCount reflects init state", () => {
      const mods = [
        createMockModule({ kind: "a" }),
        createMockModule({ kind: "b", initBehavior: "failed" }),
        createMockModule({ kind: "c" }),
      ];
      const loader = new ClientModuleLoader(mods);
      loader.initAll(stubContext());
      expect(loader.runningCount()).toBe(2);
    });
  });
});
