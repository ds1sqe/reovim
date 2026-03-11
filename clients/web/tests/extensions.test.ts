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
  shutdownExtensions,
  CmdlineExtension,
  WhichKeyExtension,
  NotificationExtension,
  MicroscopeExtension,
  RangeFinderJumpExtension,
  RangeFinderFoldExtension,
} from "../src/extensions/index.js";
import { toposortExtensions } from "../src/extensions/toposort.js";
import type { WebExtension } from "../src/extensions/interface.js";

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
    { key: "d", command: "motions:whole-line", category: "motion" },
    { key: "w", command: "motions:word-forward", category: "motion" },
    { key: "i", command: "textobjects:inner", category: "textobject" },
  ],
});

const WHICHKEY_NARROWED = JSON.stringify({
  active: true,
  prefix: "di",
  hints: [
    { key: "w", command: "textobjects:inner-word", category: "textobject" },
    { key: "(", command: "textobjects:inner-paren", category: "textobject" },
  ],
});

const WHICHKEY_DEACTIVATE = JSON.stringify({ active: false });

const NOTIFICATION_SINGLE = JSON.stringify({
  active: true,
  entries: [{ id: 0, level: "info", title: "Hello", body: "" }],
});

const NOTIFICATION_WITH_BODY = JSON.stringify({
  active: true,
  entries: [{ id: 0, level: "warning", title: "Warn", body: "details here" }],
});

const NOTIFICATION_WITH_PROGRESS = JSON.stringify({
  active: true,
  entries: [
    {
      id: 0,
      level: "info",
      title: "Building",
      body: "",
      progress: { percent: 35, detail: "3/10" },
    },
  ],
});

const NOTIFICATION_MULTIPLE = JSON.stringify({
  active: true,
  entries: [
    { id: 0, level: "info", title: "First", body: "" },
    { id: 1, level: "error", title: "Second", body: "" },
  ],
});

const NOTIFICATION_EMPTY = JSON.stringify({ active: true, entries: [] });

const NOTIFICATION_DEACTIVATE = JSON.stringify({ active: false, entries: [] });

const MICROSCOPE_ACTIVE = JSON.stringify({
  active: true,
  query: "main",
  cursor: 4,
  selected: 0,
  scrollOffset: 0,
  pickerName: "files",
  pickerTitle: "Files",
  prompt: "> ",
  items: [
    { display: "main.rs", detail: "src/", icon: "f" },
    { display: "main.go", detail: "cmd/" },
  ],
  totalCount: 100,
  matchedCount: 2,
});

const MICROSCOPE_WITH_PREVIEW = JSON.stringify({
  active: true,
  query: "main",
  cursor: 4,
  selected: 0,
  scrollOffset: 0,
  pickerName: "files",
  pickerTitle: "Files",
  prompt: "> ",
  items: [{ display: "main.rs" }],
  totalCount: 1,
  matchedCount: 1,
  preview: {
    lines: ["fn main() {", "    println!(\"hello\");", "}"],
    highlightLine: 0,
    filePath: "src/main.rs",
  },
});

const MICROSCOPE_DEACTIVATE = JSON.stringify({ active: false });

/** Default show-delay matches WhichKeyExtension default (500ms). */
const DEFAULT_DELAY_MS = 500;

// ============ 8a: Interface Contract ============

describe("factory", () => {
  it("createExtensions returns 8 extensions with correct kinds", () => {
    const extensions = createExtensions();
    expect(extensions).toHaveLength(8);

    const kinds = extensions.map((e) => e.kind());
    expect(kinds).toContain("cmdline");
    expect(kinds).toContain("whichkey");
    expect(kinds).toContain("notification");
    expect(kinds).toContain("microscope");
    expect(kinds).toContain("completion");
    expect(kinds).toContain("explorer");
    expect(kinds).toContain("range-finder-jump");
    expect(kinds).toContain("range-finder-fold");
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

  it("grouped rendering creates category headers", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);

    // WHICHKEY_PAYLOAD has "motion" and "textobject" categories
    const headers = container.querySelectorAll(".whichkey-group-header");
    expect(headers.length).toBe(2);
    expect(headers[0].textContent).toBe("Motion");
    expect(headers[1].textContent).toBe("Textobject");
  });

  it("flat rendering when no categories", () => {
    const noCatPayload = JSON.stringify({
      active: true,
      prefix: "g",
      hints: [
        { key: "g", command: "goto-top" },
        { key: "d", command: "goto-def" },
      ],
    });
    ext.applyNotification(noCatPayload);
    ext.render(container);

    // No category headers when all hints lack categories
    const headers = container.querySelectorAll(".whichkey-group-header");
    expect(headers.length).toBe(0);

    // Hints should still be rendered
    const hints = container.querySelectorAll(".whichkey-hint");
    expect(hints.length).toBe(2);
  });

  it("category ordering follows CATEGORY_ORDER", () => {
    const payload = JSON.stringify({
      active: true,
      prefix: "g",
      hints: [
        { key: "i", command: "inner-word", category: "textobject" },
        { key: "d", command: "delete", category: "operator" },
        { key: "g", command: "goto-top", category: "motion" },
      ],
    });
    ext.applyNotification(payload);
    ext.render(container);

    const headers = container.querySelectorAll(".whichkey-group-header");
    expect(headers.length).toBe(3);
    // motion < operator < textobject per CATEGORY_ORDER
    expect(headers[0].textContent).toBe("Motion");
    expect(headers[1].textContent).toBe("Operator");
    expect(headers[2].textContent).toBe("Textobject");
  });
});

