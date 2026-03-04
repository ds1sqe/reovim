/**
 * Explorer Sidebar: Playwright E2E Tests
 *
 * Verifies DOM rendering of the explorer file tree sidebar
 * in a real browser with a live server.
 *
 * Covers Issue #523 (explorer web client support).
 */

import { test, expect, waitForConnection, sendKeys, navigateToServer } from "./fixtures";

test.describe("Extension Rendering: Explorer", () => {
  test.beforeEach(async ({ page, serverPort }) => {
    await navigateToServer(page, serverPort);
    await waitForConnection(page, 15000);
  });

  test("Space+e shows explorer sidebar", async ({ page }) => {
    await sendKeys(page, "<Space>");
    await page.waitForTimeout(200);
    await sendKeys(page, "e");

    const sidebar = page.locator(".explorer-sidebar");
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });

  test("sidebar has header with root name", async ({ page }) => {
    await sendKeys(page, "<Space>");
    await page.waitForTimeout(200);
    await sendKeys(page, "e");

    await expect(page.locator(".explorer-sidebar")).toBeVisible({ timeout: 5000 });
    const header = page.locator(".explorer-header");
    await expect(header).toBeVisible();
    // Root name should be non-empty
    await expect(header).not.toHaveText("");
  });

  test("sidebar contains tree nodes", async ({ page }) => {
    await sendKeys(page, "<Space>");
    await page.waitForTimeout(200);
    await sendKeys(page, "e");

    await expect(page.locator(".explorer-sidebar")).toBeVisible({ timeout: 5000 });
    const nodes = page.locator(".explorer-node");
    await expect(nodes).not.toHaveCount(0);
  });

  test("one node has cursor highlight", async ({ page }) => {
    await sendKeys(page, "<Space>");
    await page.waitForTimeout(200);
    await sendKeys(page, "e");

    await expect(page.locator(".explorer-sidebar")).toBeVisible({ timeout: 5000 });
    const cursorNode = page.locator(".explorer-node.cursor");
    await expect(cursorNode).toHaveCount(1);
  });

  test("sidebar disappears on q", async ({ page }) => {
    await sendKeys(page, "<Space>");
    await page.waitForTimeout(200);
    await sendKeys(page, "e");

    await expect(page.locator(".explorer-sidebar")).toBeVisible({ timeout: 5000 });

    await sendKeys(page, "q");
    await expect(page.locator(".explorer-sidebar")).not.toBeVisible({ timeout: 3000 });
  });

  test("screenshot: explorer sidebar", async ({ page }) => {
    await sendKeys(page, "<Space>");
    await page.waitForTimeout(200);
    await sendKeys(page, "e");

    await expect(page.locator(".explorer-sidebar")).toBeVisible({ timeout: 5000 });
    // Wait for tree to fully render
    await page.waitForTimeout(300);

    await page.screenshot({ path: "tmp/explorer-screenshot.png", fullPage: true });
  });
});
