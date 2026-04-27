/**
 * Theme manager for runtime theme management.
 *
 * Provides centralized theme management with 4-tier lookup:
 * 1. User overrides (setOverride)
 * 2. Current theme (ThemeProvider.getStyle)
 * 3. Module defaults (StyleGroupRegistry)
 * 4. Theme default (ThemeProvider.defaultStyle)
 *
 * @module theme/manager
 */

import type { ResolvedStyle, Theme, StyleDef } from './types';
import { createDefaultStyle, DEFAULT_FG, DEFAULT_BG } from './types';
import type { ThemeProvider } from './provider';
import { StyleGroupRegistry } from './registry';
import { ALL_GROUPS, FOREGROUND } from './groups';

// =============================================================================
// Theme Parser
// =============================================================================

/**
 * Parse a Theme definition into a ThemeProvider.
 *
 * Resolves all palette references to actual hex colors.
 */
export function parseTheme(theme: Theme): ThemeProvider {
  const styles = new Map<string, ResolvedStyle>();
  const palette = theme.palette;

  // Helper to resolve a color (palette name or hex)
  const resolveColor = (color: string | undefined): string | undefined => {
    if (!color) return undefined;
    // Check palette first
    if (palette[color]) return palette[color];
    // Already a hex color
    if (color.startsWith('#')) return color;
    // Unknown - return as-is (might be a CSS color name)
    return color;
  };

  // Helper to resolve a StyleDef to ResolvedStyle
  const resolveStyle = (def: StyleDef): ResolvedStyle => {
    return {
      fg: resolveColor(def.fg) ?? DEFAULT_FG,
      bg: resolveColor(def.bg),
      bold: def.bold ?? false,
      italic: def.italic ?? false,
      underline: def.underline ?? false,
      strikethrough: def.strikethrough ?? false,
      underlineColor: resolveColor(def.underlineColor),
    };
  };

  // Process all sections
  const sections = [
    theme.syntax,
    theme.ui,
    theme.diagnostic,
    theme.gutter,
  ];

  for (const section of sections) {
    if (section) {
      for (const [group, def] of Object.entries(section)) {
        styles.set(group, resolveStyle(def));
      }
    }
  }

  // Get default style (foreground)
  const defaultStyle = styles.get(FOREGROUND) ?? createDefaultStyle();

  return {
    getStyle(group: string): ResolvedStyle | null {
      return styles.get(group) ?? null;
    },
    name(): string {
      return theme.meta.name;
    },
    defaultStyle(): ResolvedStyle {
      return defaultStyle;
    },
  };
}

// =============================================================================
// ThemeManager
// =============================================================================

/** localStorage key for persisting theme choice */
const STORAGE_KEY = 'reovim:theme';

/**
 * Manages the current theme and user overrides.
 *
 * Implements 4-tier lookup with hierarchical fallback:
 * 1. User overrides (setOverride)
 * 2. Current theme (ThemeProvider.getStyle)
 * 3. Module defaults (StyleGroupRegistry)
 * 4. Theme default (ThemeProvider.defaultStyle)
 *
 * Hierarchical fallback: "keyword.control" → "keyword" → default
 *
 * @example
 * ```typescript
 * const manager = new ThemeManager(darkTheme);
 *
 * // Override a specific style
 * manager.setOverride('keyword', { fg: '#ff0000', ... });
 *
 * // Get style (checks 4 tiers with hierarchical fallback)
 * const style = manager.getStyle('keyword.control');
 *
 * // Switch themes
 * manager.setTheme(lightTheme);
 * ```
 */
export class ThemeManager {
  /** Current theme */
  private current: ThemeProvider;

  /** User style overrides (take precedence over theme) */
  private overrides: Map<string, ResolvedStyle>;

  /** Module-provided style defaults (fallback when theme doesn't define a group) */
  private moduleDefaults: StyleGroupRegistry | null;

  /** Theme registry (name → ThemeProvider) */
  private registry: Map<string, ThemeProvider>;

  constructor(defaultTheme: ThemeProvider) {
    this.current = defaultTheme;
    this.overrides = new Map();
    this.moduleDefaults = null;
    this.registry = new Map();

    // Register the default theme
    this.registry.set(defaultTheme.name(), defaultTheme);
  }

  // ===========================================================================
  // Theme Registration
  // ===========================================================================

  /**
   * Register a theme.
   *
   * @param theme - Theme definition to register
   */
  registerTheme(theme: Theme): void {
    const provider = parseTheme(theme);
    this.registry.set(provider.name(), provider);
  }