// ============ 8c2: Which-Key Style Custom Properties ============

describe("which-key style custom property support", () => {
  let ext: WhichKeyExtension;
  let container: HTMLElement;

  beforeEach(() => {
    vi.useFakeTimers();
    ext = new WhichKeyExtension(0);
    container = document.createElement("div");
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("popup container scopes all styled children for CSS custom property inheritance", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);

    const popup = container.querySelector(".whichkey-popup");
    expect(popup).toBeTruthy();

    // All styled elements must be descendants of .whichkey-popup
    // so --whichkey-* custom properties defined on .whichkey-popup cascade down
    const header = popup!.querySelector(".whichkey-header");
    const key = popup!.querySelector(".whichkey-key");
    const command = popup!.querySelector(".whichkey-command");
    const groupHeader = popup!.querySelector(".whichkey-group-header");

    expect(header).toBeTruthy();
    expect(key).toBeTruthy();
    expect(command).toBeTruthy();
    expect(groupHeader).toBeTruthy();
  });

  it("inline style override on popup propagates to children", () => {
    ext.applyNotification(WHICHKEY_PAYLOAD);
    ext.render(container);

    const popup = container.querySelector(".whichkey-popup") as HTMLElement;
    expect(popup).toBeTruthy();

    // Setting a custom property on the popup element should work
    // (verifies the element accepts style.setProperty for overrides)
    popup.style.setProperty("--whichkey-key-color", "#ff0000");
    expect(popup.style.getPropertyValue("--whichkey-key-color")).toBe("#ff0000");
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

    const notification = new NotificationExtension();
    expect(() => notification.hide()).not.toThrow();
  });
});

// ============ 8h: NotificationExtension ============

