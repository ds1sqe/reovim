/**
 * Snippet Management Integration Tests (#529)
 *
 * Tests verify Phase 6-7 features (catalog, reload, global fallback) through
 * the web client gRPC stack.
 *
 * Requires `~/.local/share/reovim/modules/snippets/global.json` with test snippets.
 *
 * Running:
 * ```bash
 * npm run test:integration -- --testPathPattern snippet-management
 * ```
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { WebIntegrationTest } from "../helpers/integration.js";
import type { HeadlessWebClient } from "../../src/headless/index.js";

describe("Snippet Management (#529)", () => {
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
  // Catalog Command (<Space>sc)
  // ==========================================================================

  it("catalog command shows notification toast", async () => {
    // Normal mode → <Space>sc
    await client.sendKeys("<Esc>");
    await new Promise((r) => setTimeout(r, 200));
    await client.sendKeys(" sc");

    // Wait for notification to appear
    await new Promise((r) => setTimeout(r, 1000));

    const notifState = client.getExtensionState("notification");
    expect(notifState).not.toBeNull();
    expect(notifState?.toasts).toBeDefined();

    const toasts = notifState?.toasts as Array<{
      title: string;
      body: string;
    }>;
    const catalogToast = toasts?.find(
      (t) =>
        t.title.includes("Snippets for") || t.title.includes("available"),
    );
    expect(catalogToast).toBeDefined();
  });

  // ==========================================================================
  // Reload Command (<Space>sr)
  // ==========================================================================

  it("reload command shows notification toast", async () => {
    await client.sendKeys("<Esc>");
    await new Promise((r) => setTimeout(r, 200));
    await client.sendKeys(" sr");

    // Wait for notification
    await new Promise((r) => setTimeout(r, 1000));

    const notifState = client.getExtensionState("notification");
    expect(notifState).not.toBeNull();
    expect(notifState?.toasts).toBeDefined();

    const toasts = notifState?.toasts as Array<{ title: string }>;
    const reloadToast = toasts?.find(
      (t) =>
        t.title.includes("Snippets reloaded") || t.title.includes("sources"),
    );
    expect(reloadToast).toBeDefined();
  });

  // ==========================================================================
  // Global Fallback
  // ==========================================================================

  it("global snippets work from scratch buffer", async () => {
    await client.sendKeys("itst<C-s>");
    const frame = await client.waitFor(
      (f) => f.includes("TEST_EXPANDED"),
      3000,
    );
    expect(frame).toContain("TEST_EXPANDED");
  });
});
