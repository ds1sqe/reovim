/**
 * Theme module for the web client.
 *
 * Provides theming infrastructure mirroring the TUI implementation:
 * - Theme types and interfaces
 * - 42 highlight group constants
 * - ThemeProvider interface
 * - ThemeManager (4-tier lookup)
 * - StyleGroupRegistry (module defaults)
 * - Built-in themes (Dark, Light, TokyoNightOrange)
 *
 * @module theme
 */

// Types
export type {
  StyleDef,
  ResolvedStyle,
  ThemeMeta,
  Theme,
  ParsedTheme,
} from './types';

export {
  DEFAULT_FG,
  DEFAULT_BG,
  createDefaultStyle,
} from './types';

// Provider interface
export type { ThemeProvider } from './provider';

// Highlight groups
export * as groups from './groups';
export { ALL_GROUPS, SYNTAX_GROUPS, UI_GROUPS } from './groups';

// Theme manager
import { ThemeManager as ThemeManagerClass, parseTheme } from './manager';
export { ThemeManager, parseTheme } from './manager';

// Style group registry (module defaults)
export { StyleGroupRegistry } from './registry';

// Built-in themes
import {
  BUILTIN_THEMES,
  BUILTIN_THEME_NAMES,
  registerBuiltinThemes,
  createDefaultThemeManager as createDefaultThemeManagerInternal,
  darkTheme,
  lightTheme,
  tokyoNightOrangeTheme,
} from './builtin';

export {
  BUILTIN_THEMES,
  BUILTIN_THEME_NAMES,
  registerBuiltinThemes,
  darkTheme,
  lightTheme,
  tokyoNightOrangeTheme,
};
export type { BuiltinThemeName } from './builtin';

/**
 * Create a ThemeManager with all built-in themes registered.
 *
 * This is a convenience wrapper that doesn't require passing
 * the ThemeManager class and parseTheme function.
 *
 * @returns ThemeManager with dark, light, and tokyo-night-orange themes
 *
 * @example
 * ```typescript
 * import { createDefaultThemeManager } from './theme';
 *
 * const manager = createDefaultThemeManager();
 * manager.setTheme('light');  // Switch to light theme
 * manager.applyToCSSVariables();  // Apply to DOM
 * ```
 */
export function createDefaultThemeManager(): ThemeManagerClass {
  return createDefaultThemeManagerInternal(ThemeManagerClass, parseTheme);
}