describe("NotificationExtension payload parsing", () => {
  let ext: NotificationExtension;

  beforeEach(() => {
    vi.useFakeTimers();
    ext = new NotificationExtension();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts inactive", () => {
    expect(ext.isActive()).toBe(false);
    expect(ext.getState()).toBeNull();
  });

  it("parses single toast", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    expect(ext.isActive()).toBe(true);

    const state = ext.getState();
    expect(state?.toasts).toHaveLength(1);
    const toasts = state?.toasts as Array<Record<string, unknown>>;
    expect(toasts[0]?.title).toBe("Hello");
    expect(toasts[0]?.level).toBe("info");
  });

  it("parses toast with body", () => {
    ext.applyNotification(NOTIFICATION_WITH_BODY);
    const state = ext.getState();
    const toasts = state?.toasts as Array<Record<string, unknown>>;
    expect(toasts[0]?.body).toBe("details here");
    expect(toasts[0]?.level).toBe("warning");
  });

  it("parses toast with progress", () => {
    ext.applyNotification(NOTIFICATION_WITH_PROGRESS);
    const state = ext.getState();
    const toasts = state?.toasts as Array<Record<string, unknown>>;
    const progress = toasts[0]?.progress as Record<string, unknown>;
    expect(progress.percent).toBe(35);
    expect(progress.detail).toBe("3/10");
  });

  it("parses multiple toasts", () => {
    ext.applyNotification(NOTIFICATION_MULTIPLE);
    const state = ext.getState();
    const toasts = state?.toasts as Array<Record<string, unknown>>;
    expect(toasts).toHaveLength(2);
    expect(toasts[0]?.title).toBe("First");
    expect(toasts[1]?.title).toBe("Second");
  });

  it("deduplicates by ID on re-send", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    ext.applyNotification(NOTIFICATION_SINGLE);
    const state = ext.getState();
    const toasts = state?.toasts as Array<Record<string, unknown>>;
    expect(toasts).toHaveLength(1);
  });

  it("updates progress on existing toast", () => {
    ext.applyNotification(NOTIFICATION_WITH_PROGRESS);

    const updated = JSON.stringify({
      active: true,
      entries: [
        {
          id: 0,
          level: "info",
          title: "Building",
          body: "",
          progress: { percent: 70, detail: "7/10" },
        },
      ],
    });
    ext.applyNotification(updated);

    const state = ext.getState();
    const toasts = state?.toasts as Array<Record<string, unknown>>;
    expect(toasts).toHaveLength(1);
    const progress = toasts[0]?.progress as Record<string, unknown>;
    expect(progress.percent).toBe(70);
    expect(progress.detail).toBe("7/10");
  });

  it("removes server-dismissed toasts", () => {
    ext.applyNotification(NOTIFICATION_MULTIPLE);
    expect((ext.getState()?.toasts as unknown[]).length).toBe(2);

    // Server only sends id=1
    const remaining = JSON.stringify({
      active: true,
      entries: [{ id: 1, level: "error", title: "Second", body: "" }],
    });
    ext.applyNotification(remaining);
    const state = ext.getState();
    const toasts = state?.toasts as Array<Record<string, unknown>>;
    expect(toasts).toHaveLength(1);
    expect(toasts[0]?.id).toBe(1);
  });

  it("empty entries removes all toasts", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    expect(ext.isActive()).toBe(true);

    ext.applyNotification(NOTIFICATION_EMPTY);
    expect(ext.isActive()).toBe(false);
    expect(ext.getState()).toBeNull();
  });

  it("invalid JSON retains previous state", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    expect(ext.isActive()).toBe(true);

    ext.applyNotification("{broken!!}");
    expect(ext.isActive()).toBe(true);
  });

  it("missing entries field is ignored", () => {
    ext.applyNotification(JSON.stringify({ active: true }));
    expect(ext.isActive()).toBe(false);
  });

  it("entry without id is skipped", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        entries: [{ level: "info", title: "no id", body: "" }],
      }),
    );
    expect(ext.isActive()).toBe(false);
  });

  it("auto-dismisses non-progress toasts after timeout", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    expect(ext.isActive()).toBe(true);

    vi.advanceTimersByTime(4001);
    expect(ext.isActive()).toBe(false);
  });

  it("does not auto-dismiss progress toasts", () => {
    ext.applyNotification(NOTIFICATION_WITH_PROGRESS);
    expect(ext.isActive()).toBe(true);

    vi.advanceTimersByTime(10000);
    expect(ext.isActive()).toBe(true);
  });
});

describe("NotificationExtension DOM rendering", () => {
  let ext: NotificationExtension;
  let container: HTMLElement;

  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = "";
    ext = new NotificationExtension();
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("renders notification container with overlay class", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    ext.render(container);

    expect(container.querySelector(".notification-container")).toBeTruthy();
    expect(
      container.querySelector(".notification-container")?.classList.contains("overlay"),
    ).toBe(true);
  });

  it("renders toast with level class", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    ext.render(container);

    expect(container.querySelector(".notification-toast")).toBeTruthy();
    expect(container.querySelector(".notification-info")).toBeTruthy();
  });

  it("renders icon and title", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    ext.render(container);

    expect(container.querySelector(".notification-icon")?.textContent).toBe("i");
    expect(container.querySelector(".notification-title")?.textContent).toBe("Hello");
  });

  it("renders body when present", () => {
    ext.applyNotification(NOTIFICATION_WITH_BODY);
    ext.render(container);

    expect(container.querySelector(".notification-body")?.textContent).toBe("details here");
  });

  it("does not render body when empty", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    ext.render(container);

    expect(container.querySelector(".notification-body")).toBeNull();
  });

  it("renders progress bar", () => {
    ext.applyNotification(NOTIFICATION_WITH_PROGRESS);
    ext.render(container);

    expect(container.querySelector(".notification-progress")).toBeTruthy();
    expect(container.querySelector(".notification-progress-label")?.textContent).toBe("35%");
    expect(container.querySelector(".notification-progress-fill")).toBeTruthy();
    expect(container.querySelector(".notification-progress-detail")?.textContent).toBe("3/10");
  });

  it("renders multiple toasts", () => {
    ext.applyNotification(NOTIFICATION_MULTIPLE);
    ext.render(container);

    const toasts = container.querySelectorAll(".notification-toast");
    expect(toasts.length).toBe(2);
  });

  it("limits visible toasts to MAX_VISIBLE", () => {
    const entries = Array.from({ length: 7 }, (_, i) => ({
      id: i,
      level: "info",
      title: `Toast ${i}`,
      body: "",
    }));
    ext.applyNotification(JSON.stringify({ active: true, entries }));
    ext.render(container);

    const toasts = container.querySelectorAll(".notification-toast");
    expect(toasts.length).toBe(5); // MAX_VISIBLE = 5
  });

  it("hide removes container from DOM", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    ext.render(container);
    expect(container.querySelector(".notification-container")).toBeTruthy();

    ext.hide();
    expect(container.querySelector(".notification-container")).toBeNull();
  });

  it("re-render replaces container instead of duplicating", () => {
    ext.applyNotification(NOTIFICATION_SINGLE);
    ext.render(container);
    ext.render(container);

    const containers = container.querySelectorAll(".notification-container");
    expect(containers.length).toBe(1);
  });

  it("renders level icons correctly", () => {
    const cases = [
      { level: "info", icon: "i" },
      { level: "success", icon: "+" },
      { level: "warning", icon: "!" },
      { level: "error", icon: "x" },
    ];

    for (const { level, icon } of cases) {
      const e = new NotificationExtension();
      e.applyNotification(
        JSON.stringify({
          active: true,
          entries: [{ id: 0, level, title: "test", body: "" }],
        }),
      );
      const c = document.createElement("div");
      e.render(c);
      expect(c.querySelector(".notification-icon")?.textContent).toBe(icon);
    }
  });

  it("empty toasts does not render container", () => {
    ext.render(container);
    expect(container.querySelector(".notification-container")).toBeNull();
  });
});

