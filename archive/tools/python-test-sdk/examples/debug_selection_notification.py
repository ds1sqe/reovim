#!/usr/bin/env python3
"""Debug selection notifications to verify they're being sent."""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from reovim import Editor


def main():
    print("\n" + "=" * 60)
    print("  DEBUG: Selection Notifications")
    print("=" * 60)

    with Editor() as e:
        # Setup: insert text
        print("\n[1] Insert 'hello world'")
        e.keys("ihello world<Esc>0")
        snap1 = e.capture("after insert")
        print(f"    Buffer: '{snap1.buffer}'")
        print(f"    Cursor: {snap1.cursor}")
        print(f"    Mode: {snap1.mode}")

        # Enter visual mode
        print("\n[2] Enter visual mode (v)")
        e.keys("v")
        snap2 = e.capture("after v")
        print(f"    Mode: {snap2.mode}")

        # Check selection via CLI
        print("\n[3] Check selection state via CLI...")
        # Use raw CLI to get selection
        import subprocess
        import json

        binary = str(e._process._binary.path)
        address = f"127.0.0.1:{e._process._port}"

        cmd = [binary, "cli", "--grpc", address, "--format", "json", "selection"]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        print(f"    CLI output: {result.stdout.strip()}")
        if result.stderr:
            print(f"    CLI stderr: {result.stderr.strip()}")

        # Extend selection
        print("\n[4] Extend selection with 'llll'")
        e.keys("llll")
        snap3 = e.capture("after llll")
        print(f"    Cursor: {snap3.cursor}")

        # Check selection again
        print("\n[5] Check selection state again...")
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        print(f"    CLI output: {result.stdout.strip()}")

        # Try to get selection via GetSelection RPC
        print("\n[6] Try GetSelection RPC directly...")
        try:
            sel = e.get_selection()
            print(f"    Selection: {sel}")
        except Exception as ex:
            print(f"    Error getting selection: {ex}")

        # Exit visual mode
        print("\n[7] Exit visual mode with <Esc>")
        e.keys("<Esc>")
        snap4 = e.capture("after escape")
        print(f"    Mode: {snap4.mode}")

        # Check selection cleared
        print("\n[8] Check selection cleared...")
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        print(f"    CLI output: {result.stdout.strip()}")

        print("\n" + "-" * 60)
        print("Done. Check the TUI debug output for 'SelectionChanged' logs.")
        print("-" * 60)


if __name__ == "__main__":
    main()
