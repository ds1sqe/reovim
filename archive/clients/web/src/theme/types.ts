/**
 * Theme types for the web client.
 *
 * These types mirror the TUI Rust implementation but adapted for TypeScript/CSS.
 * Server provides token categories (mechanism), client applies colors (policy).
 *
 * @module theme/types
 */

// =============================================================================
// Style Types
// =============================================================================

/**
 * Style definition in theme files.
 *
 * Colors can reference palette names or be hex values directly.
 *
 * @example
 * ```json
 * { "fg": "purple", "bold": true }
 * { "fg": "#c678dd", "bg": "#282c34" }
 * ```
 */
export interface StyleDef {
  /** Foreground color (palette name or hex) */
  fg?: string;
  /** Background color (palette name or hex) */
  bg?: string;
  /** Bold text */
  bold?: boolean;
  /** Italic text */
  italic?: boolean;
  /** Underline text */
  underline?: boolean;
  /** Strikethrough text */
  strikethrough?: boolean;
  /** Underline color (for colored underlines) */
  underlineColor?: string;
}

/**
 * Resolved style with actual color values.
 *
 * All colors are resolved to hex values.
 */
export interface ResolvedStyle {
  /** Foreground color as hex (#rrggbb) */
  fg: string;
  /** Background color as hex (#rrggbb) or undefined */
  bg?: string;
  /** Bold text */
  bold: boolean;
  /** Italic text */
  italic: boolean;
  /** Underline text */
  underline: boolean;
  /** Strikethrough text */
  strikethrough: boolean;
  /** Underline color as hex */
  underlineColor?: string;
}

// =============================================================================
// Theme Definition
// =============================================================================

/**
 * Theme metadata.
 */
export interface ThemeMeta {
  /** Theme display name */
  name: string;
  /** Theme author */
  author?: string;
  /** Theme version */
  version?: string;
}

/**
 * Complete theme definition.
 *
 * Matches TUI TOML structure but in JSON format.
 * Groups are organized into syntax (13), ui (17), diagnostic (4), gutter (8).
 */
export interface Theme {
  /** Theme metadata */
  meta: ThemeMeta;

  /**
   * Color palette.
   *
   * Named colors that can be referenced in style definitions.
   *
   * @example
   * ```json
   * {
   *   "bg": "#282c34",
   *   "fg": "#abb2bf",
   *   "red": "#e06c75"
   * }
   * ```
   */
  palette: Record<string, string>;

  /**
   * Syntax highlighting styles (13 groups).
   *
   * Maps token categories to styles.
   * Supports hierarchical keys (e.g., "keyword.control").
   */
  syntax: Record<string, StyleDef>;

  /**
   * UI element styles (17 groups).
   *
   * Editor chrome: statusline, line numbers, cursor, etc.
   */
  ui: Record<string, StyleDef>;

  /**
   * Diagnostic styles (4 groups).
   *
   * LSP diagnostics: error, warn, info, hint.
   */
  diagnostic: Record<string, StyleDef>;

  /**
   * Gutter/annotation styles (8 groups).
   *
   * Sign column, git signs, fold markers, bookmarks.
   */
  gutter: Record<string, StyleDef>;
}

// =============================================================================
// Parsed Theme (Internal)
// =============================================================================

/**
 * Parsed theme with resolved styles.
 *
 * Created from Theme by resolving all palette references.
 */
export interface ParsedTheme {
  /** Theme name */
  name: string;

  /** All styles by group name (resolved from palette) */
  styles: Map<string, ResolvedStyle>;

  /** Default style (foreground) */
  defaultStyle: ResolvedStyle;
}

// =============================================================================
// Constants
// =============================================================================

/** Default foreground color (used when not specified) */
export const DEFAULT_FG = '#abb2bf';

/** Default background color (used when not specified) */
export const DEFAULT_BG = '#282c34';

/**
 * Create a default resolved style.
 */
export function createDefaultStyle(): ResolvedStyle {
  return {
    fg: DEFAULT_FG,
    bold: false,
    italic: false,
    underline: false,
    strikethrough: false,
  };
}