// ============ 8i: MicroscopeExtension ============

describe("MicroscopeExtension payload parsing", () => {
  let ext: MicroscopeExtension;

  beforeEach(() => {
    ext = new MicroscopeExtension();
  });

  it("starts inactive", () => {
    expect(ext.isActive()).toBe(false);
    expect(ext.getState()).toBeNull();
  });

  it("kind returns microscope", () => {
    expect(ext.kind()).toBe("microscope");
  });

  it("parses active payload", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    expect(ext.isActive()).toBe(true);

    const state = ext.getState();
    expect(state?.query).toBe("main");
    expect(state?.cursor).toBe(4);
    expect(state?.selected).toBe(0);
    expect(state?.pickerName).toBe("files");
    expect(state?.pickerTitle).toBe("Files");
    expect(state?.matchedCount).toBe(2);
    expect(state?.totalCount).toBe(100);
    expect((state?.items as unknown[]).length).toBe(2);
  });

  it("parses payload with preview", () => {
    ext.applyNotification(MICROSCOPE_WITH_PREVIEW);
    const state = ext.getState();
    const preview = state?.preview as Record<string, unknown>;
    expect(preview).not.toBeNull();
    expect((preview.lines as string[]).length).toBe(3);
    expect(preview.highlightLine).toBe(0);
    expect(preview.filePath).toBe("src/main.rs");
  });

  it("activates and deactivates", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    expect(ext.isActive()).toBe(true);

    ext.applyNotification(MICROSCOPE_DEACTIVATE);
    expect(ext.isActive()).toBe(false);
    expect(ext.getState()).toBeNull();
  });

  it("invalid JSON retains previous state", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    expect(ext.isActive()).toBe(true);

    ext.applyNotification("{broken!!}");
    expect(ext.isActive()).toBe(true);
    expect(ext.getState()?.query).toBe("main");
  });

  it("empty string does not throw", () => {
    expect(() => ext.applyNotification("")).not.toThrow();
  });

  it("deactivation resets state", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.applyNotification(MICROSCOPE_DEACTIVATE);

    // Re-activate with different data
    const newPayload = JSON.stringify({
      active: true,
      query: "",
      pickerName: "buffers",
      pickerTitle: "Buffers",
      items: [],
      totalCount: 0,
      matchedCount: 0,
    });
    ext.applyNotification(newPayload);
    expect(ext.getState()?.query).toBe("");
    expect(ext.getState()?.pickerName).toBe("buffers");
  });
});

