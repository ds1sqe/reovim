/**
 * Extension Audit: Headless Integration Tests
 *
 * Full server pipeline audit for which-key and cmdline extensions.
 * Uses HeadlessWebClient to verify the complete flow:
 *   Server state -> Bridge JSON -> gRPC notification -> Client state
 *
 * Covers Issues #468 (extension system + which-key) and #451 (cmdline popup).
 *
 * NOTE: Requires the reovim server binary to be built.
 * Run: cargo build -p reovim-app
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { WebIntegrationTest } from "../helpers/integration.js";
import type { HeadlessWebClient } from "../../src/headless/index.js";

// ========== Helpers ==========

interface WhichKeyState {
  active: boolean;
  prefix: string;
  hints: Array<{ key: string; command: string }>;
}

interface CmdlineState {
  active: boolean;
  prompt: string;
  input: string;
  cursor: number;
  completions: string[];
  completionIndex: number;
}

/**
 * Poll until extension becomes active (non-null state).
 */
async function waitForExtension(
  client: HeadlessWebClient,
  kind: string,
  timeoutMs = 3000,
): Promise<Record<string, unknown>> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const state = client.getExtensionState(kind);
    if (state) return state;
    await new Promise((r) => setTimeout(r, 10));
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
    await new Promise((r) => setTimeout(r, 10));
  }
  throw new Error(`Extension "${kind}" did not deactivate within ${timeoutMs}ms`);
}

/**
 * Poll until extension state matches a predicate.
 */
async function waitForExtensionState(
  client: HeadlessWebClient,
  kind: string,
  predicate: (state: Record<string, unknown>) => boolean,
  timeoutMs = 3000,
): Promise<Record<string, unknown>> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const state = client.getExtensionState(kind);
    if (state && predicate(state)) return state;
    await new Promise((r) => setTimeout(r, 10));
  }
  const lastState = client.getExtensionState(kind);
  throw new Error(
    `Extension "${kind}" state predicate not met within ${timeoutMs}ms. ` +
      `Last state: ${JSON.stringify(lastState)}`,
  );
}

// ========== 1A: Which-Key Activation & Deactivation ==========

describe("Extension Audit: Which-Key Activation", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("d activates which-key with delete operator hints", async () => {
    await client.sendKeys("d");
    const state = (await waitForExtension(client, "whichkey")) as unknown as WhichKeyState;

    expect(state.active).toBe(true);
    expect(state.prefix).toBe("d");
    expect(state.hints.length).toBeGreaterThan(0);

    const hintKeys = state.hints.map((h) => h.key);
    expect(hintKeys).toContain("d"); // dd = delete whole line
    expect(hintKeys).toContain("w"); // dw = delete word

    // Clean up: Escape back to normal
    await client.sendKeys("<Esc>");
  });

  it("y activates which-key with yank operator hints", async () => {
    await client.sendKeys("y");
    const state = (await waitForExtension(client, "whichkey")) as unknown as WhichKeyState;

    expect(state.active).toBe(true);
    expect(state.prefix).toBe("y");
    expect(state.hints.length).toBeGreaterThan(0);

    await client.sendKeys("<Esc>");
  });

  it("c activates which-key with change operator hints", async () => {
    await client.sendKeys("c");
    const state = (await waitForExtension(client, "whichkey")) as unknown as WhichKeyState;

    expect(state.active).toBe(true);
    expect(state.prefix).toBe("c");
    expect(state.hints.length).toBeGreaterThan(0);

    await client.sendKeys("<Esc>");
  });

  it("g activates which-key with goto hints", async () => {
    await client.sendKeys("g");
    const state = (await waitForExtension(client, "whichkey")) as unknown as WhichKeyState;

    expect(state.active).toBe(true);
    expect(state.prefix).toBe("g");
    expect(state.hints.length).toBeGreaterThan(0);

    const hintKeys = state.hints.map((h) => h.key);
    expect(hintKeys).toContain("gg"); // gg = go to top

    await client.sendKeys("<Esc>");
  });

  it("Escape deactivates which-key", async () => {
    await client.sendKeys("d");
    await waitForExtension(client, "whichkey");

    await client.sendKeys("<Esc>");
    await waitForExtensionClear(client, "whichkey");

    expect(client.getExtensionState("whichkey")).toBeNull();
  });

  it("completing a motion deactivates which-key", async () => {
    // Use dw on a buffer with content to avoid empty-buffer edge cases
    await client.sendKeys("ihello world<Esc>0");
    // Wait for insert mode to complete
    await new Promise((r) => setTimeout(r, 200));

    await client.sendKeys("d");
    await waitForExtension(client, "whichkey");

    // dw = delete word (completes the operator)
    await client.sendKeys("w");
    await waitForExtensionClear(client, "whichkey");

    expect(client.getExtensionState("whichkey")).toBeNull();
  });
});

