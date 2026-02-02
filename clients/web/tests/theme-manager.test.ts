/**
 * ThemeManager Tests
 *
 * Tests for the 4-tier lookup theme system.
 * Part of Phase 13.1 - Web Theme Engine.
 */

import { describe, it, expect, beforeEach } from "vitest";
import {
  ThemeManager,
  parseTheme,
  StyleGroupRegistry,
  createDefaultThemeManager,
  darkTheme,
  lightTheme,
  tokyoNightOrangeTheme,
  ALL_GROUPS,
  SYNTAX_GROUPS,
  UI_GROUPS,
} from "../src/theme/index.js";
import type { Theme, ResolvedStyle, ThemeProvider } from "../src/theme/index.js";

// ============ Test Fixtures ============

/**
 * Minimal theme for testing.
 */
const minimalTheme: Theme = {
  meta: {
    name: "test-minimal",
    author: "test",
    version: "1.0.0",
  },
  palette: {
    bg: "#000000",
    fg: "#ffffff",
    red: "#ff0000",
    green: "#00ff00",
    blue: "#0000ff",
  },
  syntax: {
    keyword: { fg: "red", bold: true },
    function: { fg: "blue" },
    comment: { fg: "fg", italic: true },
  },
  ui: {
    background: { bg: "bg" },
    foreground: { fg: "fg" },
    cursor: { fg: "bg", bg: "fg" },
  },
  diagnostic: {},
  gutter: {},
};

/**
 * Theme with hierarchical groups.
 */
const hierarchicalTheme: Theme = {
  meta: {
    name: "test-hierarchical",
    author: "test",
    version: "1.0.0",
  },
  palette: {
    purple: "#c678dd",
    blue: "#61afef",
    cyan: "#56b6c2",
  },
  syntax: {
    keyword: { fg: "purple", bold: true },
    "keyword.control": { fg: "blue" },
    // keyword.control.flow intentionally NOT defined - should fallback
  },
  ui: {
    foreground: { fg: "#ffffff" },
  },
  diagnostic: {},
  gutter: {},
};

/**
 * Create a simple mock ThemeProvider.
 */
function createMockProvider(
  name: string,
  styles: Map<string, ResolvedStyle>
): ThemeProvider {
  const defaultStyle: ResolvedStyle = {
    fg: "#888888",
    bold: false,
    italic: false,
    underline: false,
    strikethrough: false,
  };

  return {
    name: () => name,
    getStyle: (group: string) => styles.get(group) ?? null,
    defaultStyle: () => defaultStyle,
  };
}

// ============ parseTheme Tests ============

describe("parseTheme", () => {
  it("parses minimal theme successfully", () => {
    const provider = parseTheme(minimalTheme);
    expect(provider.name()).toBe("test-minimal");
  });

  it("resolves palette references to hex colors", () => {
    const provider = parseTheme(minimalTheme);
    const keywordStyle = provider.getStyle("keyword");

    expect(keywordStyle).not.toBeNull();
    expect(keywordStyle!.fg).toBe("#ff0000"); // "red" → "#ff0000"
    expect(keywordStyle!.bold).toBe(true);
  });

  it("passes through direct hex colors", () => {
    const theme: Theme = {
      meta: { name: "hex-test", author: "test", version: "1.0.0" },
      palette: {},
      syntax: { keyword: { fg: "#abcdef" } },
      ui: { foreground: { fg: "#123456" } },
      diagnostic: {},
      gutter: {},
    };

    const provider = parseTheme(theme);
    const style = provider.getStyle("keyword");
    expect(style!.fg).toBe("#abcdef");
  });

  it("handles missing optional style properties", () => {
    const provider = parseTheme(minimalTheme);
    const funcStyle = provider.getStyle("function");

    expect(funcStyle).not.toBeNull();
    expect(funcStyle!.bold).toBe(false);
    expect(funcStyle!.italic).toBe(false);
    expect(funcStyle!.underline).toBe(false);
    expect(funcStyle!.strikethrough).toBe(false);
  });

  it("returns null for undefined groups", () => {
    const provider = parseTheme(minimalTheme);
    expect(provider.getStyle("nonexistent")).toBeNull();
  });

  it("provides default style from foreground group", () => {
    const provider = parseTheme(minimalTheme);
    const defaultStyle = provider.defaultStyle();

    expect(defaultStyle.fg).toBe("#ffffff"); // fg from foreground group
  });

  it("handles all style modifiers", () => {
    const theme: Theme = {
      meta: { name: "modifiers-test", author: "test", version: "1.0.0" },
      palette: {},
      syntax: {
        keyword: {
          fg: "#ff0000",
          bg: "#000000",
          bold: true,
          italic: true,
          underline: true,
          strikethrough: true,
          underlineColor: "#00ff00",
        },
      },
      ui: { foreground: { fg: "#ffffff" } },
      diagnostic: {},
      gutter: {},
    };

    const provider = parseTheme(theme);
    const style = provider.getStyle("keyword")!;

    expect(style.fg).toBe("#ff0000");
    expect(style.bg).toBe("#000000");
    expect(style.bold).toBe(true);
    expect(style.italic).toBe(true);
    expect(style.underline).toBe(true);
    expect(style.strikethrough).toBe(true);
    expect(style.underlineColor).toBe("#00ff00");
  });
});