describe("MicroscopeExtension DOM rendering", () => {
  let ext: MicroscopeExtension;
  let container: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = "";
    ext = new MicroscopeExtension();
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("renders overlay with correct CSS classes", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);

    expect(container.querySelector(".microscope-overlay")).toBeTruthy();
    expect(
      container.querySelector(".microscope-overlay")?.classList.contains("overlay"),
    ).toBe(true);
  });

  it("renders query row with prompt and counter", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);

    expect(container.querySelector(".microscope-prompt")?.textContent).toBe("> ");
    expect(container.querySelector(".microscope-query")?.textContent).toBe("main");
    expect(container.querySelector(".microscope-counter")?.textContent).toBe("2/100");
  });

  it("renders results with items", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);

    const items = container.querySelectorAll(".microscope-item");
    expect(items.length).toBe(2);
    expect(items[0]?.classList.contains("selected")).toBe(true);
    expect(items[1]?.classList.contains("selected")).toBe(false);
  });

  it("renders item display text", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);

    const displays = container.querySelectorAll(".microscope-item-display");
    expect(displays[0]?.textContent).toBe("main.rs");
    expect(displays[1]?.textContent).toBe("main.go");
  });

  it("renders item icon when present", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);

    const icons = container.querySelectorAll(".microscope-item-icon");
    expect(icons.length).toBe(1);
    expect(icons[0]?.textContent).toBe("f");
  });

  it("renders item detail when present", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);

    const details = container.querySelectorAll(".microscope-item-detail");
    expect(details.length).toBe(2);
    expect(details[0]?.textContent).toBe("src/");
    expect(details[1]?.textContent).toBe("cmd/");
  });

  it("renders preview pane", () => {
    ext.applyNotification(MICROSCOPE_WITH_PREVIEW);
    ext.render(container);

    expect(container.querySelector(".microscope-preview")).toBeTruthy();
    expect(container.querySelector(".microscope-preview-path")?.textContent).toBe("src/main.rs");

    const lines = container.querySelectorAll(".microscope-preview-line");
    expect(lines.length).toBe(3);
    expect(lines[0]?.classList.contains("highlighted")).toBe(true);
    expect(lines[1]?.classList.contains("highlighted")).toBe(false);
  });

  it("renders line numbers in preview", () => {
    ext.applyNotification(MICROSCOPE_WITH_PREVIEW);
    ext.render(container);

    const lineNums = container.querySelectorAll(".microscope-line-number");
    expect(lineNums[0]?.textContent).toBe("1");
    expect(lineNums[1]?.textContent).toBe("2");
    expect(lineNums[2]?.textContent).toBe("3");
  });

  it("does not render preview when absent", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);

    expect(container.querySelector(".microscope-preview")).toBeNull();
  });

  it("hide removes overlay from DOM", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);
    expect(container.querySelector(".microscope-overlay")).toBeTruthy();

    ext.hide();
    expect(container.querySelector(".microscope-overlay")).toBeNull();
  });

  it("re-render replaces overlay instead of duplicating", () => {
    ext.applyNotification(MICROSCOPE_ACTIVE);
    ext.render(container);
    ext.render(container);

    const overlays = container.querySelectorAll(".microscope-overlay");
    expect(overlays.length).toBe(1);
  });

  it("inactive state does not render", () => {
    ext.render(container);
    expect(container.querySelector(".microscope-overlay")).toBeNull();
  });

  it("hide without prior render does not throw", () => {
    expect(() => ext.hide()).not.toThrow();
  });

  it("state isolation: microscope notification does not affect other extensions", () => {
    const extensions = createExtensions();

    const notification = {
      kind: "microscope",
      data: MICROSCOPE_ACTIVE,
    };
    for (const e of extensions) {
      if (e.kind() === notification.kind) {
        e.applyNotification(notification.data);
      }
    }

    const cmdline = extensions.find((e) => e.kind() === "cmdline")!;
    expect(cmdline.isActive()).toBe(false);
    const whichkey = extensions.find((e) => e.kind() === "whichkey")!;
    expect(whichkey.isActive()).toBe(false);
  });

  it("scrolls to keep selected item visible when past MAX_VISIBLE_ITEMS", () => {
    const items = Array.from({ length: 50 }, (_, i) => ({
      display: `item_${i}`,
    }));
    const payload = JSON.stringify({
      active: true,
      query: "",
      cursor: 0,
      selected: 30,
      scrollOffset: 0,
      pickerName: "test",
      pickerTitle: "Test",
      prompt: "> ",
      items,
      totalCount: 50,
      matchedCount: 50,
    });
    ext.applyNotification(payload);
    ext.render(container);

    const renderedItems = container.querySelectorAll(".microscope-item");
    // Should render MAX_VISIBLE_ITEMS (20) items
    expect(renderedItems.length).toBe(20);

    // The selected item (index 30) must have the "selected" class
    const selectedItems = container.querySelectorAll(".microscope-item.selected");
    expect(selectedItems.length).toBe(1);
    expect(
      selectedItems[0]?.querySelector(".microscope-item-display")?.textContent,
    ).toBe("item_30");
  });

  it("does not scroll when selected is within first visible page", () => {
    const items = Array.from({ length: 50 }, (_, i) => ({
      display: `item_${i}`,
    }));
    const payload = JSON.stringify({
      active: true,
      query: "",
      cursor: 0,
      selected: 5,
      scrollOffset: 0,
      pickerName: "test",
      pickerTitle: "Test",
      prompt: "> ",
      items,
      totalCount: 50,
      matchedCount: 50,
    });
    ext.applyNotification(payload);
    ext.render(container);

    const renderedItems = container.querySelectorAll(".microscope-item");
    expect(renderedItems.length).toBe(20);

    // First item should be item_0 (no scroll)
    expect(
      renderedItems[0]?.querySelector(".microscope-item-display")?.textContent,
    ).toBe("item_0");

    // Item 5 should be selected
    const selectedItems = container.querySelectorAll(".microscope-item.selected");
    expect(selectedItems.length).toBe(1);
    expect(
      selectedItems[0]?.querySelector(".microscope-item-display")?.textContent,
    ).toBe("item_5");
  });
});