// ========== 1B: Which-Key Narrowing ==========

describe("Extension Audit: Which-Key Narrowing", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("d then i narrows to inner textobjects", async () => {
    await client.sendKeys("d");
    await waitForExtension(client, "whichkey");

    await client.sendKeys("i");
    const state = (await waitForExtensionState(
      client,
      "whichkey",
      (s) => (s as unknown as WhichKeyState).prefix === "di",
    )) as unknown as WhichKeyState;

    expect(state.prefix).toBe("di");
    // Hints should be narrowed to inner textobjects
    expect(state.hints.length).toBeGreaterThan(0);

    await client.sendKeys("<Esc>");
  });

  it("d then a narrows to outer textobjects", async () => {
    await client.sendKeys("d");
    await waitForExtension(client, "whichkey");

    await client.sendKeys("a");
    const state = (await waitForExtensionState(
      client,
      "whichkey",
      (s) => (s as unknown as WhichKeyState).prefix === "da",
    )) as unknown as WhichKeyState;

    expect(state.prefix).toBe("da");
    expect(state.hints.length).toBeGreaterThan(0);

    await client.sendKeys("<Esc>");
  });
});

// ========== 1C: Cmdline Activation & Deactivation ==========

describe("Extension Audit: Cmdline Activation", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it(": activates cmdline with command prompt", async () => {
    await client.sendKeys(":");
    const state = (await waitForExtension(client, "cmdline")) as unknown as CmdlineState;

    expect(state.active).toBe(true);
    expect(state.prompt).toBe(":");
    expect(state.input).toBe("");
    expect(state.cursor).toBe(0);

    await client.sendKeys("<Esc>");
  });

  it("/ activates cmdline with forward search prompt", async () => {
    await client.sendKeys("/");
    const state = (await waitForExtension(client, "cmdline")) as unknown as CmdlineState;

    expect(state.active).toBe(true);
    expect(state.prompt).toBe("/");

    await client.sendKeys("<Esc>");
  });

  it("? activates cmdline with backward search prompt", async () => {
    await client.sendKeys("?");
    const state = (await waitForExtension(client, "cmdline")) as unknown as CmdlineState;

    expect(state.active).toBe(true);
    expect(state.prompt).toBe("?");

    await client.sendKeys("<Esc>");
  });

  it("Escape deactivates cmdline", async () => {
    await client.sendKeys(":");
    await waitForExtension(client, "cmdline");

    await client.sendKeys("<Esc>");
    await waitForExtensionClear(client, "cmdline");

    expect(client.getExtensionState("cmdline")).toBeNull();
  });

  it("Enter deactivates cmdline", async () => {
    await client.sendKeys(":");
    await waitForExtension(client, "cmdline");

    await client.sendKeys("<CR>");
    await waitForExtensionClear(client, "cmdline");

    expect(client.getExtensionState("cmdline")).toBeNull();
  });
});

// ========== 1D: Cmdline Typing ==========

describe("Extension Audit: Cmdline Typing", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("typed text appears in cmdline input", async () => {
    await client.sendKeys(":");
    await waitForExtension(client, "cmdline");

    await client.sendKeys("wq");
    const state = (await waitForExtensionState(
      client,
      "cmdline",
      (s) => (s as unknown as CmdlineState).input === "wq",
    )) as unknown as CmdlineState;

    expect(state.input).toBe("wq");
    expect(state.cursor).toBe(2);

    await client.sendKeys("<Esc>");
  });

  it("backspace deletes character before cursor", async () => {
    await client.sendKeys(":");
    await waitForExtension(client, "cmdline");

    await client.sendKeys("wq");
    await waitForExtensionState(
      client,
      "cmdline",
      (s) => (s as unknown as CmdlineState).input === "wq",
    );

    await client.sendKeys("<BS>");
    const state = (await waitForExtensionState(
      client,
      "cmdline",
      (s) => (s as unknown as CmdlineState).input === "w",
    )) as unknown as CmdlineState;

    expect(state.input).toBe("w");
    expect(state.cursor).toBe(1);

    await client.sendKeys("<Esc>");
  });
});

// ========== 1E: State Isolation ==========

describe("Extension Audit: State Isolation", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("which-key does not activate cmdline", async () => {
    await client.sendKeys("d");
    await waitForExtension(client, "whichkey");

    expect(client.getExtensionState("cmdline")).toBeNull();

    await client.sendKeys("<Esc>");
  });

  it("cmdline does not activate which-key", async () => {
    await client.sendKeys(":");
    await waitForExtension(client, "cmdline");

    expect(client.getExtensionState("whichkey")).toBeNull();

    await client.sendKeys("<Esc>");
  });
});

