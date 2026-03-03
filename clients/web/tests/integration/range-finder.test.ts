/**
 * Range-Finder Module: Headless Web Client Integration Tests (#524)
 *
 * Verifies that range-finder keybindings (s, za, zo, zc, zR, zM) are
 * dispatched correctly through the web client without crashing.
 * Command execute() bodies are stubs in Phase 4.
 *
 * NOTE: Requires the reovim server binary to be built.
 * Run: cargo build -p reovim-app
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { WebIntegrationTest } from "../helpers/integration.js";
import type { HeadlessWebClient } from "../../src/headless/index.js";

describe("Range-Finder Module (#524)", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  describe("jump search (s key)", () => {
    it("s key does not crash the server", async () => {
      // Type some text
      await client.sendKeys("ihello world<Esc>");
      await client.waitFor((f) => f.includes("hello"), 2000);

      // Press s (jump search stub)
      await client.sendKeys("s");
      await new Promise((r) => setTimeout(r, 100));

      // Server should still respond
      expect(client.isConnected()).toBe(true);

      // Buffer should be intact
      const buffer = client.getBuffer();
      expect(buffer).toContain("hello");
    });

    it("s followed by Escape stays in normal mode", async () => {
      await client.sendKeys("s");
      await new Promise((r) => setTimeout(r, 50));

      await client.sendKeys("<Esc>");
      await new Promise((r) => setTimeout(r, 50));

      // Should be in normal mode
      expect(client.getMode()).toBe("normal");
    });
  });

  describe("fold commands (z prefix)", () => {
    it("za (fold toggle) does not crash", async () => {
      await client.sendKeys("ifn main() {<CR>  hello<CR>}<Esc>");
      await client.waitFor((f) => f.includes("main"), 2000);

      await client.sendKeys("za");
      await new Promise((r) => setTimeout(r, 100));

      expect(client.isConnected()).toBe(true);
      expect(client.getBuffer()).toContain("main");
    });

    it("zo (fold open) does not crash", async () => {
      await client.sendKeys("zo");
      await new Promise((r) => setTimeout(r, 100));
      expect(client.isConnected()).toBe(true);
    });

    it("zc (fold close) does not crash", async () => {
      await client.sendKeys("zc");
      await new Promise((r) => setTimeout(r, 100));
      expect(client.isConnected()).toBe(true);
    });

    it("zR (open all folds) does not crash", async () => {
      await client.sendKeys("zR");
      await new Promise((r) => setTimeout(r, 100));
      expect(client.isConnected()).toBe(true);
    });

    it("zM (close all folds) does not crash", async () => {
      await client.sendKeys("zM");
      await new Promise((r) => setTimeout(r, 100));
      expect(client.isConnected()).toBe(true);
    });

    it("all fold keys in sequence do not crash", async () => {
      await client.sendKeys("ifn foo() {<CR>  bar<CR>}<Esc>");
      await client.waitFor((f) => f.includes("foo"), 2000);

      // Fire all fold commands in sequence
      for (const keys of ["za", "zo", "zc", "zR", "zM"]) {
        await client.sendKeys(keys);
        await new Promise((r) => setTimeout(r, 50));
      }

      expect(client.isConnected()).toBe(true);
      expect(client.getMode()).toBe("normal");
      expect(client.getBuffer()).toContain("foo");
    });
  });
});