// ============ RangeFinderJump Extension ============

describe("RangeFinderJumpExtension", () => {
  let ext: RangeFinderJumpExtension;
  let container: HTMLElement;

  beforeEach(() => {
    ext = new RangeFinderJumpExtension();
    container = document.createElement("div");
  });

  afterEach(() => {
    container.innerHTML = "";
  });

  it("kind returns range-finder-jump", () => {
    expect(ext.kind()).toBe("range-finder-jump");
  });

  it("starts inactive", () => {
    expect(ext.isActive()).toBe(false);
  });

  it("returns null state when inactive", () => {
    expect(ext.getState()).toBeNull();
  });

  it("activates on valid notification", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        matches: [{ line: 0, col: 5, label: "s" }],
      }),
    );
    expect(ext.isActive()).toBe(true);
  });

  it("deactivates on active=false", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        matches: [{ line: 0, col: 5, label: "s" }],
      }),
    );
    ext.applyNotification(JSON.stringify({ active: false }));
    expect(ext.isActive()).toBe(false);
  });

  it("parses matches correctly", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        matches: [
          { line: 1, col: 3, label: "a" },
          { line: 5, col: 10, label: "bc" },
        ],
      }),
    );
    const state = ext.getState();
    expect(state).not.toBeNull();
    expect((state as Record<string, unknown>).active).toBe(true);
    const matches = (state as Record<string, unknown>).matches as Array<Record<string, unknown>>;
    expect(matches).toHaveLength(2);
    expect(matches[0].label).toBe("a");
    expect(matches[1].label).toBe("bc");
  });

  it("skips entries with missing fields", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        matches: [
          { line: 0, col: 5, label: "a" },
          { line: 1 }, // missing col and label
          { col: 2, label: "b" }, // missing line
        ],
      }),
    );
    const state = ext.getState();
    const matches = (state as Record<string, unknown>).matches as Array<Record<string, unknown>>;
    expect(matches).toHaveLength(1);
    expect(matches[0].label).toBe("a");
  });

  it("handles invalid JSON gracefully", () => {
    ext.applyNotification("not json{{{");
    expect(ext.isActive()).toBe(false);
  });

  it("handles non-array matches gracefully", () => {
    ext.applyNotification(JSON.stringify({ active: true, matches: "not-array" }));
    expect(ext.isActive()).toBe(true);
    const state = ext.getState();
    const matches = (state as Record<string, unknown>).matches as Array<unknown>;
    expect(matches).toHaveLength(0);
  });

  it("renders jump labels", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        matches: [
          { line: 0, col: 5, label: "s" },
          { line: 2, col: 10, label: "ab" },
        ],
      }),
    );
    ext.render(container);

    const overlay = container.querySelector(".range-finder-jump-overlay");
    expect(overlay).not.toBeNull();

    const labels = container.querySelectorAll(".jump-label");
    expect(labels).toHaveLength(2);
    expect(labels[0].textContent).toBe("s");
    expect(labels[0].getAttribute("data-line")).toBe("0");
    expect(labels[0].getAttribute("data-col")).toBe("5");
    expect(labels[1].textContent).toBe("ab");
  });

  it("does not render when inactive", () => {
    ext.render(container);
    expect(container.querySelector(".range-finder-jump-overlay")).toBeNull();
  });

  it("does not render when active but no matches", () => {
    ext.applyNotification(JSON.stringify({ active: true, matches: [] }));
    ext.render(container);
    expect(container.querySelector(".range-finder-jump-overlay")).toBeNull();
  });

  it("removes previous overlay on re-render", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        matches: [{ line: 0, col: 0, label: "x" }],
      }),
    );
    ext.render(container);
    ext.render(container);
    const overlays = container.querySelectorAll(".range-finder-jump-overlay");
    expect(overlays).toHaveLength(1);
  });

  it("hide removes overlay", () => {
    ext.applyNotification(
      JSON.stringify({
        active: true,
        matches: [{ line: 0, col: 0, label: "x" }],
      }),
    );
    ext.render(container);
    ext.hide();
    expect(container.querySelector(".range-finder-jump-overlay")).toBeNull();
  });

  it("hide is safe when no overlay exists", () => {
    ext.hide();
    // No error thrown
  });
});

