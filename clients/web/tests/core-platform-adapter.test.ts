/**
 * Browser Platform Adapter Tests (#650)
 *
 * Tests platform capability queries in jsdom environment.
 *
 * @vitest-environment jsdom
 */

import { describe, it, expect } from "vitest";
import { BrowserPlatformAdapter } from "../src/core/platform-adapter.js";
import { INSETS_ZERO } from "../src/core/types.js";

describe("BrowserPlatformAdapter", () => {
  describe("defaults", () => {
    it("renderingModel returns Canvas", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.renderingModel()).toBe("Canvas");
    });

    it("gridSize returns null (web is pixel-based)", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.gridSize()).toBeNull();
    });

    it("colorDepth returns TrueColor", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.colorDepth()).toBe("TrueColor");
    });

    it("reliableUnicodeWidth returns true", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.reliableUnicodeWidth()).toBe(true);
    });

    it("smoothScroll returns true", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.smoothScroll()).toBe(true);
    });

    it("pointerEvents returns true", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.pointerEvents()).toBe(true);
    });

    it("haptic returns false", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.haptic()).toBe(false);
    });

    it("safeArea returns ZERO", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.safeArea()).toEqual(INSETS_ZERO);
    });

    it("screenReaderActive returns false", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(adapter.screenReaderActive()).toBe(false);
    });
  });

  describe("DOM queries in jsdom", () => {
    it("pixelSize returns window dimensions", () => {
      const adapter = new BrowserPlatformAdapter();
      const size = adapter.pixelSize();
      // jsdom provides window.innerWidth/innerHeight
      expect(size).not.toBeNull();
      expect(typeof size!.width).toBe("number");
      expect(typeof size!.height).toBe("number");
    });

    it("hasFocus queries document", () => {
      const adapter = new BrowserPlatformAdapter();
      // jsdom document.hasFocus() returns a boolean
      expect(typeof adapter.hasFocus()).toBe("boolean");
    });

    it("darkMode returns boolean from matchMedia", () => {
      const adapter = new BrowserPlatformAdapter();
      expect(typeof adapter.darkMode()).toBe("boolean");
    });
  });

  describe("overrides for testing", () => {
    it("overrides renderingModel", () => {
      const adapter = new BrowserPlatformAdapter({
        renderingModel: "CellGrid",
      });
      expect(adapter.renderingModel()).toBe("CellGrid");
    });

    it("overrides gridSize", () => {
      const adapter = new BrowserPlatformAdapter({
        gridSize: { width: 80, height: 24 },
      });
      expect(adapter.gridSize()).toEqual({ width: 80, height: 24 });
    });

    it("overrides gridSize to null", () => {
      const adapter = new BrowserPlatformAdapter({ gridSize: null });
      expect(adapter.gridSize()).toBeNull();
    });

    it("overrides darkMode", () => {
      const adapter = new BrowserPlatformAdapter({ darkMode: true });
      expect(adapter.darkMode()).toBe(true);

      const light = new BrowserPlatformAdapter({ darkMode: false });
      expect(light.darkMode()).toBe(false);
    });

    it("overrides hasFocus", () => {
      const adapter = new BrowserPlatformAdapter({ hasFocus: false });
      expect(adapter.hasFocus()).toBe(false);
    });

    it("overrides clipboardAvailable", () => {
      const adapter = new BrowserPlatformAdapter({
        clipboardAvailable: false,
      });
      expect(adapter.clipboardAvailable()).toBe(false);
    });

    it("overrides touchInput", () => {
      const adapter = new BrowserPlatformAdapter({ touchInput: true });
      expect(adapter.touchInput()).toBe(true);
    });

    it("overrides pixelSize", () => {
      const adapter = new BrowserPlatformAdapter({
        pixelSize: { width: 1920, height: 1080 },
      });
      expect(adapter.pixelSize()).toEqual({ width: 1920, height: 1080 });
    });

    it("overrides pixelSize to null", () => {
      const adapter = new BrowserPlatformAdapter({ pixelSize: null });
      expect(adapter.pixelSize()).toBeNull();
    });
  });
});
