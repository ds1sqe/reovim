// Buffer rendering - renders lines with selection highlighting.
//
// This module handles rendering buffer content (lines of text) into
// window elements, including cursor positioning and selection highlighting.

import type { ScreenPosition } from "../wasm/index.js";

/**
 * Selection range for highlighting.
 */
export interface SelectionRange {
  /** Anchor point (where selection started). */
  anchor: ScreenPosition;
  /** Cursor point (current cursor position). */
  cursor: ScreenPosition;
  /** Visual selection mode. */
  mode: "char" | "line" | "block";
}

/**
 * Options for the buffer renderer.
 */
export interface BufferRendererOptions {
  /** Number of digits for line numbers. */
  lineNumberWidth: number;
  /** Whether to show line numbers. */
  showLineNumbers: boolean;
  /** Tab width in spaces. */
  tabWidth: number;
}

const DEFAULT_OPTIONS: BufferRendererOptions = {
  lineNumberWidth: 4,
  showLineNumbers: true,
  tabWidth: 4,
};

/**
 * Renders buffer content to a window element.
 */
export class BufferRenderer {
  private options: BufferRendererOptions;

  constructor(options: Partial<BufferRendererOptions> = {}) {
    this.options = { ...DEFAULT_OPTIONS, ...options };
  }

  /**
   * Update renderer options.
   */
  setOptions(options: Partial<BufferRendererOptions>): void {
    this.options = { ...this.options, ...options };
  }

  /**
   * Render buffer lines to a window element.
   *
   * @param lines - Array of line strings to render
   * @param windowEl - The window element (from LayoutRenderer)
   * @param selection - Current selection, if any
   * @param cursor - Current cursor position
   * @param topLine - First visible line index (for scrolling)
   */
  render(
    lines: string[],
    windowEl: HTMLElement,
    selection: SelectionRange | null,
    cursor: ScreenPosition,
    topLine: number = 0
  ): void {
    const bufferEl = windowEl.querySelector(".window-buffer");
    if (!bufferEl) return;

    bufferEl.innerHTML = "";

    // Calculate visible range based on window height
    const windowHeight = parseInt(windowEl.style.height) || 0;
    const lineHeight = 21; // Should match CSS
    const visibleLines = Math.ceil(windowHeight / lineHeight);
    const endLine = Math.min(topLine + visibleLines, lines.length);

    for (let lineNum = topLine; lineNum < endLine; lineNum++) {
      const line = lines[lineNum] ?? "";
      const lineEl = this.createLineElement(
        line,
        lineNum,
        selection,
        cursor.y === lineNum
      );
      bufferEl.appendChild(lineEl);
    }

    // Update cursor position
    this.updateCursor(windowEl, cursor, topLine);
  }

  private createLineElement(
    line: string,
    lineNum: number,
    selection: SelectionRange | null,
    hasCursor: boolean
  ): HTMLElement {
    const lineEl = document.createElement("div");
    lineEl.className = "line" + (hasCursor ? " cursor-line" : "");
    lineEl.dataset.lineNumber = lineNum.toString();

    // Line number
    if (this.options.showLineNumbers) {
      const lineNumEl = document.createElement("span");
      lineNumEl.className = "line-number";
      lineNumEl.textContent = String(lineNum + 1).padStart(
        this.options.lineNumberWidth,
        " "
      );
      lineEl.appendChild(lineNumEl);
    }

    // Line content with selection
    const contentEl = document.createElement("span");
    contentEl.className = "line-content";

    if (selection && this.isLineInSelection(lineNum, selection)) {
      contentEl.innerHTML = this.renderLineWithSelection(
        line,
        lineNum,
        selection
      );
    } else {
      contentEl.textContent = line || " "; // Non-breaking space for empty lines
    }

    lineEl.appendChild(contentEl);
    return lineEl;
  }

  private isLineInSelection(lineNum: number, selection: SelectionRange): boolean {
    const startLine = Math.min(selection.anchor.y, selection.cursor.y);
    const endLine = Math.max(selection.anchor.y, selection.cursor.y);
    return lineNum >= startLine && lineNum <= endLine;
  }