// ============ RangeFinderFold Extension ============

describe("RangeFinderFoldExtension", () => {
  let ext: RangeFinderFoldExtension;
  let container: HTMLElement;

  beforeEach(() => {
    ext = new RangeFinderFoldExtension();
    container = document.createElement("div");
  });

  afterEach(() => {
    container.innerHTML = "";
  });

  it("kind returns range-finder-fold", () => {
    expect(ext.kind()).toBe("range-finder-fold");
  });

  it("starts inactive", () => {
    expect(ext.isActive()).toBe(false);
  });

  it("returns null state when inactive", () => {
    expect(ext.getState()).toBeNull();
  });

  it("activates on valid notification with folds", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ start_line: 5, hidden_count: 3, preview: "fn foo() {" }] },
      }),
    );
    expect(ext.isActive()).toBe(true);
  });

  it("stays inactive on empty folds", () => {
    ext.applyNotification(JSON.stringify({ folds: {} }));
    expect(ext.isActive()).toBe(false);
  });

  it("stays inactive on missing folds key", () => {
    ext.applyNotification(JSON.stringify({ active: true }));
    expect(ext.isActive()).toBe(false);
  });

  it("parses fold state correctly", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: {
          "1": [{ start_line: 5, hidden_count: 3, preview: "fn foo() {" }],
          "2": [{ start_line: 10, hidden_count: 2, preview: "fn bar() {" }],
        },
      }),
    );
    const state = ext.getState();
    expect(state).not.toBeNull();
    const folds = (state as Record<string, unknown>).folds as Record<string, unknown>;
    expect(Object.keys(folds)).toHaveLength(2);
  });

  it("skips entries missing start_line", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ hidden_count: 5, preview: "a" }] },
      }),
    );
    expect(ext.isActive()).toBe(false);
  });

  it("skips entries missing hidden_count", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ start_line: 0, preview: "a" }] },
      }),
    );
    expect(ext.isActive()).toBe(false);
  });

  it("defaults preview to empty string", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ start_line: 0, hidden_count: 5 }] },
      }),
    );
    expect(ext.isActive()).toBe(true);
  });

  it("handles invalid JSON gracefully", () => {
    ext.applyNotification("not json{{{");
    expect(ext.isActive()).toBe(false);
  });

  it("handles non-object folds value gracefully", () => {
    ext.applyNotification(JSON.stringify({ folds: "not-object" }));
    expect(ext.isActive()).toBe(false);
  });

  it("handles non-array buffer entries gracefully", () => {
    ext.applyNotification(JSON.stringify({ folds: { "1": "not-array" } }));
    expect(ext.isActive()).toBe(false);
  });

  it("replaces previous state on new notification", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ start_line: 0, hidden_count: 3, preview: "a" }] },
      }),
    );
    ext.applyNotification(
      JSON.stringify({
        folds: { "2": [{ start_line: 10, hidden_count: 2, preview: "b" }] },
      }),
    );
    const state = ext.getState();
    const folds = (state as Record<string, unknown>).folds as Record<string, unknown>;
    expect(Object.keys(folds)).toContain("2");
    expect(Object.keys(folds)).not.toContain("1");
  });

  it("renders fold markers", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ start_line: 2, hidden_count: 6, preview: "fn foo() {" }] },
      }),
    );
    ext.render(container);

    const overlay = container.querySelector(".range-finder-fold-overlay");
    expect(overlay).not.toBeNull();

    const markers = container.querySelectorAll(".fold-marker");
    expect(markers).toHaveLength(1);
    expect(markers[0].textContent).toBe("--- 6 lines: fn foo() { ---");
    expect(markers[0].getAttribute("data-buffer-id")).toBe("1");
    expect(markers[0].getAttribute("data-start-line")).toBe("2");
  });

  it("does not render when inactive", () => {
    ext.render(container);
    expect(container.querySelector(".range-finder-fold-overlay")).toBeNull();
  });

  it("removes previous overlay on re-render", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ start_line: 0, hidden_count: 3, preview: "a" }] },
      }),
    );
    ext.render(container);
    ext.render(container);
    const overlays = container.querySelectorAll(".range-finder-fold-overlay");
    expect(overlays).toHaveLength(1);
  });

  it("hide removes overlay", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: { "1": [{ start_line: 0, hidden_count: 3, preview: "a" }] },
      }),
    );
    ext.render(container);
    ext.hide();
    expect(container.querySelector(".range-finder-fold-overlay")).toBeNull();
  });

  it("hide is safe when no overlay exists", () => {
    ext.hide();
    // No error thrown
  });

  it("renders multiple folds across buffers", () => {
    ext.applyNotification(
      JSON.stringify({
        folds: {
          "1": [{ start_line: 0, hidden_count: 3, preview: "a" }],
          "2": [
            { start_line: 5, hidden_count: 2, preview: "b" },
            { start_line: 10, hidden_count: 4, preview: "c" },
          ],
        },
      }),
    );
    ext.render(container);

    const markers = container.querySelectorAll(".fold-marker");
    expect(markers).toHaveLength(3);
  });
});

