/**
 * Client Module Loader
 *
 * Manages the full CLM lifecycle: topological sort, multi-pass init
 * with deferral, and reverse-order shutdown. Replaces the raw
 * `createExtensions()` init loop.
 *
 * @module core/loader
 */

import type {
  ClientModule,
  ClientModuleRegistry,
  ModuleContext,
} from "./contracts.js";
import type { ClientModuleState } from "./types.js";
import { toposortExtensions } from "../extensions/toposort.js";

/** Maximum number of init passes before giving up on deferred modules. */
const MAX_INIT_PASSES = 3;

/**
 * Client module loader with multi-pass deferral and lifecycle management.
 *
 * Mirrors the Rust `ClientModuleLoader` with topological sort,
 * `ProbeResult`-based init, and reverse-order shutdown.
 */
export class ClientModuleLoader implements ClientModuleRegistry {
  private sorted: ClientModule[];
  private states: Map<string, ClientModuleState> = new Map();

  /**
   * Create a loader from an array of modules.
   *
   * @param modules - Modules to manage.
   * @param disabledKinds - Optional set of module kinds to exclude.
   */
  constructor(modules: ClientModule[], disabledKinds?: Set<string>) {
    let filtered = modules;
    if (disabledKinds && disabledKinds.size > 0) {
      filtered = modules.filter((m) => !disabledKinds.has(m.kind()));
    }

    // Topological sort by dependencies (reuses the generic toposort)
    this.sorted = toposortExtensions(filtered);

    // All modules start in "loaded" state
    for (const m of this.sorted) {
      this.states.set(m.kind(), "loaded");
    }
  }

  /**
   * Initialize all modules with multi-pass deferral.
   *
   * Runs up to `MAX_INIT_PASSES` rounds. In each round, modules that
   * returned `{ status: "defer" }` are retried. Modules that return
   * `{ status: "failed" }` are permanently marked as failed.
   *
   * @returns Number of successfully initialized modules.
   */
  initAll(ctx: ModuleContext): number {
    let pending = [...this.sorted];
    let successCount = 0;

    for (let pass = 0; pass < MAX_INIT_PASSES && pending.length > 0; pass++) {
      const deferred: ClientModule[] = [];

      for (const mod of pending) {
        this.states.set(mod.kind(), "initializing");

        try {
          const result = mod.init(ctx);

          switch (result.status) {
            case "success":
              this.states.set(mod.kind(), "running");
              successCount++;
              break;

            case "defer":
              // Will retry on next pass
              deferred.push(mod);
              this.states.set(mod.kind(), "loaded");
              break;

            case "failed":
              this.states.set(mod.kind(), { failed: result.error });
              break;
          }
        } catch (error) {
          const message =
            error instanceof Error ? error.message : String(error);
          this.states.set(mod.kind(), { failed: message });
        }
      }

      pending = deferred;
    }

    // Any still-deferred modules after all passes are marked failed
    for (const mod of pending) {
      this.states.set(mod.kind(), {
        failed: "exceeded maximum init passes",
      });
    }

    return successCount;
  }

  /**
   * Call `onAllLoaded` on all running modules.
   */
  onAllLoaded(ctx: ModuleContext): void {
    for (const mod of this.sorted) {
      if (this.states.get(mod.kind()) === "running") {
        mod.onAllLoaded?.(ctx);
      }
    }
  }

  /**
   * Shutdown all running modules in reverse dependency order.
   */
  exitAll(): void {
    for (let i = this.sorted.length - 1; i >= 0; i--) {
      const mod = this.sorted[i]!;
      if (this.states.get(mod.kind()) === "running") {
        try {
          mod.exit();
        } catch {
          // Best-effort shutdown -- log but continue
        }
      }
      this.states.set(mod.kind(), "loaded");
    }
  }

  /**
   * Get all modules (in dependency order).
   */
  modules(): readonly ClientModule[] {
    return this.sorted;
  }

  /**
   * Total number of loaded modules.
   */
  moduleCount(): number {
    return this.sorted.length;
  }

  // ---- ClientModuleRegistry implementation ----

  isRunning(kind: string): boolean {
    return this.states.get(kind) === "running";
  }

  moduleState(kind: string): ClientModuleState | null {
    return this.states.get(kind) ?? null;
  }

  loadedKinds(): string[] {
    return this.sorted.map((m) => m.kind());
  }

  runningCount(): number {
    let count = 0;
    for (const state of this.states.values()) {
      if (state === "running") count++;
    }
    return count;
  }
}