// ========== 2A: Microscope Picker Activation ==========

interface MicroscopeState {
  active: boolean;
  query: string;
  cursor: number;
  selected: number;
  pickerName: string;
  pickerTitle: string;
  prompt: string;
  items: Array<{ display: string; detail?: string; icon?: string }>;
  totalCount: number;
  matchedCount: number;
  preview?: {
    lines: string[];
    highlightLine?: number;
    filePath?: string;
  };
}

describe("Extension Audit: Microscope File Picker", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("<Space>f activates microscope with file picker", async () => {
    await client.sendKeys("<Space>f");
    const state = (await waitForExtension(
      client,
      "microscope",
    )) as unknown as MicroscopeState;

    expect(state.active).toBe(true);
    expect(state.pickerName).toBe("files");
    expect(state.pickerTitle).toBe("Files");
    expect(state.prompt).toBe("> ");
    expect(state.query).toBe("");
    expect(state.cursor).toBe(0);

    await client.sendKeys("<Esc>");
  });

  it("Escape deactivates microscope", async () => {
    await client.sendKeys("<Space>f");
    await waitForExtension(client, "microscope");

    await client.sendKeys("<Esc>");
    await waitForExtensionClear(client, "microscope");

    expect(client.getExtensionState("microscope")).toBeNull();
  });

  it("typing updates query in microscope state", async () => {
    await client.sendKeys("<Space>f");
    await waitForExtension(client, "microscope");

    await client.sendKeys("main");
    const state = (await waitForExtensionState(
      client,
      "microscope",
      (s) => (s as unknown as MicroscopeState).query === "main",
    )) as unknown as MicroscopeState;

    expect(state.query).toBe("main");
    expect(state.cursor).toBe(4);

    await client.sendKeys("<Esc>");
  });

  it("backspace removes character from query", async () => {
    await client.sendKeys("<Space>f");
    await waitForExtension(client, "microscope");

    await client.sendKeys("ab");
    await waitForExtensionState(
      client,
      "microscope",
      (s) => (s as unknown as MicroscopeState).query === "ab",
    );

    await client.sendKeys("<BS>");
    const state = (await waitForExtensionState(
      client,
      "microscope",
      (s) => (s as unknown as MicroscopeState).query === "a",
    )) as unknown as MicroscopeState;

    expect(state.query).toBe("a");
    expect(state.cursor).toBe(1);

    await client.sendKeys("<Esc>");
  });
});

// ========== 2B: Microscope Grep Picker ==========

describe("Extension Audit: Microscope Grep Picker", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("<Space>g activates microscope with grep picker", async () => {
    await client.sendKeys("<Space>g");
    const state = (await waitForExtension(
      client,
      "microscope",
    )) as unknown as MicroscopeState;

    expect(state.active).toBe(true);
    expect(state.pickerName).toBe("grep");
    expect(state.pickerTitle).toBe("Grep");
    expect(state.prompt).toBe("rg> ");

    await client.sendKeys("<Esc>");
  });
});

// ========== 2C: Microscope Buffer Picker ==========

describe("Extension Audit: Microscope Buffer Picker", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("<Space>b activates microscope with buffer picker", async () => {
    await client.sendKeys("<Space>b");
    const state = (await waitForExtension(
      client,
      "microscope",
    )) as unknown as MicroscopeState;

    expect(state.active).toBe(true);
    expect(state.pickerName).toBe("buffers");
    expect(state.pickerTitle).toBe("Buffers");
    expect(state.prompt).toBe("> ");

    await client.sendKeys("<Esc>");
  });
});

// ========== 2D: Microscope Command Picker ==========

describe("Extension Audit: Microscope Command Picker", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("<Space>; activates microscope with command picker", async () => {
    await client.sendKeys("<Space>;");
    const state = (await waitForExtension(
      client,
      "microscope",
    )) as unknown as MicroscopeState;

    expect(state.active).toBe(true);
    expect(state.pickerName).toBe("commands");
    expect(state.pickerTitle).toBe("Commands");
    expect(state.prompt).toBe("> ");

    await client.sendKeys("<Esc>");
  });
});

// ========== 2E: Microscope State Isolation ==========

describe("Extension Audit: Microscope State Isolation", () => {
  let test: WebIntegrationTest;
  let client: HeadlessWebClient;

  beforeEach(async () => {
    test = await WebIntegrationTest.create();
    client = await test.connect();
  }, 15000);

  afterEach(async () => {
    await test.cleanup();
  });

  it("microscope does not activate cmdline or which-key", async () => {
    await client.sendKeys("<Space>f");
    await waitForExtension(client, "microscope");

    expect(client.getExtensionState("cmdline")).toBeNull();
    expect(client.getExtensionState("whichkey")).toBeNull();

    await client.sendKeys("<Esc>");
  });
});
