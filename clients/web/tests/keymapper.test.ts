/**
 * Keymapper Tests
 *
 * Tests for browser keyboard event to vim notation conversion.
 * P0 CRITICAL: These tests verify correct key mapping which is fundamental
 * to the editor's operation.
 */

import { describe, it, expect } from "vitest";
import { browserKeyToVim, shouldPreventDefault } from "../src/keymapper.js";

/**
 * Helper to create a mock KeyboardEvent-like object.
 */
function mockKeyEvent(
  key: string,
  modifiers: {
    ctrlKey?: boolean;
    altKey?: boolean;
    shiftKey?: boolean;
    metaKey?: boolean;
  } = {}
): KeyboardEvent {
  return {
    key,
    ctrlKey: modifiers.ctrlKey ?? false,
    altKey: modifiers.altKey ?? false,
    shiftKey: modifiers.shiftKey ?? false,
    metaKey: modifiers.metaKey ?? false,
  } as KeyboardEvent;
}

describe("browserKeyToVim", () => {
  describe("character keys", () => {
    it("converts lowercase letters", () => {
      expect(browserKeyToVim(mockKeyEvent("a"))).toBe("a");
      expect(browserKeyToVim(mockKeyEvent("z"))).toBe("z");
      expect(browserKeyToVim(mockKeyEvent("m"))).toBe("m");
    });

    it("converts uppercase letters (shifted)", () => {
      expect(browserKeyToVim(mockKeyEvent("A", { shiftKey: true }))).toBe("A");
      expect(browserKeyToVim(mockKeyEvent("Z", { shiftKey: true }))).toBe("Z");
    });

    it("converts digits", () => {
      expect(browserKeyToVim(mockKeyEvent("0"))).toBe("0");
      expect(browserKeyToVim(mockKeyEvent("5"))).toBe("5");
      expect(browserKeyToVim(mockKeyEvent("9"))).toBe("9");
    });

    it("converts symbols", () => {
      expect(browserKeyToVim(mockKeyEvent("!"))).toBe("!");
      expect(browserKeyToVim(mockKeyEvent("@"))).toBe("@");
      expect(browserKeyToVim(mockKeyEvent("#"))).toBe("#");
      expect(browserKeyToVim(mockKeyEvent("$"))).toBe("$");
      expect(browserKeyToVim(mockKeyEvent("%"))).toBe("%");
      expect(browserKeyToVim(mockKeyEvent("^"))).toBe("^");
      expect(browserKeyToVim(mockKeyEvent("&"))).toBe("&");
      expect(browserKeyToVim(mockKeyEvent("*"))).toBe("*");
      expect(browserKeyToVim(mockKeyEvent("("))).toBe("(");
      expect(browserKeyToVim(mockKeyEvent(")"))).toBe(")");
    });

    it("converts punctuation", () => {
      expect(browserKeyToVim(mockKeyEvent("."))).toBe(".");
      expect(browserKeyToVim(mockKeyEvent(","))).toBe(",");
      expect(browserKeyToVim(mockKeyEvent(";"))).toBe(";");
      expect(browserKeyToVim(mockKeyEvent(":"))).toBe(":");
      expect(browserKeyToVim(mockKeyEvent("'"))).toBe("'");
      expect(browserKeyToVim(mockKeyEvent('"'))).toBe('"');
    });

    it("converts brackets", () => {
      expect(browserKeyToVim(mockKeyEvent("["))).toBe("[");
      expect(browserKeyToVim(mockKeyEvent("]"))).toBe("]");
      expect(browserKeyToVim(mockKeyEvent("{"))).toBe("{");
      expect(browserKeyToVim(mockKeyEvent("}"))).toBe("}");
    });
  });

  describe("special keys", () => {
    it("converts Escape to <Esc>", () => {
      expect(browserKeyToVim(mockKeyEvent("Escape"))).toBe("<Esc>");
    });

    it("converts Enter/Return to <CR>", () => {
      expect(browserKeyToVim(mockKeyEvent("Enter"))).toBe("<CR>");
      expect(browserKeyToVim(mockKeyEvent("Return"))).toBe("<CR>");
    });

    it("converts Tab to <Tab>", () => {
      expect(browserKeyToVim(mockKeyEvent("Tab"))).toBe("<Tab>");
    });

    it("converts Backspace to <BS>", () => {
      expect(browserKeyToVim(mockKeyEvent("Backspace"))).toBe("<BS>");
    });

    it("converts Delete to <Del>", () => {
      expect(browserKeyToVim(mockKeyEvent("Delete"))).toBe("<Del>");
    });

    it("converts Insert to <Insert>", () => {
      expect(browserKeyToVim(mockKeyEvent("Insert"))).toBe("<Insert>");
    });

    it("converts navigation keys", () => {
      expect(browserKeyToVim(mockKeyEvent("Home"))).toBe("<Home>");
      expect(browserKeyToVim(mockKeyEvent("End"))).toBe("<End>");
      expect(browserKeyToVim(mockKeyEvent("PageUp"))).toBe("<PageUp>");
      expect(browserKeyToVim(mockKeyEvent("PageDown"))).toBe("<PageDown>");
    });

    it("converts arrow keys", () => {
      expect(browserKeyToVim(mockKeyEvent("ArrowUp"))).toBe("<Up>");
      expect(browserKeyToVim(mockKeyEvent("ArrowDown"))).toBe("<Down>");
      expect(browserKeyToVim(mockKeyEvent("ArrowLeft"))).toBe("<Left>");
      expect(browserKeyToVim(mockKeyEvent("ArrowRight"))).toBe("<Right>");
    });

    it("converts Space to <Space>", () => {
      expect(browserKeyToVim(mockKeyEvent(" "))).toBe("<Space>");
    });

    it("converts < to <lt>", () => {
      expect(browserKeyToVim(mockKeyEvent("<"))).toBe("<lt>");
    });

    it("converts backslash to <Bslash>", () => {
      expect(browserKeyToVim(mockKeyEvent("\\"))).toBe("<Bslash>");
    });

    it("converts pipe to <Bar>", () => {
      expect(browserKeyToVim(mockKeyEvent("|"))).toBe("<Bar>");
    });

    it("converts function keys", () => {
      expect(browserKeyToVim(mockKeyEvent("F1"))).toBe("<F1>");
      expect(browserKeyToVim(mockKeyEvent("F5"))).toBe("<F5>");
      expect(browserKeyToVim(mockKeyEvent("F12"))).toBe("<F12>");
    });
  });

  describe("modifier keys", () => {
    it("returns null for modifier-only keys", () => {
      expect(browserKeyToVim(mockKeyEvent("Control"))).toBeNull();
      expect(browserKeyToVim(mockKeyEvent("Alt"))).toBeNull();
      expect(browserKeyToVim(mockKeyEvent("Shift"))).toBeNull();
      expect(browserKeyToVim(mockKeyEvent("Meta"))).toBeNull();
    });

    it("converts Ctrl+key to <C-key>", () => {
      expect(browserKeyToVim(mockKeyEvent("a", { ctrlKey: true }))).toBe(
        "<C-a>"
      );
      expect(browserKeyToVim(mockKeyEvent("w", { ctrlKey: true }))).toBe(
        "<C-w>"
      );
      expect(browserKeyToVim(mockKeyEvent("z", { ctrlKey: true }))).toBe(
        "<C-z>"
      );
    });

    it("converts Alt+key to <A-key>", () => {
      expect(browserKeyToVim(mockKeyEvent("a", { altKey: true }))).toBe(
        "<A-a>"
      );
      expect(browserKeyToVim(mockKeyEvent("x", { altKey: true }))).toBe(
        "<A-x>"
      );
    });

    it("converts Meta+key to <C-key> (Mac Cmd treated as Ctrl)", () => {
      expect(browserKeyToVim(mockKeyEvent("a", { metaKey: true }))).toBe(
        "<C-a>"
      );
      expect(browserKeyToVim(mockKeyEvent("s", { metaKey: true }))).toBe(
        "<C-s>"
      );
    });

    it("converts Ctrl+Shift+key to <C-S-key>", () => {
      expect(
        browserKeyToVim(mockKeyEvent("a", { ctrlKey: true, shiftKey: true }))
      ).toBe("<C-S-a>");
    });

    it("converts Alt+Shift+key to <A-S-key>", () => {
      expect(
        browserKeyToVim(mockKeyEvent("a", { altKey: true, shiftKey: true }))
      ).toBe("<A-S-a>");
    });

    it("converts Ctrl+Alt+key to <C-A-key>", () => {
      expect(
        browserKeyToVim(mockKeyEvent("a", { ctrlKey: true, altKey: true }))
      ).toBe("<C-A-a>");
    });

    it("converts Ctrl+special key with Shift to <C-S-special>", () => {
      expect(
        browserKeyToVim(mockKeyEvent("Tab", { ctrlKey: true, shiftKey: true }))
      ).toBe("<C-S-Tab>");
    });

    it("converts Shift+special keys correctly", () => {
      expect(browserKeyToVim(mockKeyEvent("Tab", { shiftKey: true }))).toBe(
        "<S-Tab>"
      );
      expect(browserKeyToVim(mockKeyEvent("Enter", { shiftKey: true }))).toBe(
        "<S-CR>"
      );
      expect(browserKeyToVim(mockKeyEvent("Escape", { shiftKey: true }))).toBe(
        "<S-Esc>"
      );
    });
  });

  describe("edge cases", () => {
    it("ignores unknown multi-character keys", () => {
      expect(browserKeyToVim(mockKeyEvent("Dead"))).toBeNull();
      expect(browserKeyToVim(mockKeyEvent("Unidentified"))).toBeNull();
    });

    it("does not add Shift modifier for regular shifted characters", () => {
      // Uppercase letters don't need <S-> prefix
      expect(browserKeyToVim(mockKeyEvent("A", { shiftKey: true }))).toBe("A");
      // Shifted symbols don't need <S-> prefix
      expect(browserKeyToVim(mockKeyEvent("!", { shiftKey: true }))).toBe("!");
      expect(browserKeyToVim(mockKeyEvent("@", { shiftKey: true }))).toBe("@");
    });

    it("handles Ctrl+bracket keys", () => {
      expect(browserKeyToVim(mockKeyEvent("[", { ctrlKey: true }))).toBe(
        "<C-[>"
      );
      expect(browserKeyToVim(mockKeyEvent("]", { ctrlKey: true }))).toBe(
        "<C-]>"
      );
    });
  });
});

