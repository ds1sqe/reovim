/**
 * Playwright Test Fixtures
 *
 * Provides a custom fixture that spawns a reovim server for E2E testing.
 * The server is started before tests and stopped after.
 *
 * Usage:
 *   import { test, expect } from "./fixtures";
 *   test("example", async ({ page, serverPort }) => {
 *     // serverPort is the dynamically assigned port
 *   });
 */

import { test as base, expect as baseExpect } from "@playwright/test";
import { spawn, ChildProcess } from "child_process";
import { setTimeout as delay } from "timers/promises";

// Find the reovim binary - check for grpc-enabled binary
function findReovimBinary(): string {
  const projectRoot = process.cwd().replace(/\/clients\/web$/, "");
  // Use reovim (grpc-enabled) for web E2E tests
  // The web client requires gRPC-Web transport
  const releasePath = `${projectRoot}/target/release/reovim`;
  const debugPath = `${projectRoot}/target/debug/reovim`;

  // Allow override via environment variable
  // For CI, the binary should be built with: cargo build -p reovim-app --features grpc
  return process.env.REOVIM_BINARY || releasePath;
}

// Extract port from server output
function extractPort(output: string): number | null {
  // Match patterns like "Listening on 127.0.0.1:12521" or "tcp://127.0.0.1:12522"
  const match = output.match(/(?:Listening on|tcp:\/\/).*?:(\d+)/);
  return match ? parseInt(match[1], 10) : null;
}

interface ServerFixture {
  serverPort: number;
  serverProcess: ChildProcess;
}

/**
 * Extended test fixture that manages a reovim server instance.
 */
export const test = base.extend<ServerFixture>({
  // Server port is dynamically assigned
  serverPort: [
    async ({}, use) => {
      const binary = findReovimBinary();
      let port: number | null = null;

      // Start server with gRPC transport (required for web client)
      // Use port 0 for dynamic assignment
      const serverProcess = spawn(binary, ["server", "--grpc", "0"], {
        stdio: ["ignore", "pipe", "pipe"],
        cwd: process.cwd().replace(/\/clients\/web$/, ""),
      });

      // Collect stderr output to find the port
      let stderrOutput = "";

      serverProcess.stderr?.on("data", (data) => {
        const text = data.toString();
        stderrOutput += text;
        process.stderr.write(`[reovim] ${text}`);

        // Try to extract port from output
        if (!port) {
          port = extractPort(stderrOutput);
        }
      });

      serverProcess.stdout?.on("data", (data) => {
        process.stdout.write(`[reovim] ${data}`);
      });

      // Wait for server to start (with timeout)
      const startTimeout = 10000;
      const startTime = Date.now();

      while (!port && Date.now() - startTime < startTimeout) {
        await delay(100);
        // Check if process died
        if (serverProcess.exitCode !== null) {
          throw new Error(
            `Server exited with code ${serverProcess.exitCode}:\n${stderrOutput}`,
          );
        }
      }

      if (!port) {
        serverProcess.kill("SIGTERM");
        throw new Error(
          `Failed to start server within ${startTimeout}ms:\n${stderrOutput}`,
        );
      }

      console.log(`[fixtures] Server started on port ${port}`);

      // Use the port in tests
      await use(port);

      // Cleanup: kill server after tests
      console.log("[fixtures] Stopping server...");
      serverProcess.kill("SIGTERM");

      // Give it time to shut down gracefully
      await delay(500);

      // Force kill if still running
      if (serverProcess.exitCode === null) {
        serverProcess.kill("SIGKILL");
      }
    },
    { scope: "test" }, // Fresh server for each test
  ],

  // Expose server process for advanced control
  serverProcess: [
    async ({ serverPort }, use) => {
      // This is a dummy - the actual process is managed by serverPort fixture
      // We expose it for potential future use
      await use(undefined as unknown as ChildProcess);
    },
    { scope: "test" },
  ],
});

// Re-export expect for convenience
export { baseExpect as expect };

/**
 * Navigate to the web client with the test server port configured.
 */
export async function navigateWithPort(
  page: import("@playwright/test").Page,
  serverPort: number,
  path = "/",
): Promise<void> {
  const url = `${path}?port=${serverPort}`;
  await page.goto(url);
  await page.waitForLoadState("domcontentloaded");
}

/**
 * Helper to wait for the web client to connect to the server.
 *
 * Checks for the absence of "connecting" or "disconnected" classes on #app.
 */
export async function waitForConnection(
  page: import("@playwright/test").Page,
  timeout = 10000,
): Promise<void> {
  await page.waitForSelector('#app:not(.connecting):not(.disconnected)', {
    timeout,
  });
}

/**
 * Helper to get the current mode from the UI.
 */
export async function getMode(
  page: import("@playwright/test").Page,
): Promise<string> {
  return page.locator("#mode").textContent() as Promise<string>;
}

/**
 * Helper to get the buffer content from the UI.
 */
export async function getBufferContent(
  page: import("@playwright/test").Page,
): Promise<string> {
  return page.locator("#buffer").textContent() as Promise<string>;
}

/**
 * Helper to send keys to the editor.
 * Uses keyboard events to simulate typing.
 */
export async function sendKeys(
  page: import("@playwright/test").Page,
  keys: string,
): Promise<void> {
  // Parse special key sequences like <Esc>, <CR>, <C-w>
  const keyPattern = /<([^>]+)>/g;
  let lastIndex = 0;
  let match;

  while ((match = keyPattern.exec(keys)) !== null) {
    // Type any regular text before this special key
    if (match.index > lastIndex) {
      await page.keyboard.type(keys.slice(lastIndex, match.index));
    }

    // Handle the special key
    const specialKey = match[1].toLowerCase();
    switch (specialKey) {
      case "esc":
      case "escape":
        await page.keyboard.press("Escape");
        break;
      case "cr":
      case "enter":
        await page.keyboard.press("Enter");
        break;
      case "bs":
      case "backspace":
        await page.keyboard.press("Backspace");
        break;
      case "tab":
        await page.keyboard.press("Tab");
        break;
      case "space":
        await page.keyboard.press("Space");
        break;
      default:
        // Handle Ctrl combinations like C-w, C-c
        if (specialKey.startsWith("c-")) {
          const key = specialKey.slice(2);
          await page.keyboard.press(`Control+${key}`);
        } else {
          // Unknown special key - try pressing as-is
          await page.keyboard.press(specialKey);
        }
    }

    lastIndex = match.index + match[0].length;
  }

  // Type any remaining regular text
  if (lastIndex < keys.length) {
    await page.keyboard.type(keys.slice(lastIndex));
  }
}
