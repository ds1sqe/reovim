/**
 * DOM Chrome Surface Tests (#651, Phase 2)
 *
 * Verifies the RenderSurface implementation for DOM-based character
 * grid rendering. Requires jsdom environment.
 *
 * @vitest-environment jsdom
 */

import { describe, it, expect, beforeEach } from "vitest";
import { DomChromeSurface, styleToCss } from "../src/core/dom-surface.js";
import { rect } from "../src/core/types.js";
import type { Style } from "../src/core/types.js";

describe("DomChromeSurface", () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement("div");
  });

  it("writeStyled writes text at correct position", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 10, 1));
    surface.writeStyled(2, 0, "hi", {});
    surface.flush();

    // Row 0: "  hi      " (spaces + "hi" + spaces)
    const rowDiv = container.children[0] as HTMLElement;
    expect(rowDiv.textContent).toBe("  hi      ");
  });

  it("fill fills a rect with a character", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 4, 2));
    surface.fill(rect(0, 0, 4, 2), "#", { fg: "red" });
    surface.flush();

    expect(container.children).toHaveLength(2);
    const row0 = container.children[0] as HTMLElement;
    const row1 = container.children[1] as HTMLElement;
    expect(row0.textContent).toBe("####");
    expect(row1.textContent).toBe("####");
  });

  it("clear resets all cells", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 5, 1));
    surface.writeStyled(0, 0, "hello", {});
    surface.clear();
    surface.flush();

    const row = container.children[0] as HTMLElement;
    expect(row.textContent).toBe("     ");
  });

  it("size returns correct dimensions", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 80, 24));
    expect(surface.size()).toEqual({ width: 80, height: 24 });
  });

  it("flush generates DOM spans with correct CSS styles", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 5, 1));
    surface.writeStyled(0, 0, "abc", { fg: "#ff0000", bold: true });
    surface.flush();

    const row = container.children[0] as HTMLElement;
    const span = row.querySelector("span")!;
    expect(span).not.toBeNull();
    expect(span.textContent).toBe("abc");
    // jsdom normalizes hex to rgb
    expect(span.style.color).toBe("rgb(255, 0, 0)");
    expect(span.style.fontWeight).toBe("bold");
  });

  it("style attributes map to correct CSS", () => {
    expect(styleToCss({ bold: true })).toBe("font-weight:bold");
    expect(styleToCss({ italic: true })).toBe("font-style:italic");
    expect(styleToCss({ underline: true })).toBe("text-decoration:underline");
    expect(styleToCss({ strikethrough: true })).toBe(
      "text-decoration:line-through",
    );
    expect(styleToCss({ dim: true })).toBe("opacity:0.5");
    expect(styleToCss({ fg: "blue", bg: "white" })).toBe(
      "color:blue;background-color:white",
    );
    expect(styleToCss({ underline: true, strikethrough: true })).toBe(
      "text-decoration:underline line-through",
    );
  });

  it("out-of-bounds writes are silently ignored", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 3, 1));
    // Write starting at x=2, so "ab" → only "a" fits at x=2
    surface.writeStyled(2, 0, "ab", {});
    surface.flush();

    const row = container.children[0] as HTMLElement;
    expect(row.textContent).toBe("  a");

    // Write at negative y — should not crash
    surface.writeStyled(0, -1, "x", {});
    // Write at y beyond bounds
    surface.writeStyled(0, 5, "x", {});
  });

  it("overlayBg changes background without changing text", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 5, 1));
    surface.writeStyled(0, 0, "hello", { fg: "white" });
    surface.overlayBg(rect(0, 0, 5, 1), "yellow");
    surface.flush();

    const row = container.children[0] as HTMLElement;
    expect(row.textContent).toBe("hello");
    const span = row.querySelector("span")!;
    expect(span.style.backgroundColor).toBe("yellow");
    expect(span.style.color).toBe("white");
  });

  it("multi-character writeStyled spans multiple cells", () => {
    const surface = new DomChromeSurface(container, rect(0, 0, 10, 1));
    surface.writeStyled(0, 0, "hello", { fg: "green" });
    surface.writeStyled(5, 0, "world", { fg: "blue" });
    surface.flush();

    const row = container.children[0] as HTMLElement;
    expect(row.textContent).toBe("helloworld");

    // Should have two styled spans (different styles)
    const spans = row.querySelectorAll("span");
    expect(spans).toHaveLength(2);
    expect(spans[0]!.textContent).toBe("hello");
    expect(spans[1]!.textContent).toBe("world");
  });
});

describe("styleToCss", () => {
  it("empty style returns empty string", () => {
    expect(styleToCss({})).toBe("");
  });

  it("reverse maps to filter invert", () => {
    expect(styleToCss({ reverse: true })).toBe("filter:invert(1)");
  });

  it("combined style produces semicolon-separated CSS", () => {
    const style: Style = {
      fg: "#000",
      bg: "#fff",
      bold: true,
      italic: true,
    };
    const css = styleToCss(style);
    expect(css).toContain("color:#000");
    expect(css).toContain("background-color:#fff");
    expect(css).toContain("font-weight:bold");
    expect(css).toContain("font-style:italic");
  });
});
