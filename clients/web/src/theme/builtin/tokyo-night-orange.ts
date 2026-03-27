/**
 * Tokyo Night Orange theme.
 *
 * Matches TUI builtin.rs TOKYO_NIGHT_ORANGE_PALETTE exactly.
 *
 * @module theme/builtin/tokyo-night-orange
 */

import type { Theme } from '../types';

/**
 * Tokyo Night Orange theme definition.
 *
 * Base colors:
 * - Background: #1a1b26
 * - Foreground: #a9b1d6
 */
export const tokyoNightOrangeTheme: Theme = {
  meta: {
    name: 'tokyo-night-orange',
    author: 'reovim',
    version: '1.0.0',
  },

  palette: {
    bg: '#1a1b26',
    fg: '#a9b1d6',
    red: '#f7768e',
    green: '#9ece6a',
    blue: '#7aa2f7',
    purple: '#bb9af7',
    cyan: '#89ddff',
    teal: '#73daca',
    yellow: '#e0af68',
    orange: '#ff9e64',
    gray: '#565f89',
    darkGray: '#3b4261',
    statusBg: '#16161e',
    selection: '#283457',
    gutterSeparator: '#323442',
    lightCyan: '#7dcfff',
  },

  syntax: {
    keyword: { fg: 'purple', bold: true },
    function: { fg: 'blue' },
    type: { fg: 'orange' },
    string: { fg: 'green' },
    number: { fg: 'orange' },
    comment: { fg: 'gray', italic: true },
    operator: { fg: 'cyan' },
    punctuation: { fg: 'fg' },
    variable: { fg: 'red' },
    constant: { fg: 'orange', bold: true },
    attribute: { fg: 'yellow' },
    property: { fg: 'teal' },
    tag: { fg: 'red' },
    // Sub-categories
    'keyword.control': { fg: 'purple', bold: true },
    'keyword.function': { fg: 'purple', bold: true },
    'keyword.type': { fg: 'purple', bold: true },
    'keyword.operator': { fg: 'purple' },
    'type.builtin': { fg: 'orange', italic: true },
    'function.builtin': { fg: 'blue', bold: true },
    'function.macro': { fg: 'cyan', italic: true },
    'function.method': { fg: 'blue' },
    'variable.builtin': { fg: 'orange' },
    'variable.parameter': { fg: 'red', italic: true },
    'variable.field': { fg: 'teal' },
    'string.escape': { fg: 'orange' },
    'comment.doc': { fg: '#606993', italic: true, bold: true },
    'punctuation.bracket': { fg: 'fg' },
    'punctuation.delimiter': { fg: 'fg' },
    namespace: { fg: 'blue' },
    constructor: { fg: 'yellow', bold: true },
    label: { fg: 'purple' },
    boolean: { fg: 'orange' },
    character: { fg: 'green' },
    'markup.heading': { fg: 'blue', bold: true },
    'markup.bold': { bold: true },
    'markup.italic': { italic: true },
    'markup.strikethrough': { fg: 'gray' },
    'markup.link': { fg: 'lightCyan', underline: true },
    'markup.link.url': { fg: 'lightCyan' },
    'markup.list': { fg: 'purple' },
    'markup.raw': { fg: 'green' },
    'markup.raw.inline': { fg: 'green' },
    embedded: { fg: 'fg' },
    special: { fg: 'cyan' },
  },

  ui: {
    background: { bg: 'bg' },
    foreground: { fg: 'fg' },
    cursor: { fg: 'bg', bg: 'orange' },
    selection: { bg: 'selection' },
    line_number: { fg: 'darkGray' },
    line_number_active: { fg: 'orange' },
    statusline_bg: { bg: 'statusBg' },
    statusline_fg: { fg: 'fg' },
    mode_normal: { fg: 'bg', bg: 'orange', bold: true },
    mode_insert: { fg: 'bg', bg: 'green', bold: true },
    mode_visual: { fg: 'bg', bg: 'purple', bold: true },
    mode_command: { fg: 'bg', bg: 'yellow', bold: true },
    mode_replace: { fg: 'bg', bg: 'red', bold: true },
    border: { fg: 'darkGray' },
    popup_bg: { bg: 'statusBg' },
    popup_fg: { fg: 'fg' },
    menu_selected: { bg: 'selection' },
    search_match: { fg: 'bg', bg: 'orange' },
  },

  diagnostic: {
    'diagnostic.error': { fg: 'red' },
    'diagnostic.warning': { fg: 'yellow' },
    'diagnostic.info': { fg: 'blue' },
    'diagnostic.hint': { fg: 'teal' },
  },

  gutter: {
    sign_column: {},
    gutter_separator: { fg: 'gutterSeparator' },
    'git.add': { fg: 'green' },
    'git.change': { fg: 'orange' },
    'git.delete': { fg: 'red' },
    'fold.open': { fg: 'gray' },
    'fold.closed': { fg: 'lightCyan' },
    bookmark: { fg: 'purple' },
  },
};