// ============ ThemeManager - 4 Tier Lookup Tests ============

describe("ThemeManager - 4-Tier Lookup", () => {
  let manager: ThemeManager;

  beforeEach(() => {
    const provider = parseTheme(minimalTheme);
    manager = new ThemeManager(provider);
  });

  describe("Tier 1: User Overrides", () => {
    it("override takes precedence over theme", () => {
      const override: ResolvedStyle = {
        fg: "#override",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      };

      manager.setOverride("keyword", override);
      const style = manager.getStyle("keyword");

      expect(style.fg).toBe("#override");
    });

    it("removeOverride restores theme style", () => {
      manager.setOverride("keyword", {
        fg: "#override",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });

      manager.removeOverride("keyword");
      const style = manager.getStyle("keyword");

      expect(style.fg).toBe("#ff0000"); // Original red from theme
    });

    it("clearOverrides removes all overrides", () => {
      manager.setOverride("keyword", {
        fg: "#a",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });
      manager.setOverride("function", {
        fg: "#b",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });

      expect(manager.overrideCount()).toBe(2);

      manager.clearOverrides();

      expect(manager.overrideCount()).toBe(0);
      expect(manager.getStyle("keyword").fg).toBe("#ff0000");
    });

    it("hasOverride returns correct status", () => {
      expect(manager.hasOverride("keyword")).toBe(false);

      manager.setOverride("keyword", {
        fg: "#x",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });

      expect(manager.hasOverride("keyword")).toBe(true);
      expect(manager.hasOverride("function")).toBe(false);
    });
  });

  describe("Tier 2: Current Theme", () => {
    it("returns theme style for defined groups", () => {
      const style = manager.getStyle("keyword");
      expect(style.fg).toBe("#ff0000");
      expect(style.bold).toBe(true);
    });

    it("returns theme style for comment with italic", () => {
      const style = manager.getStyle("comment");
      expect(style.italic).toBe(true);
    });
  });

  describe("Tier 3: Module Defaults", () => {
    it("uses module defaults when theme lacks group", () => {
      const registry = new StyleGroupRegistry();
      registry.register("custom.module.group", {
        fg: "#module",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });

      manager.setModuleDefaults(registry);
      const style = manager.getStyle("custom.module.group");

      expect(style.fg).toBe("#module");
    });

    it("theme takes precedence over module defaults", () => {
      const registry = new StyleGroupRegistry();
      registry.register("keyword", {
        fg: "#module-keyword",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });

      manager.setModuleDefaults(registry);
      const style = manager.getStyle("keyword");

      expect(style.fg).toBe("#ff0000"); // Theme keyword, not module
    });

    it("getModuleDefaults returns the registry", () => {
      expect(manager.getModuleDefaults()).toBeNull();

      const registry = new StyleGroupRegistry();
      manager.setModuleDefaults(registry);

      expect(manager.getModuleDefaults()).toBe(registry);
    });
  });

  describe("Tier 4: Theme Default", () => {
    it("falls back to theme default for unknown groups", () => {
      const style = manager.getStyle("completely.unknown.group");
      // Should be the default style from the theme
      expect(style.fg).toBe("#ffffff"); // Default fg from foreground
    });
  });

  describe("Tier Priority Order", () => {
    it("override > theme > module defaults > default", () => {
      const registry = new StyleGroupRegistry();
      registry.register("test.group", {
        fg: "#module",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });
      manager.setModuleDefaults(registry);

      // Tier 4: No definition → theme default
      expect(manager.getStyle("undefined.group").fg).toBe("#ffffff");

      // Tier 3: Module default exists
      expect(manager.getStyle("test.group").fg).toBe("#module");

      // Tier 2: Theme exists
      expect(manager.getStyle("keyword").fg).toBe("#ff0000");

      // Tier 1: Override takes priority
      manager.setOverride("keyword", {
        fg: "#override",
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
      });
      expect(manager.getStyle("keyword").fg).toBe("#override");
    });
  });
});