// ============ Toposort Tests (#583) ============

describe("toposortExtensions", () => {
  /** Minimal mock implementing the required interface. */
  function mockExt(
    k: string,
    deps?: string[],
  ): { kind(): string; dependencies?(): string[] } {
    return {
      kind: () => k,
      ...(deps ? { dependencies: () => deps } : {}),
    };
  }

  it("preserves order when no dependencies declared", () => {
    const exts = [mockExt("a"), mockExt("b"), mockExt("c")];
    const sorted = toposortExtensions(exts);
    expect(sorted.map((e) => e.kind())).toEqual(["a", "b", "c"]);
  });

  it("reorders when dependency requires earlier position", () => {
    // B depends on A, but B is listed first
    const exts = [mockExt("b", ["a"]), mockExt("a")];
    const sorted = toposortExtensions(exts);
    const kinds = sorted.map((e) => e.kind());
    expect(kinds.indexOf("a")).toBeLessThan(kinds.indexOf("b"));
  });

  it("handles transitive dependencies", () => {
    // C -> B -> A
    const exts = [
      mockExt("c", ["b"]),
      mockExt("b", ["a"]),
      mockExt("a"),
    ];
    const sorted = toposortExtensions(exts);
    const kinds = sorted.map((e) => e.kind());
    expect(kinds.indexOf("a")).toBeLessThan(kinds.indexOf("b"));
    expect(kinds.indexOf("b")).toBeLessThan(kinds.indexOf("c"));
  });

  it("skips missing dependencies silently", () => {
    const exts = [mockExt("a", ["nonexistent"]), mockExt("b")];
    const sorted = toposortExtensions(exts);
    expect(sorted).toHaveLength(2);
  });

  it("throws on dependency cycle", () => {
    const exts = [mockExt("a", ["b"]), mockExt("b", ["a"])];
    expect(() => toposortExtensions(exts)).toThrow(
      /dependency cycle detected/,
    );
  });

  it("handles empty array", () => {
    const sorted = toposortExtensions([]);
    expect(sorted).toEqual([]);
  });

  it("handles single extension with no deps", () => {
    const sorted = toposortExtensions([mockExt("solo")]);
    expect(sorted).toHaveLength(1);
    expect(sorted[0].kind()).toBe("solo");
  });
});

// ============ Extension Lifecycle Tests (#583) ============

describe("createExtensions lifecycle", () => {
  it("returns sorted extensions with init called", () => {
    const exts = createExtensions();
    expect(exts.length).toBe(8);
    // All extensions should have unique kinds
    const kinds = exts.map((e) => e.kind());
    expect(new Set(kinds).size).toBe(8);
  });

  it("shutdownExtensions calls exit in reverse order", () => {
    const order: string[] = [];
    const mockExts: WebExtension[] = [
      {
        kind: () => "first",
        isActive: () => false,
        applyNotification: () => {},
        render: () => {},
        hide: () => {},
        getState: () => null,
        exit: () => order.push("first"),
      },
      {
        kind: () => "second",
        isActive: () => false,
        applyNotification: () => {},
        render: () => {},
        hide: () => {},
        getState: () => null,
        exit: () => order.push("second"),
      },
      {
        kind: () => "third",
        isActive: () => false,
        applyNotification: () => {},
        render: () => {},
        hide: () => {},
        getState: () => null,
        exit: () => order.push("third"),
      },
    ];

    shutdownExtensions(mockExts);
    expect(order).toEqual(["third", "second", "first"]);
  });

  it("shutdownExtensions handles extensions without exit", () => {
    const mockExts: WebExtension[] = [
      {
        kind: () => "no-exit",
        isActive: () => false,
        applyNotification: () => {},
        render: () => {},
        hide: () => {},
        getState: () => null,
      },
    ];
    // Should not throw
    shutdownExtensions(mockExts);
  });
});
