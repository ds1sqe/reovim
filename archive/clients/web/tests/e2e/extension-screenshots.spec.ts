/**
 * Extension Screenshots: Visual Regression Baseline
 *
 * Captures Playwright screenshots of which-key and cmdline popups
 * for visual regression testing. Run with --update-snapshots to
 * create initial baselines.
 *
 * Usage:
 *   npx playwright test extension-screenshots.spec.ts --update-snapshots
 *
 * Covers Issues #468 (which-key popup) and #451 (cmdline popup).
 */

import { test, expect, waitForConnection, sendKeys, navigateToServer } from "./fixtures";

test.describe("Visual Baseline: Which-Key", () => {
  test.beforeEach(async ({ page, serverPort }) => {
    await navigateToServer(page, serverPort);
    await waitForConnection(page, 15000);
  });

  test("which-key popup with d operator", async ({ page }) => {
    await sendKeys(page, "d");
    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    // Brief settle time for any CSS transitions
    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("whichkey-d-operator.png", {
      fullPage: true,
      animations: "disabled",
      maxDiffPixelRatio: 0.05,
    });

    await sendKeys(page, "<Esc>");
  });

  test("which-key popup narrowed to di", async ({ page }) => {
    await sendKeys(page, "d");
    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    await sendKeys(page, "i");
    await expect(page.locator(".whichkey-header")).toHaveText("di", { timeout: 3000 });

    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("whichkey-di-narrowed.png", {
      fullPage: true,
      animations: "disabled",
      maxDiffPixelRatio: 0.05,
    });

    await sendKeys(page, "<Esc>");
  });

  test("which-key popup with g prefix", async ({ page }) => {
    await sendKeys(page, "g");
    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("whichkey-g-prefix.png", {
      fullPage: true,
      animations: "disabled",
      maxDiffPixelRatio: 0.05,
    });

    await sendKeys(page, "<Esc>");
  });

  test("which-key popup with y operator", async ({ page }) => {
    await sendKeys(page, "y");
    await expect(page.locator(".whichkey-popup")).toBeVisible({ timeout: 3000 });

    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("whichkey-y-operator.png", {
      fullPage: true,
      animations: "disabled",
      maxDiffPixelRatio: 0.05,
    });

    await sendKeys(page, "<Esc>");
  });
});

test.describe("Visual Baseline: Cmdline", () => {
  test.beforeEach(async ({ page, serverPort }) => {
    await navigateToServer(page, serverPort);
    await waitForConnection(page, 15000);
  });

  test("cmdline with colon prompt", async ({ page }) => {
    await sendKeys(page, ":");
    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });

    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("cmdline-colon-prompt.png", {
      fullPage: true,
      animations: "disabled",
    });

    await sendKeys(page, "<Esc>");
  });

  test("cmdline with typed text", async ({ page }) => {
    await sendKeys(page, ":");
    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });

    await page.keyboard.type("wq");
    await expect(page.locator(".cmdline-content")).toContainText("wq", { timeout: 3000 });

    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("cmdline-typed-wq.png", {
      fullPage: true,
      animations: "disabled",
    });

    await sendKeys(page, "<Esc>");
  });

  test("cmdline with forward search prompt", async ({ page }) => {
    await sendKeys(page, "/");
    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });

    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("cmdline-search-forward.png", {
      fullPage: true,
      animations: "disabled",
    });

    await sendKeys(page, "<Esc>");
  });

  test("cmdline with backward search prompt", async ({ page }) => {
    await sendKeys(page, "?");
    await expect(page.locator(".cmdline-container")).toBeVisible({ timeout: 3000 });

    await page.waitForTimeout(100);

    await expect(page).toHaveScreenshot("cmdline-search-backward.png", {
      fullPage: true,
      animations: "disabled",
    });

    await sendKeys(page, "<Esc>");
  });
});
