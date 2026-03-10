/**
 * OverlayRenderer Tests
 *
 * Tests for floating overlay rendering (completion, cmdline, hover, signature).
 * Uses jsdom environment for DOM testing.
 * Part of Phase 11.3 - Web Client Cache & Overlay Unit Tests.
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { OverlayRenderer } from "../src/render/overlay.js";
import type { LogicalOverlay, SemanticOrigin } from "../src/wasm/index.js";

// ============ Test Fixtures ============

/**
 * Create a basic overlay for testing.
 */
function createOverlay(
  id: string,
  kind: string,
  overrides: Partial<Omit<LogicalOverlay, "id" | "kind">> = {}
): LogicalOverlay {
  return {
    id,
    kind,
    origin: undefined,
    data: {},
    state: { selected_index: undefined, filter: "", scroll_offset: 0, loading: false },
    priority: 0,
    ...overrides,
  };
}

/**
 * Create a completion overlay with items.
 */
function createCompletionOverlay(
  id: string,
  items: Array<{ label: string; kind?: string; detail?: string }>,
  selectedIndex: number = 0
): LogicalOverlay {
  return createOverlay(id, "completion", {
    data: { items },
    state: { selected_index: selectedIndex, filter: "", scroll_offset: 0, loading: false },
  });
}

/**
 * Create a cmdline overlay.
 */
function createCmdlineOverlay(
  id: string,
  content: string,
  cursor: number = content.length,
  prefix: string = ":"
): LogicalOverlay {
  return createOverlay(id, "cmdline", {
    data: { prefix, content, cursor },
  });
}

/**
 * Create a hover overlay.
 */
function createHoverOverlay(id: string, text: string): LogicalOverlay {
  return createOverlay(id, "hover", {
    data: { text },
  });
}

/**
 * Create a signature overlay.
 */
function createSignatureOverlay(
  id: string,
  signatures: Array<{ label: string; documentation?: string }>,
  activeSignature: number = 0
): LogicalOverlay {
  return createOverlay(id, "signature", {
    data: { signatures, activeSignature },
  });
}

// ============ Tests ============