  private renderLineWithSelection(
    line: string,
    lineNum: number,
    selection: SelectionRange
  ): string {
    // Handle empty lines
    if (!line) {
      if (selection.mode === "line") {
        return '<span class="selected"> </span>';
      }
      return " ";
    }

    // Expand tabs for consistent column calculation
    const expandedLine = this.expandTabs(line);
    const chars = [...expandedLine, " "]; // Add trailing space for EOL selection

    return chars
      .map((char, col) => {
        if (this.isSelected(lineNum, col, selection)) {
          return `<span class="selected">${this.escapeHtml(char)}</span>`;
        }
        return this.escapeHtml(char);
      })
      .join("");
  }

  private isSelected(
    line: number,
    col: number,
    selection: SelectionRange
  ): boolean {
    const { anchor, cursor, mode } = selection;
    const startLine = Math.min(anchor.y, cursor.y);
    const endLine = Math.max(anchor.y, cursor.y);

    if (line < startLine || line > endLine) return false;

    switch (mode) {
      case "line":
        return true;

      case "char": {
        // Determine start and end columns based on direction
        let startCol: number;
        let endCol: number;

        if (anchor.y < cursor.y) {
          startCol = anchor.x;
          endCol = cursor.x;
        } else if (anchor.y > cursor.y) {
          startCol = cursor.x;
          endCol = anchor.x;
        } else {
          // Same line
          startCol = Math.min(anchor.x, cursor.x);
          endCol = Math.max(anchor.x, cursor.x);
        }

        if (startLine === endLine) {
          // Single line selection
          return col >= startCol && col <= endCol;
        } else if (line === startLine) {
          // First line - from start column to end
          return col >= (anchor.y <= cursor.y ? startCol : endCol);
        } else if (line === endLine) {
          // Last line - from start to end column
          return col <= (anchor.y <= cursor.y ? endCol : startCol);
        }
        // Middle lines - entire line
        return true;
      }

      case "block": {
        const startCol = Math.min(anchor.x, cursor.x);
        const endCol = Math.max(anchor.x, cursor.x);
        return col >= startCol && col <= endCol;
      }

      default:
        return false;
    }
  }

  private updateCursor(
    windowEl: HTMLElement,
    cursor: ScreenPosition,
    topLine: number
  ): void {
    const cursorEl = windowEl.querySelector(".window-cursor") as HTMLElement;
    if (!cursorEl) return;

    // Calculate cursor position
    const lineNumberOffset = this.options.showLineNumbers
      ? this.options.lineNumberWidth + 1
      : 0;

    const charWidth = 8.4; // Should match CSS
    const lineHeight = 21;

    // Adjust for scroll position
    const screenY = cursor.y - topLine;

    cursorEl.style.left = `${(cursor.x + lineNumberOffset) * charWidth}px`;
    cursorEl.style.top = `${screenY * lineHeight}px`;

    // Hide cursor if out of view
    if (screenY < 0) {
      cursorEl.style.display = "none";
    } else {
      cursorEl.style.display = "";
    }
  }

  private expandTabs(line: string): string {
    const tabWidth = this.options.tabWidth;
    let result = "";
    let col = 0;

    for (const char of line) {
      if (char === "\t") {
        const spaces = tabWidth - (col % tabWidth);
        result += " ".repeat(spaces);
        col += spaces;
      } else {
        result += char;
        col++;
      }
    }

    return result;
  }

  private escapeHtml(text: string): string {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }
}

/**
 * Create a selection range from mode and positions.
 */
export function createSelectionRange(
  anchorLine: number,
  anchorCol: number,
  cursorLine: number,
  cursorCol: number,
  mode: "char" | "line" | "block"
): SelectionRange {
  return {
    anchor: { x: anchorCol, y: anchorLine },
    cursor: { x: cursorCol, y: cursorLine },
    mode,
  };
}
