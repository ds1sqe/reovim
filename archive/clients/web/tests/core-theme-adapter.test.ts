/**
 * Theme Provider Adapter Tests (#650)
 *
 * Tests the bridge from web ThemeManager to CLM ClmThemeProvider.
 */

import { describe, it, expect } from "vitest";
import {
  ThemeProviderAdapter,
  resolvedStyleToClmStyle,
} from "../src/core/theme-adapter.js";
import { ThemeManager, parseTheme } from "../src/theme/manager.js";
import type { ResolvedStyle } from "../src/theme/types.js";

// ---- Helper: create a ThemeManager with a simple dark theme ----

function createTestManager(): ThemeManager {
  const theme = parseTheme({
    meta: { name: "test-dark" },
    palette: {
      bg: "#282c34",
      fg: "#abb2bf",
      purple: "#c678dd",
    },
    syntax: {
      keyword: { fg: "purple", bold: true },
    },
    ui: {
      foreground: { fg: "fg" },
      background: { fg: "fg", bg: "bg" },
    },
    diagnostic: {},
    gutter: {},
  });

  return new ThemeManager(theme);
}

// ---- Tests ----

describe("resolvedStyleToClmStyle", () => {
  it("converts ResolvedStyle to CLM Style", () => {
    const resolved: ResolvedStyle = {
      fg: "#c678dd",
      bg: "#282c34",
      bold: true,
      italic: false,
      underline: false,
      strikethrough: false,
    };

    const clm = resolvedStyleToClmStyle(resolved);
    expect(clm.fg).toBe("#c678dd");
    expect(clm.bg).toBe("#282c34");
    expect(clm.bold).toBe(true);
    // false attributes become undefined (falsy -> undefined)
    expect(clm.italic).toBeUndefined();
    expect(clm.underline).toBeUndefined();
    expect(clm.strikethrough).toBeUndefined();
  });

  it("handles minimal ResolvedStyle", () => {
    const resolved: ResolvedStyle = {
      fg: "#abb2bf",
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
    };

    const clm = resolvedStyleToClmStyle(resolved);
    expect(clm.fg).toBe("#abb2bf");
    expect(clm.bg).toBeUndefined();
  });
});

describe("ThemeProviderAdapter", () => {
  it("highlight returns CLM style for known group", () => {
    const manager = createTestManager();
    const adapter = new ThemeProviderAdapter(manager);

    const style = adapter.highlight("keyword");
    expect(style).not.toBeNull();
    expect(style!.fg).toBe("#c678dd");
    expect(style!.bold).toBe(true);
  });

  it("highlight returns null for unknown group", () => {
    const manager = createTestManager();
    const adapter = new ThemeProviderAdapter(manager);

    const style = adapter.highlight("nonexistent_group_xyz");
    expect(style).toBeNull();
  });

  it("highlightWithFallback returns first match", () => {
    const manager = createTestManager();
    const adapter = new ThemeProviderAdapter(manager);

    const style = adapter.highlightWithFallback([
      "nonexistent",
      "keyword",
      "foreground",
    ]);
    expect(style).not.toBeNull();
    expect(style!.fg).toBe("#c678dd");
  });

  it("highlightWithFallback returns null if none match", () => {
    const manager = createTestManager();
    const adapter = new ThemeProviderAdapter(manager);

    const style = adapter.highlightWithFallback(["nope", "also_nope"]);
    expect(style).toBeNull();
  });

  it("foreground returns the foreground style", () => {
    const manager = createTestManager();
    const adapter = new ThemeProviderAdapter(manager);

    const style = adapter.foreground();
    expect(style.fg).toBe("#abb2bf");
  });

  it("background returns the background style", () => {
    const manager = createTestManager();
    const adapter = new ThemeProviderAdapter(manager);

    const style = adapter.background();
    expect(style.bg).toBe("#282c34");
  });

  it("isDark returns true for theme named *dark*", () => {
    const manager = createTestManager();
    const adapter = new ThemeProviderAdapter(manager);

    expect(adapter.isDark()).toBe(true);
  });

  it("isDark returns false for theme named *light*", () => {
    const lightTheme = parseTheme({
      meta: { name: "solarized-light" },
      palette: { bg: "#fdf6e3", fg: "#657b83" },
      syntax: {},
      ui: {
        foreground: { fg: "fg" },
        background: { fg: "fg", bg: "bg" },
      },
      diagnostic: {},
      gutter: {},
    });

    const manager = new ThemeManager(lightTheme);
    const adapter = new ThemeProviderAdapter(manager);
    expect(adapter.isDark()).toBe(false);
  });

  it("isDark falls back to luminance when name is ambiguous", () => {
    const darkBgTheme = parseTheme({
      meta: { name: "my-custom-theme" },
      palette: { bg: "#1a1a1a", fg: "#ffffff" },
      syntax: {},
      ui: {
        foreground: { fg: "fg" },
        background: { fg: "fg", bg: "bg" },
      },
      diagnostic: {},
      gutter: {},
    });

    const manager = new ThemeManager(darkBgTheme);
    const adapter = new ThemeProviderAdapter(manager);
    // #1a1a1a is very dark
    expect(adapter.isDark()).toBe(true);
  });
});