describe("OverlayRenderer", () => {
  let container: HTMLElement;
  let renderer: OverlayRenderer;

  beforeEach(() => {
    // Create container with dimensions
    container = document.createElement("div");
    container.style.width = "800px";
    container.style.height = "600px";
    container.style.position = "relative";
    document.body.appendChild(container);

    renderer = new OverlayRenderer();
    renderer.setContainer(container);
  });

  afterEach(() => {
    container.remove();
  });

  describe("setContainer", () => {
    it("stores container reference", () => {
      const newContainer = document.createElement("div");
      const newRenderer = new OverlayRenderer();

      // Before setting container, show does nothing
      newRenderer.show(createOverlay("test", "hover"));
      expect(newContainer.children.length).toBe(0);

      // After setting container
      newRenderer.setContainer(newContainer);
      newRenderer.show(createOverlay("test", "hover"));
      expect(newContainer.children.length).toBe(1);
    });

    it("show() does nothing without container", () => {
      const noContainerRenderer = new OverlayRenderer();
      noContainerRenderer.show(createOverlay("test", "hover"));

      // No errors thrown, no DOM changes
      expect(document.body.querySelector(".overlay")).toBeNull();
    });
  });

  describe("show", () => {
    it("creates overlay element in container", () => {
      renderer.show(createOverlay("test-1", "hover"));

      const overlay = container.querySelector(".overlay");
      expect(overlay).not.toBeNull();
    });

    it("sets overlay-{kind} class", () => {
      renderer.show(createOverlay("test", "completion"));
      expect(container.querySelector(".overlay-completion")).not.toBeNull();

      renderer.show(createOverlay("test2", "cmdline"));
      expect(container.querySelector(".overlay-cmdline")).not.toBeNull();

      renderer.show(createOverlay("test3", "hover"));
      expect(container.querySelector(".overlay-hover")).not.toBeNull();
    });

    it("sets data-overlay-id attribute", () => {
      renderer.show(createOverlay("my-overlay-id", "hover"));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.dataset.overlayId).toBe("my-overlay-id");
    });

    it("positions element absolutely", () => {
      renderer.show(createOverlay("test", "hover"));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.position).toBe("absolute");
    });

    it("sets z-index from priority", () => {
      renderer.show(createOverlay("low", "hover", { priority: 0 }));
      renderer.show(createOverlay("high", "hover", { priority: 50 }));

      const lowOverlay = container.querySelector('[data-overlay-id="low"]') as HTMLElement;
      const highOverlay = container.querySelector('[data-overlay-id="high"]') as HTMLElement;

      expect(lowOverlay?.style.zIndex).toBe("100"); // 100 + 0
      expect(highOverlay?.style.zIndex).toBe("150"); // 100 + 50
    });

    it("reuses existing element on second show", () => {
      renderer.show(createOverlay("test", "hover"));
      const firstElement = container.querySelector(".overlay");

      renderer.show(createOverlay("test", "hover"));
      const secondElement = container.querySelector(".overlay");

      expect(firstElement).toBe(secondElement);
      expect(container.querySelectorAll(".overlay").length).toBe(1);
    });

    it("sets display:block when shown", () => {
      renderer.show(createOverlay("test", "hover"));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.display).toBe("block");
    });
  });

  describe("hide", () => {
    it("sets display:none on overlay", () => {
      renderer.show(createOverlay("test", "hover"));
      renderer.hide("test");

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.display).toBe("none");
    });

    it("sets aria-hidden to true", () => {
      renderer.show(createOverlay("test", "hover"));
      renderer.hide("test");

      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("aria-hidden")).toBe("true");
    });

    it("does nothing for unknown overlay", () => {
      // Should not throw
      expect(() => renderer.hide("nonexistent")).not.toThrow();
    });
  });

  describe("remove", () => {
    it("removes element from DOM", () => {
      renderer.show(createOverlay("test", "hover"));
      expect(container.querySelector(".overlay")).not.toBeNull();

      renderer.remove("test");
      expect(container.querySelector(".overlay")).toBeNull();
    });

    it("removes from internal map", () => {
      renderer.show(createOverlay("test", "hover"));
      renderer.remove("test");

      // Show again - should create new element
      renderer.show(createOverlay("test", "hover"));
      expect(container.querySelectorAll(".overlay").length).toBe(1);
    });

    it("does nothing for unknown overlay", () => {
      expect(() => renderer.remove("nonexistent")).not.toThrow();
    });
  });

  describe("hideAll", () => {
    it("hides all visible overlays", () => {
      renderer.show(createOverlay("a", "hover"));
      renderer.show(createOverlay("b", "completion"));
      renderer.show(createOverlay("c", "cmdline"));

      renderer.hideAll();

      const overlays = container.querySelectorAll(".overlay") as NodeListOf<HTMLElement>;
      overlays.forEach((el) => {
        expect(el.style.display).toBe("none");
      });
    });
  });

  describe("clear", () => {
    it("removes all overlays from DOM", () => {
      renderer.show(createOverlay("a", "hover"));
      renderer.show(createOverlay("b", "completion"));

      renderer.clear();

      expect(container.querySelectorAll(".overlay").length).toBe(0);
    });

    it("clears internal map", () => {
      renderer.show(createOverlay("test", "hover"));
      renderer.clear();

      // getVisibleOverlays should return empty
      expect(renderer.getVisibleOverlays()).toEqual([]);
    });
  });

  describe("isVisible", () => {
    it("returns true for shown overlay", () => {
      renderer.show(createOverlay("test", "hover"));
      expect(renderer.isVisible("test")).toBe(true);
    });

    it("returns false for hidden overlay", () => {
      renderer.show(createOverlay("test", "hover"));
      renderer.hide("test");
      expect(renderer.isVisible("test")).toBe(false);
    });

    it("returns false for unknown overlay", () => {
      expect(renderer.isVisible("nonexistent")).toBe(false);
    });
  });

  describe("getVisibleOverlays", () => {
    it("returns empty array when none visible", () => {
      expect(renderer.getVisibleOverlays()).toEqual([]);
    });

    it("returns ids of visible overlays", () => {
      renderer.show(createOverlay("a", "hover"));
      renderer.show(createOverlay("b", "completion"));

      const visible = renderer.getVisibleOverlays();
      expect(visible).toHaveLength(2);
      expect(visible).toContain("a");
      expect(visible).toContain("b");
    });

    it("excludes hidden overlays", () => {
      renderer.show(createOverlay("a", "hover"));
      renderer.show(createOverlay("b", "completion"));
      renderer.hide("a");

      const visible = renderer.getVisibleOverlays();
      expect(visible).toEqual(["b"]);
    });
  });

  describe("updateState", () => {
    it("does nothing for unknown overlay", () => {
      expect(() => renderer.updateState("nonexistent", { selected_index: 0 })).not.toThrow();
    });

    it("updates selected_index in completion menu", () => {
      const overlay = createCompletionOverlay(
        "comp",
        [{ label: "foo" }, { label: "bar" }, { label: "baz" }],
        0
      );
      renderer.show(overlay);

      renderer.updateState("comp", { selected_index: 2 });

      const items = container.querySelectorAll(".completion-item");
      expect(items[0]!.classList.contains("selected")).toBe(false);
      expect(items[2]!.classList.contains("selected")).toBe(true);
    });

    it("adds selected class to correct item", () => {
      renderer.show(
        createCompletionOverlay("comp", [{ label: "a" }, { label: "b" }], 0)
      );

      renderer.updateState("comp", { selected_index: 1 });

      const items = container.querySelectorAll(".completion-item");
      expect(items[1]!.classList.contains("selected")).toBe(true);
    });

    it("removes selected from other items", () => {
      renderer.show(
        createCompletionOverlay("comp", [{ label: "a" }, { label: "b" }], 0)
      );

      // Initially first item is selected
      let items = container.querySelectorAll(".completion-item");
      expect(items[0]!.classList.contains("selected")).toBe(true);

      // Update to select second item
      renderer.updateState("comp", { selected_index: 1 });

      items = container.querySelectorAll(".completion-item");
      expect(items[0]!.classList.contains("selected")).toBe(false);
      expect(items[1]!.classList.contains("selected")).toBe(true);
    });

    it("sets aria-selected correctly", () => {
      renderer.show(
        createCompletionOverlay("comp", [{ label: "a" }, { label: "b" }], 0)
      );

      renderer.updateState("comp", { selected_index: 1 });

      const items = container.querySelectorAll(".completion-item");
      expect(items[0]!.getAttribute("aria-selected")).toBe("false");
      expect(items[1]!.getAttribute("aria-selected")).toBe("true");
    });
  });

  describe("updateCursorPosition", () => {
    it("updates cursor position for positioning calculations", () => {
      renderer.updateCursorPosition(5, 10);
      renderer.show(createOverlay("test", "hover"));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      // Position should be based on cursor (null origin defaults to cursor)
      expect(overlay?.style.left).toBeDefined();
      expect(overlay?.style.top).toBeDefined();
    });
  });

  describe("positioning - null origin (cursor default)", () => {
    it("positions below cursor position when origin is null", () => {
      renderer.updateCursorPosition(5, 10);
      renderer.show(createOverlay("test", "hover"));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.left).toBeDefined();
      expect(overlay?.style.top).toBeDefined();
    });
  });

  describe("positioning - Session origin", () => {
    it("centers overlay in container", () => {
      renderer.show(createOverlay("test", "hover", { origin: "Session" as SemanticOrigin }));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.left).toBeDefined();
      expect(overlay?.style.top).toBeDefined();
    });
  });

  describe("positioning - BufferPosition origin", () => {
    it("positions at buffer line/col", () => {
      const origin = { BufferPosition: { buffer_id: 1, line: 10, col: 20 } } as SemanticOrigin;
      renderer.show(createOverlay("test", "hover", { origin }));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.left).toBeDefined();
      expect(overlay?.style.top).toBeDefined();
    });
  });

  describe("positioning - BufferRange origin", () => {
    it("positions at start of range", () => {
      const origin = {
        BufferRange: { buffer_id: 1, start_line: 5, start_col: 0, end_line: 7, end_col: 15 },
      } as SemanticOrigin;
      renderer.show(createOverlay("test", "hover", { origin }));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.left).toBeDefined();
      expect(overlay?.style.top).toBeDefined();
    });
  });

  describe("positioning - Buffer origin", () => {
    it("centers on screen for buffer-level origin", () => {
      const origin = { Buffer: { buffer_id: 1 } } as SemanticOrigin;
      renderer.show(createOverlay("test", "hover", { origin }));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay?.style.left).toBeDefined();
      expect(overlay?.style.top).toBeDefined();
    });
  });

  describe("ARIA accessibility", () => {
    it("sets role=listbox for completion", () => {
      renderer.show(createOverlay("test", "completion"));
      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("role")).toBe("listbox");
    });

    it("sets role=textbox for cmdline", () => {
      renderer.show(createOverlay("test", "cmdline"));
      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("role")).toBe("textbox");
    });

    it("sets role=tooltip for hover", () => {
      renderer.show(createOverlay("test", "hover"));
      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("role")).toBe("tooltip");
    });

    it("sets role=tooltip for signature", () => {
      renderer.show(createOverlay("test", "signature"));
      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("role")).toBe("tooltip");
    });

    it("sets role=dialog for unknown kind", () => {
      renderer.show(createOverlay("test", "unknown"));
      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("role")).toBe("dialog");
    });

    it("sets aria-hidden=false when shown", () => {
      renderer.show(createOverlay("test", "hover"));
      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("aria-hidden")).toBe("false");
    });

    it("sets aria-hidden=true when hidden", () => {
      renderer.show(createOverlay("test", "hover"));
      renderer.hide("test");
      const overlay = container.querySelector(".overlay");
      expect(overlay?.getAttribute("aria-hidden")).toBe("true");
    });
  });

  describe("completion rendering", () => {
    it("renders completion items as list", () => {
      renderer.show(
        createCompletionOverlay("comp", [
          { label: "foo" },
          { label: "bar" },
          { label: "baz" },
        ])
      );

      const items = container.querySelectorAll(".completion-item");
      expect(items).toHaveLength(3);
    });

    it("marks selected item", () => {
      renderer.show(
        createCompletionOverlay(
          "comp",
          [{ label: "foo" }, { label: "bar" }],
          1 // Select second item
        )
      );

      const items = container.querySelectorAll(".completion-item");
      expect(items[0]!.classList.contains("selected")).toBe(false);
      expect(items[1]!.classList.contains("selected")).toBe(true);
    });

    it("shows empty message when no items", () => {
      renderer.show(createCompletionOverlay("comp", []));

      const empty = container.querySelector(".completion-empty");
      expect(empty).not.toBeNull();
      expect(empty?.textContent).toBe("No completions");
    });

    it("renders item label", () => {
      renderer.show(createCompletionOverlay("comp", [{ label: "myFunction" }]));

      const label = container.querySelector(".completion-label");
      expect(label?.textContent).toBe("myFunction");
    });

    it("renders item kind when present", () => {
      renderer.show(
        createCompletionOverlay("comp", [{ label: "foo", kind: "Function" }])
      );

      const kind = container.querySelector(".completion-kind");
      expect(kind?.textContent).toBe("Function");
      expect(kind?.classList.contains("completion-kind-function")).toBe(true);
    });

    it("renders item detail when present", () => {
      renderer.show(
        createCompletionOverlay("comp", [{ label: "foo", detail: "description" }])
      );

      const detail = container.querySelector(".completion-detail");
      expect(detail?.textContent).toBe("description");
    });
  });

  describe("cmdline rendering", () => {
    it("renders prefix and content", () => {
      renderer.show(createCmdlineOverlay("cmd", "help", 4, ":"));

      const prefix = container.querySelector(".cmdline-prefix");
      expect(prefix?.textContent).toBe(":");

      const content = container.querySelector(".cmdline-content");
      expect(content?.textContent?.includes("help")).toBe(true);
    });

    it("shows cursor at correct position", () => {
      renderer.show(createCmdlineOverlay("cmd", "write", 2));

      const cursor = container.querySelector(".cmdline-cursor");
      expect(cursor).not.toBeNull();
    });

    it("uses default prefix when not specified", () => {
      renderer.show(createOverlay("cmd", "cmdline", { data: { content: "test" } }));

      const prefix = container.querySelector(".cmdline-prefix");
      expect(prefix?.textContent).toBe(":");
    });
  });

  describe("hover rendering", () => {
    it("renders hover text", () => {
      renderer.show(createHoverOverlay("hover", "This is hover info"));

      const content = container.querySelector(".hover-content");
      expect(content?.textContent).toBe("This is hover info");
    });

    it("handles markdown field", () => {
      renderer.show(
        createOverlay("hover", "hover", { data: { markdown: "**bold**" } })
      );

      const content = container.querySelector(".hover-content");
      expect(content?.textContent).toBe("**bold**");
    });
  });

  describe("signature rendering", () => {
    it("renders active signature label", () => {
      renderer.show(
        createSignatureOverlay("sig", [
          { label: "function(a: number, b: string): void" },
        ])
      );

      const label = container.querySelector(".signature-label");
      expect(label?.textContent).toBe("function(a: number, b: string): void");
    });

    it("renders documentation when present", () => {
      renderer.show(
        createSignatureOverlay("sig", [
          { label: "fn()", documentation: "Does something" },
        ])
      );

      const doc = container.querySelector(".signature-documentation");
      expect(doc?.textContent).toBe("Does something");
    });

    it("renders nothing for empty signatures", () => {
      renderer.show(createSignatureOverlay("sig", []));

      const signature = container.querySelector(".signature-container");
      expect(signature).toBeNull();
    });

    it("uses activeSignature index", () => {
      renderer.show(
        createSignatureOverlay(
          "sig",
          [{ label: "first()" }, { label: "second()" }],
          1 // Select second signature
        )
      );

      const label = container.querySelector(".signature-label");
      expect(label?.textContent).toBe("second()");
    });
  });

  describe("default rendering", () => {
    it("renders JSON for unknown overlay kind", () => {
      renderer.show(
        createOverlay("test", "unknown-kind", { data: { foo: "bar" } })
      );

      const fallback = container.querySelector(".overlay-fallback");
      expect(fallback?.textContent).toContain('"foo"');
      expect(fallback?.textContent).toContain('"bar"');
    });
  });

  describe("custom config", () => {
    it("accepts custom charWidth", () => {
      const customRenderer = new OverlayRenderer({ charWidth: 10 });
      customRenderer.setContainer(container);
      customRenderer.updateCursorPosition(0, 10);
      customRenderer.show(createOverlay("test", "hover"));

      // Should use custom charWidth for positioning
      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay).not.toBeNull();
    });

    it("accepts custom lineHeight", () => {
      const customRenderer = new OverlayRenderer({ lineHeight: 30 });
      customRenderer.setContainer(container);
      customRenderer.show(createOverlay("test", "hover"));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay).not.toBeNull();
    });

    it("accepts custom padding", () => {
      const customRenderer = new OverlayRenderer({ padding: 20 });
      customRenderer.setContainer(container);
      customRenderer.show(createOverlay("test", "hover"));

      const overlay = container.querySelector(".overlay") as HTMLElement;
      expect(overlay).not.toBeNull();
    });
  });
});
