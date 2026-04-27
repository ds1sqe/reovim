/**
 * Theme Provider Adapter
 *
 * Bridges the existing web `ThemeManager` to the CLM `ClmThemeProvider` interface.
 *
 * @module core/theme-adapter
 */

import type { ClmThemeProvider } from "./contracts.js";
import type { Style } from "./types.js";
import type { ThemeManager } from "../theme/manager.js";
import type { ResolvedStyle } from "../theme/types.js";

/**
 * Convert a web `ResolvedStyle` to a CLM `Style`.
 *
 * The web theme system uses resolved hex colors with mandatory `fg` and
 * boolean attributes. The CLM `Style` uses optional fields.
 */
export function resolvedStyleToClmStyle(resolved: ResolvedStyle): Style {
  return {
    fg: resolved.fg,
    bg: resolved.bg,
    bold: resolved.bold || undefined,
    italic: resolved.italic || undefined,
    underline: resolved.underline || undefined,
    strikethrough: resolved.strikethrough || undefined,
  };
}

/**
 * Adapter bridging `ThemeManager` to `ClmThemeProvider`.
 *
 * Delegates all lookups to the existing 4-tier `ThemeManager` and
 * converts `ResolvedStyle` results to CLM `Style`.
 */
export class ThemeProviderAdapter implements ClmThemeProvider {
  private manager: ThemeManager;

  constructor(manager: ThemeManager) {
    this.manager = manager;
  }

  highlight(group: string): Style | null {
    const resolved = this.manager.tryGetStyle(group);
    if (!resolved) return null;
    return resolvedStyleToClmStyle(resolved);
  }

  highlightWithFallback(groups: string[]): Style | null {
    for (const group of groups) {
      const style = this.highlight(group);
      if (style) return style;
    }
    return null;
  }

  foreground(): Style {
    const resolved = this.manager.getStyle("foreground");
    return resolvedStyleToClmStyle(resolved);
  }

  background(): Style {
    const resolved = this.manager.getStyle("background");
    return resolvedStyleToClmStyle(resolved);
  }

  isDark(): boolean {
    const name = this.manager.currentThemeName().toLowerCase();
    if (name.includes("dark")) return true;
    if (name.includes("light")) return false;

    // Heuristic: check background luminance
    const bgStyle = this.manager.getStyle("background");
    if (bgStyle.bg) {
      return isColorDark(bgStyle.bg);
    }
    // Default to dark if unknown
    return true;
  }
}

/**
 * Simple luminance check for hex colors.
 * Returns true if the color is "dark" (low luminance).
 */
function isColorDark(hex: string): boolean {
  // Strip leading #
  const clean = hex.startsWith("#") ? hex.slice(1) : hex;
  if (clean.length < 6) return true;

  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);

  // Relative luminance (simplified sRGB)
  const luminance = 0.299 * r + 0.587 * g + 0.114 * b;
  return luminance < 128;
}
