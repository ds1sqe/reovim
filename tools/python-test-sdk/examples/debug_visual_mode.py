#!/usr/bin/env python3
"""Debug visual mode selection step by step.

This script tests visual mode operations methodically to understand
why the selection-based delete isn't working.
"""

import sys
from pathlib import Path

# Add parent to path for development
sys.path.insert(0, str(Path(__file__).parent.parent))

from reovim import Editor


def debug_visual_delete():
    """Debug visual mode delete step by step."""
    print("\n" + "=" * 60)
    print("  DEBUG: Visual Mode Delete")
    print("=" * 60)

    with Editor() as e:
        # Step 1: Set up buffer with "hello world"
        print("\n[Step 1] Insert 'hello world' and return to normal mode")
        e.keys("ihello world<Esc>")
        snap1 = e.capture("after insert")
        print(f"  Mode:   {snap1.mode}")
        print(f"  Buffer: '{snap1.buffer}'")
        print(f"  Cursor: {snap1.cursor}")

        # Step 2: Go to beginning of line
        print("\n[Step 2] Go to beginning of line (0)")
        e.keys("0")
        snap2 = e.capture("after 0")
        print(f"  Mode:   {snap2.mode}")
        print(f"  Buffer: '{snap2.buffer}'")
        print(f"  Cursor: {snap2.cursor}")

        # Step 3: Enter visual mode
        print("\n[Step 3] Enter visual mode (v)")
        e.keys("v")
        snap3 = e.capture("after v")
        print(f"  Mode:   {snap3.mode}")
        print(f"  Buffer: '{snap3.buffer}'")
        print(f"  Cursor: {snap3.cursor}")

        # Step 4: Extend selection by 4 characters
        print("\n[Step 4] Extend selection (llll)")
        e.keys("llll")
        snap4 = e.capture("after llll")
        print(f"  Mode:   {snap4.mode}")
        print(f"  Buffer: '{snap4.buffer}'")
        print(f"  Cursor: {snap4.cursor}")

        # Step 5: Delete selection
        print("\n[Step 5] Delete selection (d)")
        e.keys("d")
        snap5 = e.capture("after d")
        print(f"  Mode:   {snap5.mode}")
        print(f"  Buffer: '{snap5.buffer}'")
        print(f"  Cursor: {snap5.cursor}")

        # Verify result
        print("\n" + "-" * 60)
        expected = " world"  # "hello" deleted, " world" remains
        if snap5.buffer == expected:
            print(f"SUCCESS: Buffer is '{expected}'")
        else:
            print(f"FAILURE: Expected '{expected}', got '{snap5.buffer}'")
        print("-" * 60)


def debug_simple_delete_word():
    """Debug simple dw command as baseline."""
    print("\n" + "=" * 60)
    print("  DEBUG: Simple dw (delete word) - Baseline")
    print("=" * 60)

    with Editor() as e:
        print("\n[Setup] Insert 'hello world'")
        e.keys("ihello world<Esc>0")
        snap1 = e.capture("setup")
        print(f"  Buffer: '{snap1.buffer}', Cursor: {snap1.cursor}")

        print("\n[Action] dw")
        e.keys("dw")
        snap2 = e.capture("after dw")
        print(f"  Buffer: '{snap2.buffer}', Cursor: {snap2.cursor}")

        expected = "world"
        if snap2.buffer == expected:
            print(f"\nSUCCESS: dw works correctly")
        else:
            print(f"\nFAILURE: Expected '{expected}', got '{snap2.buffer}'")


def main():
    print("\n" + "+" + "-" * 58 + "+")
    print("|" + " " * 12 + "VISUAL MODE DEBUG SESSION" + " " * 21 + "|")
    print("+" + "-" * 58 + "+")

    try:
        # First verify basic deletion works
        debug_simple_delete_word()

        # Then test visual mode
        debug_visual_delete()

        print("\n" + "=" * 60)
        print("  DEBUG SESSION COMPLETE")
        print("=" * 60)

    except Exception as ex:
        print(f"\nError: {ex}")
        import traceback
        traceback.print_exc()
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