describe("shouldPreventDefault", () => {
  it("prevents Escape", () => {
    expect(shouldPreventDefault(mockKeyEvent("Escape"))).toBe(true);
  });

  it("prevents Tab", () => {
    expect(shouldPreventDefault(mockKeyEvent("Tab"))).toBe(true);
  });

  it("prevents Backspace", () => {
    expect(shouldPreventDefault(mockKeyEvent("Backspace"))).toBe(true);
  });

  it("prevents Ctrl+letter combinations", () => {
    expect(shouldPreventDefault(mockKeyEvent("a", { ctrlKey: true }))).toBe(
      true
    );
    expect(shouldPreventDefault(mockKeyEvent("s", { ctrlKey: true }))).toBe(
      true
    );
    expect(shouldPreventDefault(mockKeyEvent("z", { ctrlKey: true }))).toBe(
      true
    );
  });

  it("prevents Ctrl+bracket combinations", () => {
    expect(shouldPreventDefault(mockKeyEvent("[", { ctrlKey: true }))).toBe(
      true
    );
    expect(shouldPreventDefault(mockKeyEvent("]", { ctrlKey: true }))).toBe(
      true
    );
    expect(shouldPreventDefault(mockKeyEvent("\\", { ctrlKey: true }))).toBe(
      true
    );
  });

  it("prevents Meta+key combinations (Mac)", () => {
    expect(shouldPreventDefault(mockKeyEvent("s", { metaKey: true }))).toBe(
      true
    );
    expect(shouldPreventDefault(mockKeyEvent("a", { metaKey: true }))).toBe(
      true
    );
  });

  it("prevents Alt+key combinations", () => {
    expect(shouldPreventDefault(mockKeyEvent("a", { altKey: true }))).toBe(
      true
    );
    expect(shouldPreventDefault(mockKeyEvent("x", { altKey: true }))).toBe(
      true
    );
  });

  it("does not prevent regular character keys", () => {
    expect(shouldPreventDefault(mockKeyEvent("a"))).toBe(false);
    expect(shouldPreventDefault(mockKeyEvent("1"))).toBe(false);
    expect(shouldPreventDefault(mockKeyEvent("."))).toBe(false);
  });

  it("does not prevent Shift+character (no Ctrl/Alt)", () => {
    expect(shouldPreventDefault(mockKeyEvent("A", { shiftKey: true }))).toBe(
      false
    );
  });
});

