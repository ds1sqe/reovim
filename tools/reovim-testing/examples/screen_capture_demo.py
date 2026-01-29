#!/usr/bin/env python3
"""
Screen Capture Demo - Real TUI Frame Buffer

Uses the headless TUI to capture the exact frame buffer, including
ANSI colors and formatting. Demonstrates the full state capture API.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))
from reovim_test import TestSession

BINARY = str(Path(__file__).parent.parent.parent.parent / "target/release/reovim")


def demo():
    print("\n" + "=" * 70)
    print("  REOVIM TUI FRAME BUFFER CAPTURE DEMO")
    print("=" * 70)

    with TestSession(binary_path=BINARY).start("hello world") as t:
        # Initial state with frame capture
        print("\n▸ INITIAL STATE (via capture())")
        print("-" * 70)
        state = t.capture("initial")
        state.print_frame()  # New method: prints ANSI frame

        print("\n▸ STRUCTURED DATA")
        print("-" * 70)
        print(f"  Mode:   {state.mode}")
        print(f"  Buffer: \"{state.buffer}\"")
        print(f"  Cursor: line={state.cursor.line}, col={state.cursor.col}")
        print(f"  Frame:  {len(state.frame)} bytes")

        # Delete word
        print("\n▸ AFTER 'dw' (delete word)")
        print("-" * 70)
        t.keys("dw")
        after_dw = t.capture("after dw")
        after_dw.print_frame()

        # Insert text
        print("\n▸ AFTER 'iNew text: <Esc>' (insert)")
        print("-" * 70)
        t.keys("iNew text: <Esc>")
        after_insert = t.capture("after insert")
        after_insert.print_frame()

        # Show JSON output (useful for LLM agents)
        print("\n▸ JSON OUTPUT (for LLM agents)")
        print("-" * 70)
        final = t.capture("final")
        # Show truncated JSON (frame is large)
        import json
        data = final.to_dict()
        data["frame"] = data["frame"][:100] + "..." if len(data["frame"]) > 100 else data["frame"]
        print(json.dumps(data, indent=2))

        # Save frame for comparison
        print("\n▸ SAVING FRAMES")
        print("-" * 70)
        state.save_frame("/tmp/frame_initial.ansi")
        after_dw.save_frame("/tmp/frame_after_dw.ansi")
        after_insert.save_frame("/tmp/frame_after_insert.ansi")
        print("  Saved: /tmp/frame_initial.ansi")
        print("  Saved: /tmp/frame_after_dw.ansi")
        print("  Saved: /tmp/frame_after_insert.ansi")

    print("\n" + "=" * 70)
    print("  DEMO COMPLETE")
    print("=" * 70 + "\n")


if __name__ == "__main__":
    demo()
