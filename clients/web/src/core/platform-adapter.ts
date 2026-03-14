/**
 * Browser Platform Adapter
 *
 * Implements `PlatformCapabilities` for browser environments.
 * All DOM/window queries are guarded for SSR/Node safety.
 *
 * @module core/platform-adapter
 */

import type { PlatformCapabilities } from "./contracts.js";
import type { RenderingModel, ColorDepth, Insets } from "./types.js";
import { INSETS_ZERO } from "./types.js";

/** Optional overrides for testing. */
export interface PlatformOverrides {
  renderingModel?: RenderingModel;
  gridSize?: { width: number; height: number } | null;
  colorDepth?: ColorDepth;
  pixelSize?: { width: number; height: number } | null;
  reliableUnicodeWidth?: boolean;
  darkMode?: boolean;
  smoothScroll?: boolean;
  pointerEvents?: boolean;
  touchInput?: boolean;
  haptic?: boolean;
  safeArea?: Insets;
  hasFocus?: boolean;
  clipboardAvailable?: boolean;
  screenReaderActive?: boolean;
}

/**
 * Browser implementation of PlatformCapabilities.
 *
 * Queries the DOM for runtime capabilities. Safe to construct in Node/SSR
 * environments -- all queries return sensible defaults when `window` or
 * `document` is unavailable.
 */
export class BrowserPlatformAdapter implements PlatformCapabilities {
  private overrides: PlatformOverrides;

  constructor(overrides: PlatformOverrides = {}) {
    this.overrides = overrides;
  }

  renderingModel(): RenderingModel {
    return this.overrides.renderingModel ?? "Canvas";
  }

  gridSize(): { width: number; height: number } | null {
    if (this.overrides.gridSize !== undefined) return this.overrides.gridSize;
    // Web is pixel-based, not a cell grid.
    return null;
  }

  colorDepth(): ColorDepth {
    return this.overrides.colorDepth ?? "TrueColor";
  }

  pixelSize(): { width: number; height: number } | null {
    if (this.overrides.pixelSize !== undefined) return this.overrides.pixelSize;
    if (typeof window === "undefined") return null;
    return { width: window.innerWidth, height: window.innerHeight };
  }

  reliableUnicodeWidth(): boolean {
    return this.overrides.reliableUnicodeWidth ?? true;
  }

  darkMode(): boolean {
    if (this.overrides.darkMode !== undefined) return this.overrides.darkMode;
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return false;
    }
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  }

  smoothScroll(): boolean {
    return this.overrides.smoothScroll ?? true;
  }

  pointerEvents(): boolean {
    return this.overrides.pointerEvents ?? true;
  }

  touchInput(): boolean {
    if (this.overrides.touchInput !== undefined) return this.overrides.touchInput;
    if (typeof window === "undefined") return false;
    return "ontouchstart" in window || navigator.maxTouchPoints > 0;
  }

  haptic(): boolean {
    return this.overrides.haptic ?? false;
  }

  safeArea(): Insets {
    return this.overrides.safeArea ?? INSETS_ZERO;
  }

  hasFocus(): boolean {
    if (this.overrides.hasFocus !== undefined) return this.overrides.hasFocus;
    if (typeof document === "undefined") return false;
    return document.hasFocus();
  }

  clipboardAvailable(): boolean {
    if (this.overrides.clipboardAvailable !== undefined) {
      return this.overrides.clipboardAvailable;
    }
    if (typeof navigator === "undefined") return false;
    return navigator.clipboard !== undefined;
  }

  screenReaderActive(): boolean {
    return this.overrides.screenReaderActive ?? false;
  }
}