// ============================================================================
// WebKeymapper Tests (Browser Keyboard Policy)
// ============================================================================

import { WebKeymapper } from "../src/keymapper.js";

/**
 * Helper to create a mock KeyboardEvent with preventDefault.
 */
function mockKeyEventWithPreventDefault(
  key: string,
  modifiers: {
    ctrlKey?: boolean;
    altKey?: boolean;
    shiftKey?: boolean;
    metaKey?: boolean;
  } = {}
): KeyboardEvent {
  return {
    key,
    ctrlKey: modifiers.ctrlKey ?? false,
    altKey: modifiers.altKey ?? false,
    shiftKey: modifiers.shiftKey ?? false,
    metaKey: modifiers.metaKey ?? false,
    preventDefault: () => {},
  } as KeyboardEvent;
}

describe("WebKeymapper", () => {
  describe("leader key sequences", () => {
    it("translates \\w to <C-w>", () => {
      const keymapper = new WebKeymapper();

      // Press backslash - should return null (waiting for next key)
      const leaderResult = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("\\")
      );
      expect(leaderResult).toBeNull();
      expect(keymapper.isLeaderPending()).toBe(true);

      // Press w - should return <C-w>
      const wResult = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("w")
      );
      expect(wResult).toBe("<C-w>");
      expect(keymapper.isLeaderPending()).toBe(false);
    });

    it("translates \\t to <C-t>", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("t")
      );

      expect(result).toBe("<C-t>");
    });

    it("translates \\n to <C-n>", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("n")
      );

      expect(result).toBe("<C-n>");
    });

    it("translates \\d to <C-d> (half page down)", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("d")
      );

      expect(result).toBe("<C-d>");
    });

    it("translates \\u to <C-u> (half page up)", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("u")
      );

      expect(result).toBe("<C-u>");
    });

    it("translates \\r to <C-r> (redo)", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("r")
      );

      expect(result).toBe("<C-r>");
    });

    it("is case-insensitive for leader mappings", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("W")
      );

      expect(result).toBe("<C-w>");
    });
  });

  describe("leader cancellation", () => {
    it("cancels leader on Escape", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      expect(keymapper.isLeaderPending()).toBe(true);

      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("Escape")
      );

      expect(result).toBeNull();
      expect(keymapper.isLeaderPending()).toBe(false);
    });

    it("passes through unmapped keys after leader", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      // 'q' is not in the leader map
      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("q")
      );

      // Should pass through the key normally
      expect(result).toBe("q");
      expect(keymapper.isLeaderPending()).toBe(false);
    });
  });

  describe("normal mode behavior", () => {
    it("passes through regular keys", () => {
      const keymapper = new WebKeymapper();

      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("j")
      );

      expect(result).toBe("j");
      expect(keymapper.isLeaderPending()).toBe(false);
    });

    it("passes through Ctrl+key combinations", () => {
      const keymapper = new WebKeymapper();

      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("a", { ctrlKey: true })
      );

      expect(result).toBe("<C-a>");
    });

    it("ignores modifier-only presses", () => {
      const keymapper = new WebKeymapper();

      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("Control")
      );

      expect(result).toBeNull();
    });

    it("does not trigger leader with Ctrl+backslash", () => {
      const keymapper = new WebKeymapper();

      const result = keymapper.handleKeyEvent(
        mockKeyEventWithPreventDefault("\\", { ctrlKey: true })
      );

      // Should be treated as Ctrl+\ not leader
      expect(result).toBe("<C-Bslash>");
      expect(keymapper.isLeaderPending()).toBe(false);
    });
  });

  describe("callbacks", () => {
    it("calls onLeaderStart when leader pressed", () => {
      let called = false;
      const keymapper = new WebKeymapper({
        onLeaderStart: () => {
          called = true;
        },
      });

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));

      expect(called).toBe(true);
    });

    it("calls onLeaderEnd when sequence completes", () => {
      let called = false;
      const keymapper = new WebKeymapper({
        onLeaderEnd: () => {
          called = true;
        },
      });

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("w"));

      expect(called).toBe(true);
    });

    it("calls onLeaderEnd when leader cancelled", () => {
      let called = false;
      const keymapper = new WebKeymapper({
        onLeaderEnd: () => {
          called = true;
        },
      });

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("Escape"));

      expect(called).toBe(true);
    });
  });

  describe("state management", () => {
    it("starts in normal state", () => {
      const keymapper = new WebKeymapper();
      expect(keymapper.getState()).toBe("normal");
    });

    it("reset() returns to normal state", () => {
      const keymapper = new WebKeymapper();

      keymapper.handleKeyEvent(mockKeyEventWithPreventDefault("\\"));
      expect(keymapper.isLeaderPending()).toBe(true);

      keymapper.reset();
      expect(keymapper.isLeaderPending()).toBe(false);
      expect(keymapper.getState()).toBe("normal");
    });
  });
});
