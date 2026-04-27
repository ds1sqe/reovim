#!/usr/bin/env python3
"""Debug paste operation to see if buffer updates are notified."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from reovim import Editor


def main():
    print("\n" + "=" * 60)
    print("  DEBUG: Paste Operation")
    print("=" * 60)

    with Editor() as e:
        # Setup: insert text and yank it
        print("\n[1] Insert 'hello' and yank it")
        e.keys("ihello<Esc>0yaw")
        snap1 = e.capture("after yank")
        print(f"    Buffer: '{snap1.buffer}'")
        print(f"    Cursor: {snap1.cursor}")

        # Go to end of line
        print("\n[2] Go to end of line")
        e.keys("$")
        snap2 = e.capture("at end")
        print(f"    Cursor: {snap2.cursor}")

        # Paste after
        print("\n[3] Paste after cursor (p)")
        e.keys("p")
        snap3 = e.capture("after paste")
        print(f"    Buffer: '{snap3.buffer}'")
        print(f"    Cursor: {snap3.cursor}")

        # Verify
        print("\n" + "-" * 60)
        expected = "hellohello"  # Should have pasted "hello" after
        if snap3.buffer == expected:
            print(f"SUCCESS: Paste worked, buffer is '{expected}'")
        else:
            print(f"RESULT: Buffer is '{snap3.buffer}'")
            print(f"         (Expected '{expected}')")
        print("-" * 60)


if __name__ == "__main__":
    main()