// ============ Hierarchical Fallback Tests ============

describe("ThemeManager - Hierarchical Fallback", () => {
  let manager: ThemeManager;

  beforeEach(() => {
    const provider = parseTheme(hierarchicalTheme);
    manager = new ThemeManager(provider);
  });

  it("exact match returns exact style", () => {
    const style = manager.getStyle("keyword.control");
    expect(style.fg).toBe("#61afef"); // blue, the exact definition
  });

  it("falls back to parent when child not defined", () => {
    // keyword.control.flow is NOT defined
    // Should fallback to keyword.control → #61afef
    const style = manager.getStyle("keyword.control.flow");
    expect(style.fg).toBe("#61afef");
  });

  it("falls back multiple levels", () => {
    // keyword.special.rare.deep is NOT defined
    // Should fallback: keyword.special.rare → keyword.special → keyword → #c678dd
    const style = manager.getStyle("keyword.special.rare.deep");
    expect(style.fg).toBe("#c678dd"); // purple from base keyword
  });

  it("root group returns its style", () => {
    const style = manager.getStyle("keyword");
    expect(style.fg).toBe("#c678dd");
    expect(style.bold).toBe(true);
  });

  it("completely unknown group falls to theme default", () => {
    const style = manager.getStyle("not.a.real.category.at.all");
    expect(style.fg).toBe("#ffffff"); // foreground default
  });

  it("tryGetStyle returns null for missing groups", () => {
    expect(manager.tryGetStyle("not.defined")).toBeNull();
  });
});

// ============ Theme Registration & Switching Tests ============

describe("ThemeManager - Theme Registration", () => {
  let manager: ThemeManager;

  beforeEach(() => {
    const provider = parseTheme(minimalTheme);
    manager = new ThemeManager(provider);
  });

  it("registers and lists themes", () => {
    manager.registerTheme(lightTheme);

    expect(manager.hasTheme("light")).toBe(true);
    expect(manager.listThemes()).toContain("light");
  });

  it("default theme is registered automatically", () => {
    expect(manager.hasTheme("test-minimal")).toBe(true);
  });

  it("registerTheme accepts Theme definition", () => {
    const custom: Theme = {
      meta: { name: "custom-theme", author: "test", version: "1.0.0" },
      palette: {},
      syntax: {},
      ui: { foreground: { fg: "#aabbcc" } },
      diagnostic: {},
      gutter: {},
    };

    manager.registerTheme(custom);
    expect(manager.hasTheme("custom-theme")).toBe(true);
  });

  it("registerProvider accepts ThemeProvider directly", () => {
    const mockProvider = createMockProvider("mock-theme", new Map());
    manager.registerProvider(mockProvider);

    expect(manager.hasTheme("mock-theme")).toBe(true);
  });
});

describe("ThemeManager - Theme Switching", () => {
  let manager: ThemeManager;

  beforeEach(() => {
    manager = createDefaultThemeManager();
  });

  it("starts with dark theme as default", () => {
    expect(manager.currentThemeName()).toBe("dark");
  });

  it("switches to light theme", () => {
    const result = manager.setTheme("light");

    expect(result).toBe(true);
    expect(manager.currentThemeName()).toBe("light");
  });

  it("switches to tokyo-night-orange theme", () => {
    const result = manager.setTheme("tokyo-night-orange");

    expect(result).toBe(true);
    expect(manager.currentThemeName()).toBe("tokyo-night-orange");
  });

  it("returns false for unknown theme", () => {
    const result = manager.setTheme("nonexistent-theme");

    expect(result).toBe(false);
    expect(manager.currentThemeName()).toBe("dark"); // Unchanged
  });

  it("setThemeProvider switches and registers", () => {
    const mockProvider = createMockProvider("new-theme", new Map());
    manager.setThemeProvider(mockProvider);

    expect(manager.currentThemeName()).toBe("new-theme");
    expect(manager.hasTheme("new-theme")).toBe(true);
  });

  it("style lookup reflects current theme after switch", () => {
    const darkKeyword = manager.getStyle("keyword");

    manager.setTheme("light");
    const lightKeyword = manager.getStyle("keyword");

    // Different themes have different colors
    expect(darkKeyword.fg).not.toBe(lightKeyword.fg);
  });
});