  /**
   * Register a theme provider directly.
   *
   * @param provider - ThemeProvider to register
   */
  registerProvider(provider: ThemeProvider): void {
    this.registry.set(provider.name(), provider);
  }

  /**
   * List available theme names.
   */
  listThemes(): string[] {
    return Array.from(this.registry.keys());
  }

  /**
   * Check if a theme is registered.
   */
  hasTheme(name: string): boolean {
    return this.registry.has(name);
  }

  // ===========================================================================
  // Theme Switching
  // ===========================================================================

  /**
   * Set the current theme by name.
   *
   * @param name - Theme name to activate
   * @returns true if theme was found and activated
   */
  setTheme(name: string): boolean {
    const theme = this.registry.get(name);
    if (!theme) return false;

    this.current = theme;
    this.saveToStorage();
    this.applyToCSSVariables();
    return true;
  }

  /**
   * Set the current theme directly.
   *
   * @param theme - ThemeProvider to activate
   */
  setThemeProvider(theme: ThemeProvider): void {
    this.current = theme;
    this.registry.set(theme.name(), theme);
    this.saveToStorage();
    this.applyToCSSVariables();
  }

  /**
   * Get the current theme.
   */
  currentTheme(): ThemeProvider {
    return this.current;
  }

  /**
   * Get the current theme name.
   */
  currentThemeName(): string {
    return this.current.name();
  }

  // ===========================================================================
  // Module Defaults
  // ===========================================================================

  /**
   * Set the module defaults registry.
   *
   * Modules register their style group defaults here during init.
   * The registry is used as tier 3 in the lookup order.
   */
  setModuleDefaults(registry: StyleGroupRegistry): void {
    this.moduleDefaults = registry;
  }

  /**
   * Get the module defaults registry.
   */
  getModuleDefaults(): StyleGroupRegistry | null {
    return this.moduleDefaults;
  }

  // ===========================================================================
  // User Overrides
  // ===========================================================================

  /**
   * Set a style override for a highlight group.
   *
   * Overrides take precedence over theme-defined styles.
   */
  setOverride(group: string, style: ResolvedStyle): void {
    this.overrides.set(group, style);
  }

  /**
   * Remove a style override.
   */
  removeOverride(group: string): boolean {
    return this.overrides.delete(group);
  }

  /**
   * Clear all style overrides.
   */
  clearOverrides(): void {
    this.overrides.clear();
  }

  /**
   * Check if a group has an override.
   */
  hasOverride(group: string): boolean {
    return this.overrides.has(group);
  }

  /**
   * Get the number of overrides.
   */
  overrideCount(): number {
    return this.overrides.size;
  }

  // ===========================================================================
  // Style Lookup (4-Tier with Hierarchical Fallback)
  // ===========================================================================

  /**
   * Get the style for a highlight group with hierarchical fallback.
   *
   * Uses 4-tier lookup order:
   * 1. User overrides (setOverride)
   * 2. Current theme (ThemeProvider.getStyle)
   * 3. Module defaults (StyleGroupRegistry)
   * 4. Theme default (ThemeProvider.defaultStyle)
   *
   * **Hierarchical Fallback**: If "keyword.control" is not found,
   * walks up: "keyword.control" → "keyword" → default.
   */
  getStyle(group: string): ResolvedStyle {
    // Try exact match with full 4-tier lookup
    const exact = this.lookupExact(group);
    if (exact) return exact;

    // Hierarchical fallback: walk up the dot-separated hierarchy
    let current = group;
    while (current.includes('.')) {
      current = current.substring(0, current.lastIndexOf('.'));
      const fallback = this.lookupExact(current);
      if (fallback) return fallback;
    }

    // Final fallback to theme default
    return this.current.defaultStyle();
  }

  /**
   * Perform exact lookup through all 4 tiers (no hierarchical fallback).
   */
  private lookupExact(group: string): ResolvedStyle | null {
    // Tier 1: User overrides
    const override = this.overrides.get(group);
    if (override) return override;

    // Tier 2: Current theme
    const themeStyle = this.current.getStyle(group);
    if (themeStyle) return themeStyle;

    // Tier 3: Module defaults
    if (this.moduleDefaults) {
      const moduleStyle = this.moduleDefaults.get(group);
      if (moduleStyle) return moduleStyle;
    }

    return null;
  }

