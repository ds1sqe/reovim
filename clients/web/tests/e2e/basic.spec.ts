/**
 * Basic E2E Tests for reovim Web Client
 *
 * These tests verify fundamental functionality:
 * - Page loads and connects to server
 * - Mode display works
 * - Basic text input
 *
 * NOTE: These tests require both the Vite dev server AND a reovim server.
 * The fixture automatically spawns a reovim server for each test.
 */

import { test, expect, waitForConnection, getMode, sendKeys, navigateWithPort } from "./fixtures";

test.describe("reovim Web Client", () => {
  test.beforeEach(async ({ page, serverPort }) => {
    // Navigate to the web client with the correct server port
    // The web client reads the server URL from window.location.hostname:12521
    // For E2E tests, we need to configure it dynamically
    await page.goto("/");

    // Wait for initial page load
    await page.waitForLoadState("domcontentloaded");
  });

  test("should load the editor page", async ({ page }) => {
    // Check that the main elements are present
    await expect(page.locator("#app")).toBeVisible();
    await expect(page.locator("#statusline")).toBeVisible();
    await expect(page.locator("#editor")).toBeVisible();
    await expect(page.locator("#mode")).toBeVisible();
  });

  test("should display mode indicator", async ({ page }) => {
    // The mode indicator should show something (even if disconnected)
    const modeElement = page.locator("#mode");
    await expect(modeElement).toBeVisible();

    // Should have some text content
    const modeText = await modeElement.textContent();
    expect(modeText).toBeTruthy();
  });

  test("should have editor and buffer elements", async ({ page }) => {
    // Check for the editor structure
    await expect(page.locator("#editor")).toBeVisible();
    await expect(page.locator("#buffer")).toBeVisible();
    await expect(page.locator("#cursor")).toBeVisible();
  });

  test("should have commandline element", async ({ page }) => {
    // Commandline footer should exist
    await expect(page.locator("#commandline")).toBeVisible();
  });

  test("should display page title", async ({ page }) => {
    // Check the page title
    await expect(page).toHaveTitle(/reovim/i);
  });
});

// Server-connected tests (require server fixture)
test.describe("reovim Connected Tests", () => {
  // Skip these tests if we can't connect to the server
  // In CI, the server binary might not be available
  test.skip(
    ({ serverPort }) => !serverPort,
    "Server not available",
  );

  test.beforeEach(async ({ page, serverPort }) => {
    // Navigate with the dynamically assigned server port
    await navigateWithPort(page, serverPort);
  });

  test("should connect to server", async ({ page }) => {
    // Wait for connection
    await waitForConnection(page, 15000);

    // Check connection status
    const app = page.locator("#app");
    await expect(app).not.toHaveClass(/disconnected/);
    await expect(app).not.toHaveClass(/connecting/);
  });

  test("should show NORMAL mode after connection", async ({ page }) => {
    await waitForConnection(page, 15000);

    const mode = await getMode(page);
    expect(mode?.toUpperCase()).toContain("NORMAL");
  });

  test("should switch to INSERT mode with 'i'", async ({ page }) => {
    await waitForConnection(page, 15000);

    // Capture console messages for debugging
    const consoleLogs: string[] = [];
    page.on("console", (msg) => consoleLogs.push(`[${msg.type()}] ${msg.text()}`));

    // Verify we start in NORMAL mode
    const initialMode = await getMode(page);
    expect(initialMode?.toUpperCase()).toContain("NORMAL");

    // Press 'i' to enter insert mode
    // Focus the document first to ensure key events are captured
    await page.locator("body").click();
    await page.keyboard.press("i");

    // Wait a moment for server round-trip
    await page.waitForTimeout(500);

    // Log any console messages for debugging
    if (consoleLogs.length > 0) {
      console.log("Browser console:", consoleLogs.join("\n"));
    }

    // Wait for mode element to contain INSERT (with retry)
    await expect(page.locator("#mode")).toContainText(/INSERT/i, { timeout: 5000 });
  });

  test("should return to NORMAL mode with Escape", async ({ page }) => {
    await waitForConnection(page, 15000);

    // Enter insert mode
    await sendKeys(page, "i");
    // Wait for INSERT mode
    await expect(page.locator("#mode")).toContainText(/INSERT/i, { timeout: 5000 });

    // Exit to normal mode
    await sendKeys(page, "<Esc>");
    // Wait for NORMAL mode
    await expect(page.locator("#mode")).toContainText(/NORMAL/i, { timeout: 5000 });
  });
});