// ============ StyleGroupRegistry Tests ============

describe("StyleGroupRegistry", () => {
  let registry: StyleGroupRegistry;

  beforeEach(() => {
    registry = new StyleGroupRegistry();
  });

  it("registers and retrieves styles", () => {
    const style: ResolvedStyle = {
      fg: "#custom",
      bold: true,
      italic: false,
      underline: false,
      strikethrough: false,
    };

    registry.register("my.custom.group", style);
    expect(registry.get("my.custom.group")).toEqual(style);
  });

  it("returns null for unregistered groups", () => {
    expect(registry.get("not.registered")).toBeNull();
  });

  it("registerBatch registers multiple styles", () => {
    registry.registerBatch([
      [
        "group.a",
        {
          fg: "#a",
          bold: false,
          italic: false,
          underline: false,
          strikethrough: false,
        },
      ],
      [
        "group.b",
        {
          fg: "#b",
          bold: false,
          italic: false,
          underline: false,
          strikethrough: false,
        },
      ],
    ]);

    expect(registry.get("group.a")!.fg).toBe("#a");
    expect(registry.get("group.b")!.fg).toBe("#b");
  });

  it("clear removes all registrations", () => {
    registry.register("test", {
      fg: "#x",
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
    });
    registry.clear();

    expect(registry.get("test")).toBeNull();
  });

  it("has returns correct status", () => {
    expect(registry.has("test")).toBe(false);

    registry.register("test", {
      fg: "#x",
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
    });

    expect(registry.has("test")).toBe(true);
  });
});

// ============ Built-in Theme Tests ============

describe("Built-in Themes", () => {
  describe("darkTheme", () => {
    let provider: ThemeProvider;

    beforeEach(() => {
      provider = parseTheme(darkTheme);
    });

    it("has correct name", () => {
      expect(provider.name()).toBe("dark");
    });

    it("defines all syntax groups", () => {
      for (const group of SYNTAX_GROUPS) {
        const style = provider.getStyle(group);
        expect(style).not.toBeNull();
      }
    });

    it("has distinctive keyword color", () => {
      const style = provider.getStyle("keyword");
      expect(style).not.toBeNull();
      expect(style!.fg).toBe("#c678dd"); // purple
      expect(style!.bold).toBe(true);
    });
  });

  describe("lightTheme", () => {
    let provider: ThemeProvider;

    beforeEach(() => {
      provider = parseTheme(lightTheme);
    });

    it("has correct name", () => {
      expect(provider.name()).toBe("light");
    });

    it("has light background", () => {
      const bg = provider.getStyle("background");
      expect(bg).not.toBeNull();
      expect(bg!.bg).toBe("#fafafa");
    });
  });

  describe("tokyoNightOrangeTheme", () => {
    let provider: ThemeProvider;

    beforeEach(() => {
      provider = parseTheme(tokyoNightOrangeTheme);
    });

    it("has correct name", () => {
      expect(provider.name()).toBe("tokyo-night-orange");
    });

    it("has orange mode indicator", () => {
      const mode = provider.getStyle("mode_normal");
      expect(mode).not.toBeNull();
      expect(mode!.bg).toBe("#ff9e64"); // orange
    });
  });
});

// ============ Highlight Groups Tests ============

describe("Highlight Groups", () => {
  it("ALL_GROUPS contains 42 groups", () => {
    expect(ALL_GROUPS.length).toBe(42);
  });

  it("SYNTAX_GROUPS contains 13 groups", () => {
    expect(SYNTAX_GROUPS.length).toBe(13);
  });

  it("UI_GROUPS contains 17 groups", () => {
    expect(UI_GROUPS.length).toBe(17);
  });

  it("groups are unique", () => {
    const unique = new Set(ALL_GROUPS);
    expect(unique.size).toBe(ALL_GROUPS.length);
  });
});

// ============ createDefaultThemeManager Tests ============

describe("createDefaultThemeManager", () => {
  it("creates manager with dark as default", () => {
    const manager = createDefaultThemeManager();
    expect(manager.currentThemeName()).toBe("dark");
  });

  it("has all three built-in themes registered", () => {
    const manager = createDefaultThemeManager();
    expect(manager.hasTheme("dark")).toBe(true);
    expect(manager.hasTheme("light")).toBe(true);
    expect(manager.hasTheme("tokyo-night-orange")).toBe(true);
  });

  it("lists exactly 3 themes", () => {
    const manager = createDefaultThemeManager();
    expect(manager.listThemes().length).toBe(3);
  });
});
