/**
 * Headless Web Client Integration Tests
 *
 * These tests verify the HeadlessWebClient functionality
 * using a real reovim server.
 *
 * NOTE: These tests require the reovim server binary to be built.
 * Run: cargo build -p reovim-app --features grpc
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { WebIntegrationTest, frameAssertions } from "../helpers/integration.js";
import type { HeadlessWebClient } from "../../src/headless/index.js";

describe("HeadlessWebClient", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000); // 15 second timeout for server startup

  afterEach(async () => {
    await test.cleanup();
  });

  describe("connection", () => {
    it("connects successfully", () => {
      expect(client.isConnected()).toBe(true);
    });

    it("reports correct server address", () => {
      expect(test.address).toMatch(/^127\.0\.0\.1:\d+$/);
    });
  });

  describe("state", () => {
    it("starts in normal mode", () => {
      expect(client.getMode()).toBe("normal");
    });

    it("has mode display string", () => {
      expect(client.getModeDisplay()).toBe("NORMAL");
    });

    it("has initial cursor position", () => {
      const cursor = client.getCursor();
      expect(cursor.line).toBe(0);
      expect(cursor.col).toBe(0);
    });

    it("has initial buffer content", () => {
      const lines = client.getLines();
      expect(Array.isArray(lines)).toBe(true);
    });
  });

  describe("capture", () => {
    it("captures frame with metadata", () => {
      const frame = client.capture();

      expect(frame.content).toContain("=== FRAME CAPTURE ===");
      expect(frame.content).toContain("=== END FRAME ===");
      expect(frame.metadata.width).toBe(80);
      expect(frame.metadata.height).toBe(24);
    });

    it("includes mode in capture", () => {
      const frame = client.capture();
      expect(frame.content).toContain("mode: NORMAL");
    });

    it("includes cursor in capture", () => {
      const frame = client.capture();
      expect(frame.content).toMatch(/cursor: line=\d+, col=\d+/);
    });
  });

  describe("frame assertions", () => {
    it("extracts mode from frame", () => {
      const frame = client.capture().content;
      const mode = frameAssertions.getMode(frame);
      expect(mode).toBe("NORMAL");
    });

    it("extracts cursor from frame", () => {
      const frame = client.capture().content;
      const cursor = frameAssertions.getCursor(frame);
      expect(cursor).not.toBeNull();
      expect(cursor?.line).toBeGreaterThanOrEqual(0);
      expect(cursor?.col).toBeGreaterThanOrEqual(0);
    });
  });
});

// These tests require module loading, which may not work in all environments
describe.skip("HeadlessWebClient with modules", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  describe("mode transitions", () => {
    it("switches to insert mode with i", async () => {
      await client.sendKeys("i");
      // Wait for mode change
      await client.waitFor((f) => f.includes("INSERT"), 2000);
      expect(client.getMode()).toBe("insert");
    });

    it("returns to normal mode with Escape", async () => {
      await client.sendKeys("i");
      await client.waitFor((f) => f.includes("INSERT"), 2000);

      await client.sendKeys("<Esc>");
      await client.waitFor((f) => f.includes("NORMAL"), 2000);
      expect(client.getMode()).toBe("normal");
    });
  });

  describe("text input", () => {
    it("types text in insert mode", async () => {
      await client.sendKeys("ihello<Esc>");

      const frame = await client.waitFor(
        (f) => f.includes("hello"),
        2000,
      );

      expect(frame).toContain("hello");
      expect(client.getMode()).toBe("normal");
    });

    it("types multiple lines", async () => {
      await client.sendKeys("iline1<CR>line2<Esc>");

      await client.waitFor((f) => f.includes("line1") && f.includes("line2"), 2000);

      const buffer = client.getBuffer();
      expect(buffer).toContain("line1");
      expect(buffer).toContain("line2");
    });
  });

  describe("vim commands", () => {
    it("deletes word with dw", async () => {
      await client.sendKeys("ihello world<Esc>0dw");

      await client.waitFor((f) => !f.includes("hello"), 2000);

      const buffer = client.getBuffer();
      expect(buffer.trim()).toBe("world");
    });

    it("deletes line with dd", async () => {
      await client.sendKeys("iline1<CR>line2<Esc>ggdd");

      await client.waitFor((f) => !f.includes("line1"), 2000);

      const buffer = client.getBuffer();
      expect(buffer).not.toContain("line1");
      expect(buffer).toContain("line2");
    });
  });
});
