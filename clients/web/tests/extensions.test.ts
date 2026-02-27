/**
 * WebExtension System Tests (#468)
 *
 * Comprehensive tests covering the extension interface contract, individual
 * extension implementations, error handling, notification->render flow,
 * and state isolation.
 *
 * Uses jsdom environment for DOM testing.
 * WhichKey tests use fake timers to control the show-delay (#462).
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  createExtensions,
  CmdlineExtension,
  WhichKeyExtension,
} from "../src/extensions/index.js";

// ============ Test Fixtures ============

const CMDLINE_PAYLOAD = JSON.stringify({
  active: true,
  prompt: ":",
  input: "wq",
  cursor: 2,
  completions: ["write", "wq"],
  completion_index: 0,
});

const CMDLINE_SIMPLE = JSON.stringify({
  active: true,
  prompt: ":",
  input: "w",
  cursor: 1,
});

const CMDLINE_DEACTIVATE = JSON.stringify({ active: false });

const WHICHKEY_PAYLOAD = JSON.stringify({
  active: true,
  prefix: "d",
  hints: [
    { key: "d", command: "motions:whole-line" },
    { key: "w", command: "motions:word-forward" },
    { key: "i", command: "textobjects:inner" },
  ],
});

const WHICHKEY_NARROWED = JSON.stringify({
  active: true,
  prefix: "di",
  hints: [
    { key: "w", command: "textobjects:inner-word" },
    { key: "(", command: "textobjects:inner-paren" },
  ],
});

const WHICHKEY_DEACTIVATE = JSON.stringify({ active: false });

/** Default show-delay matches WhichKeyExtension default (500ms). */
const DEFAULT_DELAY_MS = 500;

// ============ 8a: Interface Contract ============

describe("factory", () => {
  it("createExtensions returns 2 extensions with correct kinds", () => {
    const extensions = createExtensions();
    expect(extensions).toHaveLength(2);

    const kinds = extensions.map((e) => e.kind());
    expect(kinds).toContain("cmdline");
    expect(kinds).toContain("whichkey");
  });

  it("extensions start inactive", () => {
    const extensions = createExtensions();
    for (const ext of extensions) {
      expect(ext.isActive()).toBe(false);
    }
  });

  it("inactive extensions return null state", () => {
    const extensions = createExtensions();
    for (const ext of extensions) {
      expect(ext.getState()).toBeNull();
    }
  });
});

// ============ 8b: Notification Payload Parsing ============

describe("CmdlineExtension payload parsing", () => {
  let ext: CmdlineExtension;

  beforeEach(() => {
    ext = new CmdlineExtension();
  });

  it("parses full cmdline payload", () => {
    ext.applyNotification(CMDLINE_PAYLOAD);
    const state = ext.getState();
    expect(state?.active).toBe(true);
    expect(state?.prompt).toBe(":");
    expect(state?.input).toBe("wq");
    expect(state?.cursor).toBe(2);
    expect(state?.completions).toEqual(["write", "wq"]);
    expect(state?.completionIndex).toBe(0);
  });

  it("activates and deactivates", () => {
    ext.applyNotification(CMDLINE_SIMPLE);
    expect(ext.isActive()).toBe(true);

    ext.applyNotification(CMDLINE_DEACTIVATE);
    expect(ext.isActive()).toBe(false);
    expect(ext.getState()).toBeNull();
  });
});

describe("WhichKeyExtension payload parsing", () => {
  let ext: WhichKeyExtension;

  beforeEach(() => {
    vi.useFakeTimers();
    ext = new WhichKeyExtension();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("parses full whichkey payload", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS);

    const state = ext.getState();
    expect(state?.active).toBe(true);
    expect(state?.prefix).toBe("d");
    expect(state?.hints).toHaveLength(3);
  });

  it("narrows hints on subsequent notification", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS);
    expect((ext.getState()?.hints as unknown[]).length).toBe(3);

    ext.applyNotification(WHICHKEY_NARROWED);
    const state = ext.getState();
    expect(state?.prefix).toBe("di");
    expect((state?.hints as unknown[]).length).toBe(2);
  });

  it("activates and deactivates", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS);
    expect(ext.isActive()).toBe(true);

    ext.applyNotification(WHICHKEY_DEACTIVATE);
    expect(ext.isActive()).toBe(false);
    expect(ext.getState()).toBeNull();
  });

  it("not visible before delay expires", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS - 1);
    expect(ext.isActive()).toBe(false);
    expect(ext.getState()).toBeNull();
  });

  it("fast completion prevents popup", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(200);
    ext.applyNotification(WHICHKEY_DEACTIVATE);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS);
    expect(ext.isActive()).toBe(false);
  });
});

// ============ 8c: DOM Rendering + CSS Class Verification ============

