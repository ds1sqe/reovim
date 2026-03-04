/**
 * Explorer Extension: Headless Integration Tests
 *
 * Verifies the explorer sidebar works end-to-end:
 *   Server state -> Bridge JSON -> gRPC notification -> Client state
 *
 * Covers Issue #523 (explorer web client support).
 *
 * NOTE: Requires the reovim server binary to be built.
 * Run: cargo build -p reovim-app
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { WebIntegrationTest } from "../helpers/integration.js";
import type { HeadlessWebClient } from "../../src/headless/index.js";

// ========== Helpers ==========

interface ExplorerState {
  active: boolean;
  rootName: string;
  cursorIndex: number;
  scrollOffset: number;
  width: number;
  inputMode: string;
  inputBuffer: string;
  showHidden: boolean;
  message: string | null;
  nodeCount: number;
}

/**
 * Poll until extension becomes active (non-null state).
 */
async function waitForExtension(
  client: HeadlessWebClient,
  kind: string,
  timeoutMs = 5000,
): Promise<Record<string, unknown>> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const state = client.getExtensionState(kind);
    if (state) return state;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`Extension "${kind}" did not activate within ${timeoutMs}ms`);
}

/**
 * Poll until extension becomes inactive (null state).
 */
async function waitForExtensionClear(
  client: HeadlessWebClient,
  kind: string,
  timeoutMs = 3000,
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (client.getExtensionState(kind) === null) return;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`Extension "${kind}" did not deactivate within ${timeoutMs}ms`);
}

// ========== Tests ==========

describe("Extension Audit: Explorer Sidebar", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
    // Wait for initial state
    await new Promise((r) => setTimeout(r, 500));
  });

  afterEach(async () => {
    await test.cleanup();
  });

  it("<Space>e activates explorer extension", async () => {
    // Explorer should start inactive
    const initial = client.getExtensionState("explorer");
    expect(initial).toBeNull();

    // <Space>e opens the explorer sidebar
    await client.sendKeys("<Space>");
    await new Promise((r) => setTimeout(r, 200));
    await client.sendKeys("e");

    // Wait for explorer to activate
    const state = (await waitForExtension(client, "explorer")) as ExplorerState;

    expect(state.active).toBe(true);
    expect(state.nodeCount).toBeGreaterThan(0);
    expect(state.width).toBeGreaterThan(0);
    expect(state.inputMode).toBe("none");

    console.log("[explorer test] State:", JSON.stringify(state, null, 2));
  });

  it("<Space>e then q closes explorer", async () => {
    // Open explorer
    await client.sendKeys("<Space>");
    await new Promise((r) => setTimeout(r, 200));
    await client.sendKeys("e");
    await waitForExtension(client, "explorer");

    // Close explorer with q
    await client.sendKeys("q");
    await waitForExtensionClear(client, "explorer");

    const state = client.getExtensionState("explorer");
    expect(state).toBeNull();
  });

  it("explorer shows root name", async () => {
    await client.sendKeys("<Space>");
    await new Promise((r) => setTimeout(r, 200));
    await client.sendKeys("e");

    const state = (await waitForExtension(client, "explorer")) as ExplorerState;

    // rootName should be the directory name (non-empty)
    expect(state.rootName).toBeTruthy();
    expect(typeof state.rootName).toBe("string");
    console.log("[explorer test] Root name:", state.rootName);
  });

  it("j/k navigation updates cursor index", async () => {
    // Open explorer
    await client.sendKeys("<Space>");
    await new Promise((r) => setTimeout(r, 200));
    await client.sendKeys("e");

    const initial = (await waitForExtension(client, "explorer")) as ExplorerState;
    const initialCursor = initial.cursorIndex;

    // Move down
    await client.sendKeys("j");
    await new Promise((r) => setTimeout(r, 200));

    const afterJ = client.getExtensionState("explorer") as ExplorerState;
    expect(afterJ.cursorIndex).toBe(initialCursor + 1);

    // Move back up
    await client.sendKeys("k");
    await new Promise((r) => setTimeout(r, 200));

    const afterK = client.getExtensionState("explorer") as ExplorerState;
    expect(afterK.cursorIndex).toBe(initialCursor);
  });

  it("frame capture includes explorer state", async () => {
    await client.sendKeys("<Space>");
    await new Promise((r) => setTimeout(r, 200));
    await client.sendKeys("e");

    await waitForExtension(client, "explorer");

    // Capture frame — verify the explorer state is readable
    const frame = client.capture("plain_text");
    console.log("[explorer test] Frame capture:\n", frame.content);

    // Mode should be EXPLORER
    expect(client.getMode().toUpperCase()).toContain("EXPLORER");
  });
});
