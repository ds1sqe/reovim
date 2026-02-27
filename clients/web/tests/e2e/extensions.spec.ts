/**
 * Extension Rendering: Playwright E2E Tests
 *
 * Verifies DOM rendering of which-key and cmdline extension popups
 * in a real browser with a live server.
 *
 * Covers Issues #468 (which-key popup) and #451 (cmdline popup).
 *
 * NOTE: Requires both the Vite dev server AND a reovim server binary.
 */

import { test, expect, waitForConnection, sendKeys, navigateToServer } from "./fixtures";

// ========== 2A: Which-Key DOM Rendering ==========

test.describe("Extension Rendering: Which-Key", () => {
  test.beforeEach(async ({ page, serverPort }) => {
    await navigateToServer(page, serverPort);
    await waitForConnection(page, 15000);
  });

  test("shows which-key popup on d", async ({ page }) => {
    await sendKeys(page, "d");

    const popup = page.locator(".whichkey-popup");
    await expect(popup).toBeVisible({ timeout: 3000 });
  });

  test("popup header displays operator prefix", async ({ page }) => {
    await sendKeys(page, "d");

    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });
    await expect(page.locator(".whichkey-header")).toHaveText("d");
  });

  test("popup contains hint entries", async ({ page }) => {
    await sendKeys(page, "d");

    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    const hints = page.locator(".whichkey-hint");
    await expect(hints).not.toHaveCount(0);
  });

  test("hint entries have key and command spans", async ({ page }) => {
    await sendKeys(page, "d");

    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    // At least one hint should have both key and command
    const firstHint = page.locator(".whichkey-hint").first();
    await expect(firstHint.locator(".whichkey-key")).toBeVisible();
    await expect(firstHint.locator(".whichkey-command")).toBeVisible();
  });

  test("popup has overlay class for positioning", async ({ page }) => {
    await sendKeys(page, "d");

    const popup = page.locator(".whichkey-popup");
    await expect(popup).toBeVisible({ timeout: 3000 });
    await expect(popup).toHaveClass(/overlay/);
  });

  test("popup disappears on Escape", async ({ page }) => {
    await sendKeys(page, "d");
    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    await sendKeys(page, "<Esc>");
    await expect(page.locator(".whichkey-popup")).not.toBeVisible({ timeout: 3000 });
  });

  test("g prefix shows goto hints", async ({ page }) => {
    await sendKeys(page, "g");

    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });
    await expect(page.locator(".whichkey-header")).toHaveText("g");

    const hints = page.locator(".whichkey-hint");
    await expect(hints).not.toHaveCount(0);

    // Clean up
    await sendKeys(page, "<Esc>");
  });

  test("narrowing updates header to di", async ({ page }) => {
    await sendKeys(page, "d");
    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    await sendKeys(page, "i");
    // Header should update to "di"
    await expect(page.locator(".whichkey-header")).toHaveText("di", { timeout: 3000 });

    await sendKeys(page, "<Esc>");
  });
});

// ========== 2B: Cmdline DOM Rendering ==========

test.describe("Extension Rendering: Cmdline", () => {
  test.beforeEach(async ({ page, serverPort }) => {
    await navigateToServer(page, serverPort);
    await waitForConnection(page, 15000);
  });

  test("shows cmdline container on colon", async ({ page }) => {
    await sendKeys(page, ":");

    const container = page.locator(".cmdline-container");
    await expect(container).toBeVisible({ timeout: 3000 });
  });

  test("cmdline shows colon prompt", async ({ page }) => {
    await sendKeys(page, ":");

    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });
    await expect(page.locator(".cmdline-prefix")).toHaveText(":");
  });

  test("cmdline has cursor element", async ({ page }) => {
    await sendKeys(page, ":");

    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });
    await expect(page.locator(".cmdline-cursor")).toBeVisible();
  });

  test("typed text appears in cmdline content", async ({ page }) => {
    await sendKeys(page, ":");
    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });

    // Type text into cmdline
    await page.keyboard.type("wq");

    // Wait for the text to appear
    await expect(page.locator(".cmdline-content")).toContainText("wq", { timeout: 3000 });
  });

  test("cmdline disappears on Escape", async ({ page }) => {
    await sendKeys(page, ":");
    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });

    await sendKeys(page, "<Esc>");
    await expect(page.locator(".cmdline-container")).not.toBeVisible({ timeout: 3000 });
  });

  test("forward search shows / prompt", async ({ page }) => {
    await sendKeys(page, "/");

    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });
    await expect(page.locator(".cmdline-prefix")).toHaveText("/");

    await sendKeys(page, "<Esc>");
  });

  test("backward search shows ? prompt", async ({ page }) => {
    await sendKeys(page, "?");

    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });
    await expect(page.locator(".cmdline-prefix")).toHaveText("?");

    await sendKeys(page, "<Esc>");
  });
});