describe("CmdlineExtension DOM rendering", () => {
  let ext: CmdlineExtension;
  let container: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = "";
    ext = new CmdlineExtension();
    container = document.createElement("div");
    const cmdlineEl = document.createElement("div");
    cmdlineEl.id = "commandline";
    container.appendChild(cmdlineEl);
    document.body.appendChild(container);
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("renders cmdline DOM with correct CSS classes", () => {
    ext.applyNotification(CMDLINE_SIMPLE);
    ext.render(container);

    expect(container.querySelector(".cmdline-container")).toBeTruthy();
    expect(container.querySelector(".cmdline-prefix")).toBeTruthy();
    expect(container.querySelector(".cmdline-prefix")?.textContent).toBe(":");
    expect(container.querySelector(".cmdline-content")).toBeTruthy();
    expect(container.querySelector(".cmdline-cursor")).toBeTruthy();
  });

  it("renders completions with CSS classes", () => {
    ext.applyNotification(CMDLINE_PAYLOAD);
    ext.render(container);

    const completions = container.querySelectorAll(".cmdline-completion");
    expect(completions.length).toBe(2);
    expect(completions[0]!.textContent).toBe("write");
    expect(completions[1]!.textContent).toBe("wq");
    expect(completions[0]!.classList.contains("selected")).toBe(true);
  });

  it("hide clears commandline element", () => {
    ext.applyNotification(CMDLINE_SIMPLE);
    ext.render(container);
    expect(container.querySelector(".cmdline-container")).toBeTruthy();

    ext.hide();
    const cmdlineEl = document.getElementById("commandline");
    expect(cmdlineEl?.innerHTML).toBe("");
  });
});

describe("WhichKeyExtension DOM rendering", () => {
  let ext: WhichKeyExtension;
  let container: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = "";
    // Zero delay for render tests — they test DOM structure, not timing
    ext = new WhichKeyExtension(0);
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("renders popup with correct CSS classes", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);

    expect(container.querySelector(".whichkey-popup")).toBeTruthy();
    expect(container.querySelector(".whichkey-header")).toBeTruthy();
    expect(container.querySelector(".whichkey-header")?.textContent).toContain("d");
    expect(container.querySelector(".whichkey-hints")).toBeTruthy();
  });

  it("renders all hints with key and command classes", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);

    const hints = container.querySelectorAll(".whichkey-hint");
    expect(hints.length).toBe(3);

    const firstKey = container.querySelector(".whichkey-key");
    expect(firstKey?.textContent).toBe("d");

    const firstCommand = container.querySelector(".whichkey-command");
    expect(firstCommand?.textContent).toBe("motions:whole-line");
  });

  it("popup has overlay class", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);

    const popup = container.querySelector(".whichkey-popup");
    expect(popup?.classList.contains("overlay")).toBe(true);
  });

  it("hide removes popup from DOM", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);
    expect(container.querySelector(".whichkey-popup")).toBeTruthy();

    ext.hide();
    expect(container.querySelector(".whichkey-popup")).toBeNull();
  });

  it("re-render replaces popup instead of duplicating", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);
    ext.render(container);

    const popups = container.querySelectorAll(".whichkey-popup");
    expect(popups.length).toBe(1);
  });
});

// ============ 8d: Error Handling ============

describe("error handling", () => {
  it("CmdlineExtension: invalid JSON retains previous state", () => {
    const ext = new CmdlineExtension();
    ext.applyNotification(CMDLINE_SIMPLE);
    expect(ext.isActive()).toBe(true);
    expect(ext.getState()?.input).toBe("w");

    ext.applyNotification("{broken json!!}");
    expect(ext.isActive()).toBe(true);
    expect(ext.getState()?.input).toBe("w");
  });

  it("WhichKeyExtension: invalid JSON retains previous state", () => {
    vi.useFakeTimers();
    const ext = new WhichKeyExtension();
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS);
    expect(ext.isActive()).toBe(true);

    ext.applyNotification("not json");
    // Previous state retained (still visible)
    expect(ext.isActive()).toBe(true);
    expect(ext.getState()?.prefix).toBe("d");
    vi.useRealTimers();
  });

  it("empty string does not throw", () => {
    const ext = new CmdlineExtension();
    expect(() => ext.applyNotification("")).not.toThrow();
  });

  it("null-like data does not throw", () => {
    const ext = new WhichKeyExtension();
    expect(() => ext.applyNotification("null")).not.toThrow();
  });
});

// ============ 8e: Integration — Notification->Render Flow ============

