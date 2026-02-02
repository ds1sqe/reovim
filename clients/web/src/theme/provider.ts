/**
 * ThemeProvider interface.
 *
 * Matches the TUI Rust trait for style lookups.
 *
 * @module theme/provider
 */

import type { ResolvedStyle } from './types';

/**
 * Theme provides styles for highlight groups.
 *
 * This interface matches the TUI Rust trait:
 * ```rust
 * pub trait ThemeProvider: Send + Sync {
 *     fn get_style(&self, group: &str) -> Option<Style>;
 *     fn name(&self) -> &str;
 *     fn default_style(&self) -> Style;
 * }
 * ```
 *
 * @example
 * ```typescript
 * class CustomTheme implements ThemeProvider {
 *   getStyle(group: string): ResolvedStyle | null {
 *     if (group === 'keyword') {
 *       return { fg: '#c678dd', bold: true, italic: false, underline: false, strikethrough: false };
 *     }
 *     return null;
 *   }
 *
 *   name(): string {
 *     return 'custom';
 *   }
 * }
 * ```
 */
export interface ThemeProvider {
  /**
   * Get the style for a highlight group by name.
   *
   * Returns null if the group is not defined in this theme.
   *
   * @param group - Highlight group name (e.g., "keyword", "function")
   * @returns Resolved style or null if not found
   */
  getStyle(group: string): ResolvedStyle | null;

  /**
   * Get the theme name.
   *
   * @returns Theme display name
   */
  name(): string;

  /**
   * Get the default/fallback style.
   *
   * Used when no specific style is found for a group.
   * Typically returns the foreground style.
   *
   * @returns Default resolved style
   */
  defaultStyle(): ResolvedStyle;
}
