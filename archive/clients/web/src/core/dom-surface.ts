/**
 * DOM Chrome Surface
 *
 * Implements the `RenderSurface` interface for DOM-based character
 * grid rendering. Uses a virtual buffer that is flushed to DOM
 * elements via `flush()`.
 *
 * @module core/dom-surface
 */

import type { RenderSurface } from "./contracts.js";
import type { Rect, Style } from "./types.js";

/** A single cell in the virtual buffer. */
interface Cell {
  ch: string;
  style: Style;
}

/** Convert a Style to inline CSS string. Standalone utility, reusable. */
export function styleToCss(style: Style): string {
  const parts: string[] = [];
  if (style.fg) parts.push(`color:${style.fg}`);
  if (style.bg) parts.push(`background-color:${style.bg}`);
  if (style.bold) parts.push("font-weight:bold");
  if (style.italic) parts.push("font-style:italic");
  if (style.underline) parts.push("text-decoration:underline");
  if (style.strikethrough) {
    // Combine with existing underline if present
    if (style.underline) {
      parts.pop(); // remove "text-decoration:underline"
      parts.push("text-decoration:underline line-through");
    } else {
      parts.push("text-decoration:line-through");
    }
  }
  if (style.dim) parts.push("opacity:0.5");
  if (style.reverse) parts.push("filter:invert(1)");
  return parts.join(";");
}

/** Default empty cell. */
function emptyCell(): Cell {
  return { ch: " ", style: {} };
}

/**
 * Get a cell from the buffer. Bounds are checked by the caller;
 * this helper satisfies TypeScript's noUncheckedIndexedAccess.
 */
function getCell(buffer: Cell[][], row: number, col: number): Cell {
  // Bounds validated by caller — row/col are within [0, height) / [0, width)
  return buffer[row]![col]!;
}

/**
 * RenderSurface implementation backed by a virtual character grid.
 *
 * Modules call `writeStyled`, `fill`, `applyStyle`, etc. to populate
 * the virtual buffer. `flush()` converts the buffer to DOM `<span>`
 * elements inside the container.
 */
export class DomChromeSurface implements RenderSurface {
  private readonly width: number;
  private readonly height: number;
  private readonly container: HTMLElement;
  private buffer: Cell[][];

  constructor(container: HTMLElement, rect: Rect) {
    this.container = container;
    this.width = rect.width;
    this.height = rect.height;
    this.buffer = this.createBuffer();
  }

  writeStyled(x: number, y: number, text: string, style: Style): void {
    if (y < 0 || y >= this.height) return;
    for (let i = 0; i < text.length; i++) {
      const cx = x + i;
      if (cx < 0 || cx >= this.width) continue;
      const ch = text[i];
      if (ch !== undefined) {
        this.buffer[y]![cx] = { ch, style };
      }
    }
  }

  applyStyle(rect: Rect, style: Style): void {
    for (let row = rect.y; row < rect.y + rect.height; row++) {
      if (row < 0 || row >= this.height) continue;
      for (let col = rect.x; col < rect.x + rect.width; col++) {
        if (col < 0 || col >= this.width) continue;
        this.buffer[row]![col] = { ch: getCell(this.buffer, row, col).ch, style };
      }
    }
  }

  overlayBg(rect: Rect, color: string): void {
    for (let row = rect.y; row < rect.y + rect.height; row++) {
      if (row < 0 || row >= this.height) continue;
      for (let col = rect.x; col < rect.x + rect.width; col++) {
        if (col < 0 || col >= this.width) continue;
        const cell = getCell(this.buffer, row, col);
        this.buffer[row]![col] = {
          ch: cell.ch,
          style: { ...cell.style, bg: color },
        };
      }
    }
  }

  fill(rect: Rect, ch: string, style: Style): void {
    for (let row = rect.y; row < rect.y + rect.height; row++) {
      if (row < 0 || row >= this.height) continue;
      for (let col = rect.x; col < rect.x + rect.width; col++) {
        if (col < 0 || col >= this.width) continue;
        this.buffer[row]![col] = { ch, style };
      }
    }
  }

  clear(): void {
    this.buffer = this.createBuffer();
  }

  size(): { width: number; height: number } {
    return { width: this.width, height: this.height };
  }

  /** Flush the virtual buffer to DOM. Full rebuild (optimization deferred). */
  flush(): void {
    this.container.innerHTML = "";
    for (let row = 0; row < this.height; row++) {
      const rowDiv = document.createElement("div");
      rowDiv.style.whiteSpace = "pre";
      rowDiv.style.lineHeight = "1";

      // Merge adjacent cells with same style into spans
      let runStart = 0;
      while (runStart < this.width) {
        const startCell = getCell(this.buffer, row, runStart);
        const runStyle = startCell.style;
        let runEnd = runStart + 1;
        while (
          runEnd < this.width &&
          stylesEqual(getCell(this.buffer, row, runEnd).style, runStyle)
        ) {
          runEnd++;
        }

        const text = this.buffer[row]!
          .slice(runStart, runEnd)
          .map((c) => c.ch)
          .join("");
        const css = styleToCss(runStyle);

        if (css) {
          const span = document.createElement("span");
          span.style.cssText = css;
          span.textContent = text;
          rowDiv.appendChild(span);
        } else {
          rowDiv.appendChild(document.createTextNode(text));
        }

        runStart = runEnd;
      }

      this.container.appendChild(rowDiv);
    }
  }

  private createBuffer(): Cell[][] {
    return Array.from({ length: this.height }, () =>
      Array.from({ length: this.width }, () => emptyCell()),
    );
  }
}

/** Compare two styles for equality. */
function stylesEqual(a: Style, b: Style): boolean {
  return (
    a.fg === b.fg &&
    a.bg === b.bg &&
    (a.bold ?? false) === (b.bold ?? false) &&
    (a.italic ?? false) === (b.italic ?? false) &&
    (a.underline ?? false) === (b.underline ?? false) &&
    (a.strikethrough ?? false) === (b.strikethrough ?? false) &&
    (a.dim ?? false) === (b.dim ?? false) &&
    (a.reverse ?? false) === (b.reverse ?? false)
  );
}