  /**
   * Get the style for a highlight group, returning null if not found.
   *
   * Checks tiers 1-3 only (overrides → theme → module defaults).
   * Unlike getStyle, this doesn't fall back to the theme's default style.
   */
  tryGetStyle(group: string): ResolvedStyle | null {
    return this.lookupExact(group);
  }

  // ===========================================================================
  // Persistence (localStorage)
  // ===========================================================================

  /**
   * Load theme choice from localStorage.
   */
  loadFromStorage(): void {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved && this.registry.has(saved)) {
        this.current = this.registry.get(saved)!;
      }
    } catch {
      // localStorage not available (SSR, private browsing)
    }
  }

  /**
   * Save current theme choice to localStorage.
   */
  private saveToStorage(): void {
    try {
      localStorage.setItem(STORAGE_KEY, this.current.name());
    } catch {
      // localStorage not available
    }
  }

  // ===========================================================================
  // CSS Variable Application
  // ===========================================================================

  /**
   * Apply all theme styles as CSS custom properties.
   *
   * Sets --theme-{group} variables on document.documentElement.
   * Provides convenient aliases for common UI elements.
   *
   * Safe to call in Node.js (no-op if document is unavailable).
   */
  applyToCSSVariables(): void {
    // Guard for server-side rendering / Node.js testing
    if (typeof document === 'undefined') return;

    const root = document.documentElement;

    // Apply all 42 standard groups
    for (const group of ALL_GROUPS) {
      const style = this.getStyle(group);
      const cssName = `--theme-${group.replace(/[._]/g, '-')}`;

      root.style.setProperty(`${cssName}-fg`, style.fg);
      if (style.bg) {
        root.style.setProperty(`${cssName}-bg`, style.bg);
      }
    }

    // Apply convenient aliases for common UI elements
    const bg = this.getStyle('background');
    const fg = this.getStyle('foreground');
    const cursor = this.getStyle('cursor');
    const selection = this.getStyle('selection');
    const lineNumber = this.getStyle('line_number');
    const lineNumberActive = this.getStyle('line_number_active');
    const statuslineBg = this.getStyle('statusline_bg');
    const statuslineFg = this.getStyle('statusline_fg');
    const modeNormal = this.getStyle('mode_normal');
    const modeInsert = this.getStyle('mode_insert');
    const modeVisual = this.getStyle('mode_visual');
    const modeCommand = this.getStyle('mode_command');
    const border = this.getStyle('border');
    const popupBg = this.getStyle('popup_bg');
    const popupFg = this.getStyle('popup_fg');
    const menuSelected = this.getStyle('menu_selected');
    const comment = this.getStyle('comment');
    const diagError = this.getStyle('diagnostic.error');

    // Core colors
    root.style.setProperty('--theme-bg', bg.bg ?? DEFAULT_BG);
    root.style.setProperty('--theme-fg', fg.fg);

    // Cursor
    root.style.setProperty('--theme-cursor-fg', cursor.fg);
    root.style.setProperty('--theme-cursor-bg', cursor.bg ?? fg.fg);

    // Selection
    root.style.setProperty('--theme-selection-bg', selection.bg ?? border.fg);

    // Line numbers
    root.style.setProperty('--theme-line-number', lineNumber.fg);
    root.style.setProperty('--theme-line-number-active', lineNumberActive.fg);

    // Statusline
    root.style.setProperty('--theme-statusline-bg', statuslineBg.bg ?? DEFAULT_BG);
    root.style.setProperty('--theme-statusline-fg', statuslineFg.fg);

    // Mode indicators (bg color used for the indicator background)
    root.style.setProperty('--theme-mode-normal', modeNormal.bg ?? '#61afef');
    root.style.setProperty('--theme-mode-insert', modeInsert.bg ?? '#98c379');
    root.style.setProperty('--theme-mode-visual', modeVisual.bg ?? '#c678dd');
    root.style.setProperty('--theme-mode-command', modeCommand.bg ?? '#e5c07b');

    // Borders and separators
    root.style.setProperty('--theme-border', border.fg);
    root.style.setProperty('--theme-accent', modeNormal.bg ?? '#61afef');

    // Popup/overlay
    root.style.setProperty('--theme-popup-bg', popupBg.bg ?? DEFAULT_BG);
    root.style.setProperty('--theme-popup-fg', popupFg.fg);
    root.style.setProperty('--theme-menu-selected-bg', menuSelected.bg ?? border.fg);

    // Text variants
    root.style.setProperty('--theme-text-muted', comment.fg);
    root.style.setProperty('--theme-text-accent', diagError.fg);
  }
}
