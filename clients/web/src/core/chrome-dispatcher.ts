/**
 * Chrome Dispatcher
 *
 * Owns the chrome compositor, DOM container cache, and rendering
 * dispatch for chrome modules. Extracted from Editor to isolate
 * the chrome rendering concern.
 *
 * @module core/chrome-dispatcher
 */

import type { ClientModule } from "./contracts.js";
import type { Rect } from "./types.js";
import { ChromeCompositor, type ChromeRegion } from "./compositor.js";
import { DomChromeSurface } from "./dom-surface.js";
import { WebExtensionAdapter } from "./extension-adapter.js";

/**
 * Manages chrome region allocation and rendering dispatch.
 *
 * - Uses `ChromeCompositor` for layout computation
 * - Maintains a persistent DOM container cache (avoids DOM thrashing)
 * - Supports two rendering paths:
 *   1. DOM path (WebExtensionAdapter) — delegates to wrappedExtension.render()
 *   2. Cell-grid path (native CLM) — creates DomChromeSurface, calls chromeRender()
 */
export class ChromeDispatcher {
  private readonly compositor: ChromeCompositor;
  private readonly parentContainer: HTMLElement;
  private readonly containers: Map<string, HTMLElement> = new Map();

  constructor(parentContainer: HTMLElement) {
    this.compositor = new ChromeCompositor();
    this.parentContainer = parentContainer;
  }

  /**
   * Compute layout, update containers, render all chrome modules.
   *
   * @returns The remaining viewport rect after chrome allocation.
   */
  dispatch(
    modules: readonly ClientModule[],
    screenWidth: number,
    screenHeight: number,
  ): Rect {
    const layout = this.compositor.computeLayout(
      modules,
      screenWidth,
      screenHeight,
    );

    // Track which modules still have chrome
    const activeIds = new Set<string>();

    for (const region of layout.regions) {
      activeIds.add(region.moduleId);

      const mod = modules.find((m) => m.id() === region.moduleId);
      if (!mod) continue;

      const container = this.getOrCreateContainer(region);
      this.positionContainer(container, region);
      this.renderModule(mod, container, region.rect);
    }

    // Hide containers for modules that no longer have chrome
    for (const [id, container] of this.containers) {
      if (!activeIds.has(id)) {
        container.style.display = "none";
      }
    }

    return layout.viewport;
  }

  /**
   * Re-render a single module's chrome (notification-triggered update).
   */
  handleModuleUpdate(
    moduleId: string,
    modules: readonly ClientModule[],
    screenWidth: number,
    screenHeight: number,
  ): void {
    const mod = modules.find((m) => m.id() === moduleId);
    if (!mod || !mod.hasChrome()) return;

    // Recompute layout to get the region for this module
    const layout = this.compositor.computeLayout(
      modules,
      screenWidth,
      screenHeight,
    );

    const region = layout.regions.find((r) => r.moduleId === moduleId);
    if (!region) return;

    const container = this.getOrCreateContainer(region);
    this.positionContainer(container, region);
    this.renderModule(mod, container, region.rect);
  }

  /** Remove all managed containers from the DOM. */
  destroy(): void {
    for (const container of this.containers.values()) {
      container.remove();
    }
    this.containers.clear();
  }

  /**
   * Render a single module into its container.
   * Handles both DOM path (WebExtensionAdapter) and cell-grid path (native CLM).
   */
  private renderModule(
    mod: ClientModule,
    container: HTMLElement,
    rect: Rect,
  ): void {
    if (mod instanceof WebExtensionAdapter) {
      // DOM rendering path
      const webExt = mod.wrappedExtension;
      if (webExt.isActive()) {
        container.style.display = "";
        webExt.render(container);
      } else {
        container.style.display = "none";
        webExt.hide();
      }
    } else if (mod.chromeRender) {
      // Cell-grid rendering path (native CLM modules)
      container.style.display = "";
      const surface = new DomChromeSurface(container, rect);
      mod.chromeRender(surface, rect);
      surface.flush();
    }
  }

  private getOrCreateContainer(region: ChromeRegion): HTMLElement {
    let container = this.containers.get(region.moduleId);
    if (!container) {
      container = document.createElement("div");
      container.dataset.chromeModule = region.moduleId;
      container.style.position = "absolute";
      this.parentContainer.appendChild(container);
      this.containers.set(region.moduleId, container);
    }
    return container;
  }

  private positionContainer(
    container: HTMLElement,
    region: ChromeRegion,
  ): void {
    container.style.left = `${region.rect.x}px`;
    container.style.top = `${region.rect.y}px`;
    container.style.width = `${region.rect.width}px`;
    container.style.height = `${region.rect.height}px`;
    container.style.zIndex = `${region.zOrder}`;
  }
}