describe("notification -> render flow", () => {
  it("dispatches extensionUpdated to matching extension", () => {
    document.body.innerHTML = "";
    const extensions = createExtensions();
    const container = document.createElement("div");
    const cmdlineEl = document.createElement("div");
    cmdlineEl.id = "commandline";
    container.appendChild(cmdlineEl);
    document.body.appendChild(container);

    const notification = {
      kind: "cmdline",
      data: CMDLINE_SIMPLE,
    };

    for (const ext of extensions) {
      if (ext.kind() === notification.kind) {
        ext.applyNotification(notification.data);
        ext.isActive() ? ext.render(container) : ext.hide();
      }
    }

    expect(container.querySelector(".cmdline-prefix")?.textContent).toBe(":");

    // Non-matching extension was NOT affected
    const whichkey = extensions.find((e) => e.kind() === "whichkey")!;
    expect(whichkey.isActive()).toBe(false);
  });

  it("deactivation hides extension DOM", () => {
    document.body.innerHTML = "";
    const ext = new CmdlineExtension();
    const container = document.createElement("div");
    const cmdlineEl = document.createElement("div");
    cmdlineEl.id = "commandline";
    container.appendChild(cmdlineEl);
    document.body.appendChild(container);

    ext.applyNotification(CMDLINE_SIMPLE);
    ext.render(container);
    expect(container.querySelector(".cmdline-container")).toBeTruthy();

    ext.applyNotification(CMDLINE_DEACTIVATE);
    ext.hide();
    expect(document.getElementById("commandline")?.innerHTML).toBe("");
  });

  it("state isolation: cmdline notification does not affect whichkey", () => {
    document.body.innerHTML = "";
    const extensions = createExtensions();
    const container = document.createElement("div");
    const cmdlineEl = document.createElement("div");
    cmdlineEl.id = "commandline";
    container.appendChild(cmdlineEl);
    document.body.appendChild(container);

    const notification = {
      kind: "cmdline",
      data: CMDLINE_SIMPLE,
    };
    for (const ext of extensions) {
      if (ext.kind() === notification.kind) {
        ext.applyNotification(notification.data);
        ext.render(container);
      }
    }

    const whichkey = extensions.find((e) => e.kind() === "whichkey")!;
    expect(whichkey.isActive()).toBe(false);
    expect(whichkey.getState()).toBeNull();
  });

  it("state isolation: whichkey notification does not affect cmdline", () => {
    document.body.innerHTML = "";
    const extensions = createExtensions();
    const container = document.createElement("div");
    document.body.appendChild(container);

    const notification = {
      kind: "whichkey",
      data: WHICHKEY_PAYLOAD,
    };
    for (const ext of extensions) {
      if (ext.kind() === notification.kind) {
        ext.applyNotification(notification.data);
        ext.render(container);
      }
    }

    const cmdline = extensions.find((e) => e.kind() === "cmdline")!;
    expect(cmdline.isActive()).toBe(false);
    expect(cmdline.getState()).toBeNull();
  });

  it("non-matching kind is ignored", () => {
    const extensions = createExtensions();
    const notification = { kind: "unknown-extension", data: "{}" };

    for (const ext of extensions) {
      if (ext.kind() === notification.kind) {
        ext.applyNotification(notification.data);
      }
    }

    // All extensions remain inactive
    for (const ext of extensions) {
      expect(ext.isActive()).toBe(false);
    }
  });
});

// ============ 8f: getState for Headless Testing ============

describe("headless state queries", () => {
  it("CmdlineExtension getState returns full state when active", () => {
    const ext = new CmdlineExtension();
    ext.applyNotification(CMDLINE_PAYLOAD);

    const state = ext.getState();
    expect(state).not.toBeNull();
    expect(state?.active).toBe(true);
    expect(state?.prompt).toBe(":");
    expect(state?.input).toBe("wq");
    expect(state?.cursor).toBe(2);
    expect(state?.completions).toEqual(["write", "wq"]);
  });

  it("WhichKeyExtension getState returns full state when active", () => {
    vi.useFakeTimers();
    const ext = new WhichKeyExtension();
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS);

    const state = ext.getState();
    expect(state).not.toBeNull();
    expect(state?.active).toBe(true);
    expect(state?.prefix).toBe("d");
    expect(state?.hints).toHaveLength(3);
    vi.useRealTimers();
  });

  it("getState reflects latest notification", () => {
    vi.useFakeTimers();
    const ext = new WhichKeyExtension();
    ext.applyNotification(WHICHKEY_PAYLOAD);
    vi.advanceTimersByTime(DEFAULT_DELAY_MS);
    expect(ext.getState()?.prefix).toBe("d");

    ext.applyNotification(WHICHKEY_NARROWED);
    expect(ext.getState()?.prefix).toBe("di");
    expect((ext.getState()?.hints as unknown[]).length).toBe(2);
    vi.useRealTimers();
  });
});

// ============ 8g: Container Null Handling ============

describe("container null handling", () => {
  it("CmdlineExtension render with empty container is a no-op", () => {
    const ext = new CmdlineExtension();
    ext.applyNotification(CMDLINE_SIMPLE);

    const emptyContainer = document.createElement("div");
    // No #commandline child -- render should not throw
    expect(() => ext.render(emptyContainer)).not.toThrow();
  });

  it("WhichKeyExtension render with any container works", () => {
    const ext = new WhichKeyExtension(0);
    ext.applyNotification(WHICHKEY_PAYLOAD);

    const container = document.createElement("div");
    expect(() => ext.render(container)).not.toThrow();
    expect(container.querySelector(".whichkey-popup")).toBeTruthy();
  });

  it("hide without prior render does not throw", () => {
    const cmdline = new CmdlineExtension();
    expect(() => cmdline.hide()).not.toThrow();

    const whichkey = new WhichKeyExtension();
    expect(() => whichkey.hide()).not.toThrow();
  });
});
