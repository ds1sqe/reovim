/**
 * Playwright E2E Test Configuration
 *
 * Configures Playwright for testing the reovim web client.
 * The tests require both the Vite dev server and a reovim server.
 */

import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  // Run tests in parallel with reasonable concurrency
  fullyParallel: true,
  // Fail the build on CI if test.only is left in the code
  forbidOnly: !!process.env.CI,
  // Retry failed tests once on CI
  retries: process.env.CI ? 1 : 0,
  // Limit workers on CI to prevent resource exhaustion
  workers: process.env.CI ? 2 : undefined,
  // Reporter configuration
  reporter: [["html", { open: "never" }], ["list"]],
  // Shared settings for all projects
  use: {
    // Base URL for page.goto() calls
    baseURL: "http://localhost:5173",
    // Collect trace on failure for debugging
    trace: "retain-on-failure",
    // Take screenshot on failure
    screenshot: "only-on-failure",
    // Reasonable timeout for actions
    actionTimeout: 10000,
  },
  // Configure projects for different browsers
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    // Firefox and WebKit can be enabled later
    // {
    //   name: "firefox",
    //   use: { ...devices["Desktop Firefox"] },
    // },
    // {
    //   name: "webkit",
    //   use: { ...devices["Desktop Safari"] },
    // },
  ],
  // Timeout for each test
  timeout: 30000,
  // Expect timeout
  expect: {
    timeout: 5000,
  },
  // Run web server before starting tests
  // Use npx vite directly to skip prebuild script which may have wasm-pack issues
  webServer: {
    command: "npx vite",
    url: "http://localhost:5173",
    reuseExistingServer: !process.env.CI,
    timeout: 60000,
  },
});
