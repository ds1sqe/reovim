#!/usr/bin/env python3
"""Debug visual mode using CLI-only (no TUI) - mimics Rust integration test.

This tests whether visual mode works when:
- Server is running
- Keys are sent directly via CLI (no TUI connected)
"""

import subprocess
import sys
import time
from pathlib import Path

# Add parent to path for development
sys.path.insert(0, str(Path(__file__).parent.parent))

from reovim.discovery import discover_binary, find_free_port, find_project_root
from reovim.process import ServerProcess


def cli_command(binary_path: str, address: str, *args) -> str:
    """Run a CLI command and return output."""
    cmd = [binary_path, "cli", "--grpc", address, *args]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
    if result.returncode != 0:
        print(f"CLI command failed: {' '.join(cmd)}")
        print(f"stderr: {result.stderr}")
        return ""
    return result.stdout


def cli_json(binary_path: str, address: str, *args) -> dict:
    """Run a CLI command with JSON output."""
    import json
    cmd = [binary_path, "cli", "--grpc", address, "--format", "json", *args]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
    if result.returncode != 0:
        print(f"CLI command failed: {' '.join(cmd)}")
        print(f"stderr: {result.stderr}")
        return {}
    try:
        return json.loads(result.stdout)
    except:
        return {}


def main():
    print("\n" + "=" * 60)
    print("  DEBUG: CLI-only Visual Mode (mimics Rust test)")
    print("=" * 60)

    # Discover binary
    base_dir = find_project_root()
    binary = discover_binary(base_dir)
    print(f"Binary: {binary.path}")

    # Start server (no TUI)
    port = find_free_port()
    server = ServerProcess(binary=binary, port=port)
    actual_port = server.start()
    address = f"127.0.0.1:{actual_port}"
    print(f"Server started on {address}")

    time.sleep(0.5)  # Let server initialize

    try:
        # Check server is responding
        print("\n[1] Ping server...")
        cli_command(str(binary.path), address, "ping")
        print("    OK")

        # Create a temp file with content
        temp_file = "/tmp/reovim-debug-cli.txt"
        with open(temp_file, "w") as f:
            f.write("hello world")
        print(f"\n[2] Created temp file: {temp_file}")

        # Open the file using :e command
        print("\n[3] Open file via :e command...")
        cli_command(str(binary.path), address, "keys", f":e {temp_file}<CR>")
        time.sleep(0.2)

        # Check buffer content
        print("\n[4] Check buffer content...")
        data = cli_json(str(binary.path), address, "buffer")
        buffer_content = "\n".join(data.get("lines", []))
        print(f"    Buffer: '{buffer_content}'")

        # Check mode
        mode_data = cli_json(str(binary.path), address, "mode")
        print(f"    Mode: {mode_data.get('display', 'unknown')}")

        # Check cursor
        cursor_data = cli_json(str(binary.path), address, "cursor")
        pos = cursor_data.get("position", {})
        print(f"    Cursor: ({pos.get('line', 0)}, {pos.get('column', 0)})")

        # Now try visual mode
        print("\n[5] Enter visual mode (v)...")
        cli_command(str(binary.path), address, "keys", "v")
        time.sleep(0.1)
        mode_data = cli_json(str(binary.path), address, "mode")
        print(f"    Mode: {mode_data.get('display', 'unknown')}")

        print("\n[6] Extend selection (lll)...")
        cli_command(str(binary.path), address, "keys", "lll")
        time.sleep(0.1)
        cursor_data = cli_json(str(binary.path), address, "cursor")
        pos = cursor_data.get("position", {})
        print(f"    Cursor: ({pos.get('line', 0)}, {pos.get('column', 0)})")

        print("\n[7] Delete selection (d)...")
        cli_command(str(binary.path), address, "keys", "d")
        time.sleep(0.1)

        # Check final state
        print("\n[8] Final state:")
        data = cli_json(str(binary.path), address, "buffer")
        buffer_content = "\n".join(data.get("lines", []))
        print(f"    Buffer: '{buffer_content}'")
        mode_data = cli_json(str(binary.path), address, "mode")
        print(f"    Mode: {mode_data.get('display', 'unknown')}")

        # Verify result
        print("\n" + "-" * 60)
        expected = "o world"  # Delete "hell" (chars 0-3)
        if buffer_content == expected:
            print(f"SUCCESS: Buffer is '{expected}'")
        else:
            print(f"FAILURE: Expected '{expected}', got '{buffer_content}'")
        print("-" * 60)

    finally:
        # Cleanup
        server.stop()
        Path(temp_file).unlink(missing_ok=True)
        print("\nServer stopped.")


if __name__ == "__main__":
    sys.exit(main() or 0)
