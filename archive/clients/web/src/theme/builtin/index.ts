/**
 * Built-in themes loader.
 *
 * Registers the 3 default themes matching TUI:
 * - dark (OneDark-inspired, default)
 * - light (High-contrast light)
 * - tokyo-night-orange (Tokyo Night variant)
 *
 * @module theme/builtin
 */

import type { ThemeManager } from '../manager';
import { darkTheme } from './dark';
import { lightTheme } from './light';
import { tokyoNightOrangeTheme } from './tokyo-night-orange';

/** All built-in themes. */
export const BUILTIN_THEMES = [
  darkTheme,
  lightTheme,
  tokyoNightOrangeTheme,
] as const;

/** Built-in theme names. */
export const BUILTIN_THEME_NAMES = [
  'dark',
  'light',
  'tokyo-night-orange',
] as const;

export type BuiltinThemeName = (typeof BUILTIN_THEME_NAMES)[number];

/**
 * Register all built-in themes with a ThemeManager.
 *
 * @param manager - ThemeManager to register themes with
 */
export function registerBuiltinThemes(manager: ThemeManager): void {
  for (const theme of BUILTIN_THEMES) {
    manager.registerTheme(theme);
  }
}

/**
 * Create a ThemeManager with all built-in themes registered.
 *
 * Uses 'dark' as the default theme.
 *
 * Note: Import ThemeManager and parseTheme from '../manager' in your code
 * and call this function to initialize with built-in themes.
 *
 * @example
 * ```typescript
 * import { ThemeManager, parseTheme } from './theme/manager';
 * import { darkTheme, lightTheme, tokyoNightOrangeTheme } from './theme/builtin';
 *
 * const manager = new ThemeManager(parseTheme(darkTheme));
 * manager.registerTheme(lightTheme);
 * manager.registerTheme(tokyoNightOrangeTheme);
 * ```
 */
export function createDefaultThemeManager(
  ThemeManagerClass: typeof ThemeManager,
  parseThemeFn: typeof import('../manager').parseTheme
): ThemeManager {
  const defaultProvider = parseThemeFn(darkTheme);
  const manager = new ThemeManagerClass(defaultProvider);

  // Register the other themes
  manager.registerTheme(lightTheme);
  manager.registerTheme(tokyoNightOrangeTheme);

  return manager;
}

// Re-export individual themes
export { darkTheme } from './dark';
export { lightTheme } from './light';
export { tokyoNightOrangeTheme } from './tokyo-night-orange';
