/**
 * Core CLM Types Tests (#650)
 *
 * Tests for value types, discriminated unions, and factory functions.
 */

import { describe, it, expect } from "vitest";
import {
  formatVersion,
  probeSuccess,
  probeDefer,
  probeFailed,
  initFailed,
  exitFailed,
  notificationParse,
  otherError,
  rect,
  INSETS_ZERO,
  defaultStyle,
} from "../src/core/types.js";
import type {
  Version,
  ProbeResult,
  ClientModuleState,
  Style,
  OptionValue,
  Rect,
} from "../src/core/types.js";

describe("Version", () => {
  it("formats as major.minor.patch", () => {
    const v: Version = { major: 1, minor: 2, patch: 3 };
    expect(formatVersion(v)).toBe("1.2.3");
  });

  it("formats zero version", () => {
    const v: Version = { major: 0, minor: 0, patch: 0 };
    expect(formatVersion(v)).toBe("0.0.0");
  });
});

describe("ProbeResult", () => {
  it("creates success", () => {
    const result = probeSuccess();
    expect(result.status).toBe("success");
  });

  it("creates defer with reason", () => {
    const result = probeDefer("dependency not ready");
    expect(result.status).toBe("defer");
    if (result.status === "defer") {
      expect(result.reason).toBe("dependency not ready");
    }
  });

  it("creates failed with error", () => {
    const result = probeFailed("missing feature");
    expect(result.status).toBe("failed");
    if (result.status === "failed") {
      expect(result.error).toBe("missing feature");
    }
  });

  it("discriminates correctly", () => {
    const results: ProbeResult[] = [
      probeSuccess(),
      probeDefer("wait"),
      probeFailed("boom"),
    ];

    const statuses = results.map((r) => r.status);
    expect(statuses).toEqual(["success", "defer", "failed"]);
  });
});

describe("ClientModuleError", () => {
  it("initFailed creates prefixed message", () => {
    const err = initFailed("bad config");
    expect(err.message).toBe("init failed: bad config");
    expect(err.source).toBeUndefined();
  });

  it("exitFailed includes source error", () => {
    const source = new Error("underlying");
    const err = exitFailed("cleanup failed", source);
    expect(err.message).toBe("exit failed: cleanup failed");
    expect(err.source).toBe(source);
  });

  it("notificationParse creates prefixed message", () => {
    const err = notificationParse("invalid JSON");
    expect(err.message).toBe("notification parse: invalid JSON");
  });

  it("otherError passes message through", () => {
    const err = otherError("something went wrong");
    expect(err.message).toBe("something went wrong");
  });
});

describe("ClientModuleState", () => {
  it("represents string states", () => {
    const states: ClientModuleState[] = ["loaded", "initializing", "running"];
    expect(states).toHaveLength(3);
  });

  it("represents failed state with message", () => {
    const state: ClientModuleState = { failed: "init error" };
    expect(typeof state).toBe("object");
    if (typeof state === "object") {
      expect(state.failed).toBe("init error");
    }
  });
});

describe("Rect", () => {
  it("creates with factory function", () => {
    const r: Rect = rect(10, 20, 100, 50);
    expect(r.x).toBe(10);
    expect(r.y).toBe(20);
    expect(r.width).toBe(100);
    expect(r.height).toBe(50);
  });
});

describe("Insets", () => {
  it("ZERO is all zeros", () => {
    expect(INSETS_ZERO.top).toBe(0);
    expect(INSETS_ZERO.bottom).toBe(0);
    expect(INSETS_ZERO.left).toBe(0);
    expect(INSETS_ZERO.right).toBe(0);
  });

  it("ZERO is frozen", () => {
    expect(Object.isFrozen(INSETS_ZERO)).toBe(true);
  });
});

describe("Style", () => {
  it("defaultStyle returns empty object", () => {
    const s: Style = defaultStyle();
    expect(s.fg).toBeUndefined();
    expect(s.bg).toBeUndefined();
    expect(s.bold).toBeUndefined();
  });

  it("accepts all optional fields", () => {
    const s: Style = {
      fg: "#ff0000",
      bg: "#000000",
      bold: true,
      italic: true,
      underline: true,
      strikethrough: true,
      dim: true,
      reverse: true,
    };
    expect(s.fg).toBe("#ff0000");
    expect(s.bold).toBe(true);
  });
});

describe("OptionValue", () => {
  it("discriminates bool", () => {
    const v: OptionValue = { kind: "bool", value: true };
    expect(v.kind).toBe("bool");
    if (v.kind === "bool") {
      expect(v.value).toBe(true);
    }
  });

  it("discriminates integer", () => {
    const v: OptionValue = { kind: "integer", value: 42 };
    expect(v.kind).toBe("integer");
    if (v.kind === "integer") {
      expect(v.value).toBe(42);
    }
  });

  it("discriminates string", () => {
    const v: OptionValue = { kind: "string", value: "hello" };
    expect(v.kind).toBe("string");
    if (v.kind === "string") {
      expect(v.value).toBe("hello");
    }
  });
});
