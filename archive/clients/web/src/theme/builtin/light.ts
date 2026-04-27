/**
 * Light theme.
 *
 * Matches TUI builtin.rs LIGHT_PALETTE exactly.
 *
 * @module theme/builtin/light
 */

import type { Theme } from '../types';

/**
 * Light theme definition.
 *
 * Base colors:
 * - Background: #fafafa (Almost white)
 * - Foreground: #383a42 (Dark grey)
 */
export const lightTheme: Theme = {
  meta: {
    name: 'light',
    author: 'reovim',
    version: '1.0.0',
  },

  palette: {
    bg: '#fafafa',
    fg: '#383a42',
    red: '#e45649',
    green: '#50a14f',
    blue: '#4078f2',
    purple: '#a626a4',
    cyan: '#0184bc',
    yellow: '#c18401',
    orange: '#986801',
    gray: '#a0a1a7',
    lightGray: '#d8dee9',
    statusBg: '#e9e9ec',
    border: '#d8dee9',
    gutterSeparator: '#c8c8c8',
  },

  syntax: {
    keyword: { fg: 'purple', bold: true },
    function: { fg: 'blue' },
    type: { fg: 'yellow' },
    string: { fg: 'green' },
    number: { fg: 'orange' },
    comment: { fg: 'gray', italic: true },
    operator: { fg: 'cyan' },
    punctuation: { fg: 'fg' },
    variable: { fg: 'red' },
    constant: { fg: 'orange', bold: true },
    attribute: { fg: 'yellow' },
    property: { fg: 'red' },
    tag: { fg: 'red' },
    // Sub-categories
    'keyword.control': { fg: 'purple', bold: true },
    'keyword.function': { fg: 'purple', bold: true },
    'keyword.type': { fg: 'purple', bold: true },
    'keyword.operator': { fg: 'purple' },
    'type.builtin': { fg: 'yellow', italic: true },
    'function.builtin': { fg: 'blue', bold: true },
    'function.macro': { fg: 'cyan', italic: true },
    'function.method': { fg: 'blue' },
    'variable.builtin': { fg: 'orange' },
    'variable.parameter': { fg: 'red', italic: true },
    'variable.field': { fg: 'red' },
    'string.escape': { fg: 'orange' },
    'comment.doc': { fg: '#828389', italic: true, bold: true },
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
    'markup.link': { fg: 'cyan', underline: true },
    'markup.link.url': { fg: 'cyan' },
    'markup.list': { fg: 'purple' },
    'markup.raw': { fg: 'green' },
    'markup.raw.inline': { fg: 'green' },
    embedded: { fg: 'fg' },
    special: { fg: 'cyan' },
  },

  ui: {
    background: { bg: 'bg' },
    foreground: { fg: 'fg' },
    cursor: { fg: 'bg', bg: 'blue' },
    selection: { bg: 'lightGray' },
    line_number: { fg: 'gray' },
    line_number_active: { fg: 'fg' },
    statusline_bg: { bg: 'statusBg' },
    statusline_fg: { fg: 'fg' },
    mode_normal: { fg: 'bg', bg: 'blue', bold: true },
    mode_insert: { fg: 'bg', bg: 'green', bold: true },
    mode_visual: { fg: 'bg', bg: 'purple', bold: true },
    mode_command: { fg: 'fg', bg: 'yellow', bold: true },
    mode_replace: { fg: 'bg', bg: 'red', bold: true },
    border: { fg: 'border' },
    popup_bg: { bg: 'statusBg' },
    popup_fg: { fg: 'fg' },
    menu_selected: { bg: 'lightGray' },
    search_match: { fg: 'bg', bg: 'yellow' },
  },

  diagnostic: {
    'diagnostic.error': { fg: 'red' },
    'diagnostic.warning': { fg: 'yellow' },
    'diagnostic.info': { fg: 'blue' },
    'diagnostic.hint': { fg: 'cyan' },
  },

  gutter: {
    sign_column: {},
    gutter_separator: { fg: 'gutterSeparator' },
    'git.add': { fg: 'green' },
    'git.change': { fg: 'yellow' },
    'git.delete': { fg: 'red' },
    'fold.open': { fg: '#a0a0a0' },
    'fold.closed': { fg: 'blue' },
    bookmark: { fg: 'purple' },
  },
};
