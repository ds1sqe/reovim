/**
 * Highlight group constants.
 *
 * Defines the 42 standard highlight groups used by themes.
 * Matches TUI Rust implementation exactly.
 *
 * Groups are organized into four categories:
 * - Syntax (13 groups): Code elements like keywords, functions, strings
 * - UI (17 groups): Editor chrome like statusline, line numbers, borders
 * - Diagnostic (4 groups): LSP diagnostics like errors and warnings
 * - Gutter (8 groups): Annotation system groups (signs, git, folds)
 *
 * @module theme/groups
 */

// =============================================================================
// Syntax Groups (13 groups)
// =============================================================================

/** Keyword highlight (if, else, fn, let, match, loop, etc.) */
export const KEYWORD = 'keyword';

/** Function names */
export const FUNCTION = 'function';

/** Type names (struct, enum, trait) */
export const TYPE = 'type';

/** String literals ("hello") */
export const STRING = 'string';

/** Numeric literals (42, 3.14, 0xff) */
export const NUMBER = 'number';

/** Comments (line and block) */
export const COMMENT = 'comment';

/** Operators (+, -, *, /, =, ==) */
export const OPERATOR = 'operator';

/** Punctuation ((), {}, [], ;, :) */
export const PUNCTUATION = 'punctuation';

/** Variable names */
export const VARIABLE = 'variable';

/** Constants (CONSTANT_NAME) */
export const CONSTANT = 'constant';

/** Attributes (#[derive], @decorator) */
export const ATTRIBUTE = 'attribute';

/** Object/struct properties */
export const PROPERTY = 'property';

/** HTML/XML tags */
export const TAG = 'tag';

// =============================================================================
// UI Groups (17 groups)
// =============================================================================

/** Default background color */
export const BACKGROUND = 'background';

/** Default foreground color */
export const FOREGROUND = 'foreground';

/** Cursor color */
export const CURSOR = 'cursor';

/** Visual selection highlight */
export const SELECTION = 'selection';

/** Line number column */
export const LINE_NUMBER = 'line_number';

/** Active/cursor line number */
export const LINE_NUMBER_ACTIVE = 'line_number_active';

/** Statusline background */
export const STATUSLINE_BG = 'statusline_bg';

/** Statusline foreground */
export const STATUSLINE_FG = 'statusline_fg';

/** Normal mode indicator */
export const MODE_NORMAL = 'mode_normal';

/** Insert mode indicator */
export const MODE_INSERT = 'mode_insert';

/** Visual mode indicator */
export const MODE_VISUAL = 'mode_visual';

/** Command mode indicator */
export const MODE_COMMAND = 'mode_command';

/** UI borders */
export const BORDER = 'border';

/** Popup/floating window background */
export const POPUP_BG = 'popup_bg';

/** Popup/floating window foreground */
export const POPUP_FG = 'popup_fg';

/** Menu selected item */
export const MENU_SELECTED = 'menu_selected';

/** Search match highlight */
export const SEARCH_MATCH = 'search_match';

// =============================================================================
// Diagnostic Groups (4 groups)
// =============================================================================

/** Error diagnostic */
export const DIAGNOSTIC_ERROR = 'diagnostic.error';

/** Warning diagnostic */
export const DIAGNOSTIC_WARN = 'diagnostic.warn';

/** Info diagnostic */
export const DIAGNOSTIC_INFO = 'diagnostic.info';

/** Hint diagnostic */
export const DIAGNOSTIC_HINT = 'diagnostic.hint';

// =============================================================================
// Gutter/Annotation Groups (8 groups)
// =============================================================================

/** Sign column background */
export const SIGN_COLUMN = 'sign_column';

/** Separator between gutter and content area */
export const GUTTER_SEPARATOR = 'gutter_separator';

/** Git added line indicator */
export const GIT_ADD = 'git.add';

/** Git changed line indicator */
export const GIT_CHANGE = 'git.change';

/** Git deleted line indicator */
export const GIT_DELETE = 'git.delete';

/** Fold marker - region is open/expanded */
export const FOLD_OPEN = 'fold.open';

/** Fold marker - region is closed/collapsed */
export const FOLD_CLOSED = 'fold.closed';

/** Bookmark indicator */
export const BOOKMARK = 'bookmark';

// =============================================================================
// All Groups Array (for testing completeness)
// =============================================================================

/**
 * All built-in highlight groups (42 total).
 *
 * Used by tests to verify theme completeness and for CSS variable generation.
 */
export const ALL_GROUPS: readonly string[] = [
  // Syntax (13)
  KEYWORD,
  FUNCTION,
  TYPE,
  STRING,
  NUMBER,
  COMMENT,
  OPERATOR,
  PUNCTUATION,
  VARIABLE,
  CONSTANT,
  ATTRIBUTE,
  PROPERTY,
  TAG,
  // UI (17)
  BACKGROUND,
  FOREGROUND,
  CURSOR,
  SELECTION,
  LINE_NUMBER,
  LINE_NUMBER_ACTIVE,
  STATUSLINE_BG,
  STATUSLINE_FG,
  MODE_NORMAL,
  MODE_INSERT,
  MODE_VISUAL,
  MODE_COMMAND,
  BORDER,
  POPUP_BG,
  POPUP_FG,
  MENU_SELECTED,
  SEARCH_MATCH,
  // Diagnostic (4)
  DIAGNOSTIC_ERROR,
  DIAGNOSTIC_WARN,
  DIAGNOSTIC_INFO,
  DIAGNOSTIC_HINT,
  // Gutter/Annotation (8)
  SIGN_COLUMN,
  GUTTER_SEPARATOR,
  GIT_ADD,
  GIT_CHANGE,
  GIT_DELETE,
  FOLD_OPEN,
  FOLD_CLOSED,
  BOOKMARK,
] as const;

/**
 * Syntax groups only (13).
 *
 * Used for token category → style lookups.
 */
export const SYNTAX_GROUPS: readonly string[] = [
  KEYWORD,
  FUNCTION,
  TYPE,
  STRING,
  NUMBER,
  COMMENT,
  OPERATOR,
  PUNCTUATION,
  VARIABLE,
  CONSTANT,
  ATTRIBUTE,
  PROPERTY,
  TAG,
] as const;

/**
 * UI groups only (17).
 */
export const UI_GROUPS: readonly string[] = [
  BACKGROUND,
  FOREGROUND,
  CURSOR,
  SELECTION,
  LINE_NUMBER,
  LINE_NUMBER_ACTIVE,
  STATUSLINE_BG,
  STATUSLINE_FG,
  MODE_NORMAL,
  MODE_INSERT,
  MODE_VISUAL,
  MODE_COMMAND,
  BORDER,
  POPUP_BG,
  POPUP_FG,
  MENU_SELECTED,
  SEARCH_MATCH,
] as const;
