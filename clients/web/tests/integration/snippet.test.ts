/**
 * Snippet Feature Integration Tests (#136)
 *
 * Tests verify the snippet workflow through the web client gRPC stack:
 * type trigger prefix -> `<C-s>` expand -> Tab/S-Tab navigate -> Esc cancel.
 *
 * Requires `~/.local/share/reovim/modules/snippets/global.json` with test snippets.
 *
 * Running:
 * ```bash
 * npm run test:integration -- --testPathPattern snippet
 * ```
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { WebIntegrationTest, frameAssertions } from "../helpers/integration.js";
import type { HeadlessWebClient } from "../../src/headless/index.js";

describe("Snippet Feature (#136)", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  // ==========================================================================
  // Basic Expansion
  // ==========================================================================

  describe("expansion", () => {
    it("expands simple snippet with <C-s>", async () => {
      await client.sendKeys("itst<C-s>");

      const frame = await client.waitFor(
        (f) => f.includes("TEST_EXPANDED"),
        3000,
      );

      expect(frame).toContain("TEST_EXPANDED");
    });

    it("does nothing for unknown prefix", async () => {
      await client.sendKeys("ixyz<C-s>");

      // Small delay for processing
      await new Promise((r) => setTimeout(r, 300));

      const buffer = client.getBuffer();
      expect(buffer).toContain("xyz");
      expect(buffer).not.toContain("TEST_EXPANDED");
    });

    it("expands function snippet template", async () => {
      await client.sendKeys("ifn<C-s>");

      const frame = await client.waitFor(
        (f) => f.includes("fn ") && f.includes("{"),
        3000,
      );

      expect(frame).toContain("fn ");
      // Phase 2: $1 placeholder "name" is selected (not deleted) on expansion
      expect(frame).toContain("name");
      // $2 placeholder "params" remains (not yet navigated to)
      expect(frame).toContain("params");
      expect(frame).toContain("{");
      expect(frame).toContain("}");
    });

    it("expands multi-line for loop snippet", async () => {
      await client.sendKeys("ifor<C-s>");

      const frame = await client.waitFor(
        (f) => f.includes("for ") && f.includes("in "),
        3000,
      );

      expect(frame).toContain("for ");
      expect(frame).toContain("in ");
      expect(frame).toContain("{");
      expect(frame).toContain("}");
    });
  });

  // ==========================================================================
  // Mode Transitions
  // ==========================================================================

  describe("mode transitions", () => {
    it("enters NAVIGATING mode after expansion", async () => {
      await client.sendKeys("ifn<C-s>");

      const frame = await client.waitFor(
        (f) => {
          const lower = f.toLowerCase();
          return lower.includes("navigating") || lower.includes("snippet");
        },
        3000,
      );

      const mode = frameAssertions.getMode(frame);
      expect(mode?.toLowerCase()).toMatch(/navigating|snippet/);
    });

    it("returns to INSERT mode after Escape", async () => {
      await client.sendKeys("ifn<C-s>");

      // Wait for expansion
      await client.waitFor((f) => f.includes("fn "), 3000);

      // Cancel snippet
      await client.sendKeys("<Esc>");

      const frame = await client.waitFor(
        (f) => f.toLowerCase().includes("insert"),
        3000,
      );

      const mode = frameAssertions.getMode(frame);
      expect(mode?.toUpperCase()).toContain("INSERT");
    });
  });

  // ==========================================================================
  // Tab Stop Navigation
  // ==========================================================================

  describe("tab navigation", () => {
    it("Tab advances to next tab stop", async () => {
      await client.sendKeys("ifn<C-s>");

      // Wait for expansion
      await client.waitFor((f) => f.includes("fn "), 3000);

      // Capture cursor at $1 (name)
      const cursorBefore = client.getCursor();

      // Tab to $2 (params)
      await client.sendKeys("<Tab>");
      await new Promise((r) => setTimeout(r, 200));

      const cursorAfter = client.getCursor();

      // Cursor should have moved (col increased since params is after name)
      expect(cursorAfter.col).toBeGreaterThan(cursorBefore.col);

      // Template should still be intact
      const buffer = client.getBuffer();
      expect(buffer).toContain("fn ");
    });

    it("S-Tab navigates backward", async () => {
      await client.sendKeys("ifn<C-s>");
      await client.waitFor((f) => f.includes("fn "), 3000);

      // Tab forward to $2
      await client.sendKeys("<Tab>");
      await new Promise((r) => setTimeout(r, 200));

      const cursorAt2 = client.getCursor();

      // S-Tab back to $1
      await client.sendKeys("<S-Tab>");
      await new Promise((r) => setTimeout(r, 200));

      const cursorBack = client.getCursor();

      // Cursor should have moved back (col decreased)
      expect(cursorBack.col).toBeLessThan(cursorAt2.col);
    });

    it("navigates through all stops", async () => {
      await client.sendKeys("ifn<C-s>");
      await client.waitFor((f) => f.includes("fn "), 3000);

      // $1 -> $2 -> $0 (final)
      await client.sendKeys("<Tab>");
      await new Promise((r) => setTimeout(r, 100));
      await client.sendKeys("<Tab>");
      await new Promise((r) => setTimeout(r, 100));

      // After reaching $0, snippet may auto-finalize or stay in navigating
      const buffer = client.getBuffer();
      expect(buffer).toContain("fn ");
      expect(buffer).toContain("{");
      expect(buffer).toContain("}");
    });
  });

  // ==========================================================================
  // Full Workflow
  // ==========================================================================

  describe("full workflow", () => {
    it("expand -> navigate -> cancel", async () => {
      // Step 1: Expand
      await client.sendKeys("ifn<C-s>");
      await client.waitFor((f) => f.includes("fn "), 3000);

      // Step 2: Navigate forward
      await client.sendKeys("<Tab>");
      await new Promise((r) => setTimeout(r, 200));

      // Step 3: Navigate backward
      await client.sendKeys("<S-Tab>");
      await new Promise((r) => setTimeout(r, 200));

      // Step 4: Cancel
      await client.sendKeys("<Esc>");
      const frame = await client.waitFor(
        (f) => f.toLowerCase().includes("insert"),
        3000,
      );

      // Verify final state
      const mode = frameAssertions.getMode(frame);
      expect(mode?.toUpperCase()).toContain("INSERT");

      const buffer = client.getBuffer();
      expect(buffer).toContain("fn ");
    });

    it("expand -> type in placeholder -> tab -> escape", async () => {
      // Expand "fn" snippet
      await client.sendKeys("ifn<C-s>");
      await client.waitFor((f) => f.includes("fn "), 3000);

      // Type in first placeholder ($1 = name)
      await client.sendKeys("my_func");
      await new Promise((r) => setTimeout(r, 200));

      // Verify typed text appears
      const bufferAfterType = client.getBuffer();
      expect(bufferAfterType).toContain("my_func");

      // Tab to next stop
      await client.sendKeys("<Tab>");
      await new Promise((r) => setTimeout(r, 200));

      // Cancel snippet
      await client.sendKeys("<Esc>");

      await client.waitFor(
        (f) => f.toLowerCase().includes("insert"),
        3000,
      );

      // Final buffer should have the typed function name
      const finalBuffer = client.getBuffer();
      expect(finalBuffer).toContain("my_func");
    });
  });
});
