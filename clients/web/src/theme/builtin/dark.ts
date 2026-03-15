/**
 * Dark theme (OneDark-inspired).
 *
 * Matches TUI builtin.rs DARK_PALETTE exactly.
 *
 * @module theme/builtin/dark
 */

import type { Theme } from '../types';

/**
 * Dark theme definition.
 *
 * Base colors:
 * - Background: #282c34 (Dark grey-blue)
 * - Foreground: #abb2bf (Light grey)
 */
export const darkTheme: Theme = {
  meta: {
    name: 'dark',
    author: 'reovim',
    version: '1.0.0',
  },

  palette: {
    bg: '#282c34',
    fg: '#abb2bf',
    red: '#e06c75',
    green: '#98c379',
    blue: '#61afef',
    purple: '#c678dd',
    cyan: '#56b6c2',
    yellow: '#e5c07b',
    orange: '#d19a66',
    gray: '#5c6370',
    darkGray: '#4b5263',
    statusBg: '#21252b',
    border: '#3e4451',
    gutterSeparator: '#3c4048',
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
    'comment.doc': { fg: '#6c7380', italic: true, bold: true },
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
    selection: { bg: 'border' },
    line_number: { fg: 'darkGray' },
    line_number_active: { fg: 'fg' },
    statusline_bg: { bg: 'statusBg' },
    statusline_fg: { fg: 'fg' },
    mode_normal: { fg: 'bg', bg: 'blue', bold: true },
    mode_insert: { fg: 'bg', bg: 'green', bold: true },
    mode_visual: { fg: 'bg', bg: 'purple', bold: true },
    mode_command: { fg: 'bg', bg: 'yellow', bold: true },
    border: { fg: 'border' },
    popup_bg: { bg: 'statusBg' },
    popup_fg: { fg: 'fg' },
    menu_selected: { bg: 'border' },
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
    'fold.open': { fg: 'gray' },
    'fold.closed': { fg: 'blue' },
    bookmark: { fg: 'purple' },
  },
};
