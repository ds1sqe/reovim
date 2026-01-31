#!/usr/bin/env python3
"""
Capture Relay Demo - Proves CLI→Server→TUI→Server→CLI flow works.

This script demonstrates the complete capture relay system:
1. Starts a server
2. Connects a headless TUI
3. Sends keys to modify the buffer
4. Captures the screen via CLI (which relays through server to TUI)
5. Shows the captured frame
"""

import subprocess
import time
import sys
import os
import signal

# Find the binary
BINARY = os.path.join(
    os.path.dirname(__file__),
    "..", "..", "..",
    "target", "release", "reovim-new"
)

if not os.path.exists(BINARY):
    # Try debug build
    BINARY = os.path.join(
        os.path.dirname(__file__),
        "..", "..", "..",
        "target", "debug", "reovim-new"
    )

def run_demo():
    print("\n" + "=" * 70)
    print("  CAPTURE RELAY DEMO: CLI → Server → TUI → Server → CLI")
    print("=" * 70)

    server_proc = None
    tui_proc = None
    port = 14000

    try:
        # Step 1: Start server
        print("\n[1] Starting server...")
        server_proc = subprocess.Popen(
            [BINARY, "server", "--grpc", str(port)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        time.sleep(1)  # Wait for server to start
        print(f"    Server started on port {port}")

        # Step 2: Connect headless TUI
        print("\n[2] Connecting headless TUI...")
        tui_proc = subprocess.Popen(
            [BINARY, "tui", "--grpc", f"127.0.0.1:{port}", "--headless"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        time.sleep(1)  # Wait for TUI to connect
        print("    Headless TUI connected")

        # Step 3: Send some keys to create content
        print("\n[3] Sending keys to create content...")
        result = subprocess.run(
            [BINARY, "cli", "--grpc", f"127.0.0.1:{port}", "keys", "iHello, LLM Agent!<Esc>"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        print(f"    Keys sent: {result.stdout.strip()}")
        time.sleep(0.5)

        # Add another line
        result = subprocess.run(
            [BINARY, "cli", "--grpc", f"127.0.0.1:{port}", "keys", "oThis is a capture test.<Esc>"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        print(f"    More keys: {result.stdout.strip()}")
        time.sleep(0.5)

        # Step 4: Capture via CLI (this triggers the relay!)
        print("\n[4] Capturing screen via CLI relay...")
        print("    Flow: CLI → GetScreenContent RPC → Server")
        print("           Server → capture_request notification → TUI")
        print("           TUI → SubmitCaptureResponse RPC → Server")
        print("           Server → response → CLI")

        result = subprocess.run(
            [BINARY, "cli", "--grpc", f"127.0.0.1:{port}", "capture", "--capture-format", "plain_text"],
            capture_output=True,
            text=True,
            timeout=10,
        )

        if result.returncode == 0:
            print("\n[5] CAPTURED FRAME (plain_text):")
            print("-" * 70)
            # Show first 20 lines of the capture
            lines = result.stdout.split('\n')[:20]
            for line in lines:
                print(f"    {line}")
            if len(result.stdout.split('\n')) > 20:
                print("    ...")
            print("-" * 70)
        else:
            print(f"\n[ERROR] Capture failed: {result.stderr}")
            return False

        # Step 5: Capture with ANSI colors
        print("\n[6] Capturing with ANSI colors...")
        result = subprocess.run(
            [BINARY, "cli", "--grpc", f"127.0.0.1:{port}", "capture", "--capture-format", "raw_ansi"],
            capture_output=True,
            text=True,
            timeout=10,
        )

        if result.returncode == 0:
            print("\n    CAPTURED FRAME (raw_ansi) - with colors:")
            print("-" * 70)
            # Show first 10 lines
            lines = result.stdout.split('\n')[:10]
            for line in lines:
                print(f"    {line}")
            print("-" * 70)
            print(f"\n    Total frame size: {len(result.stdout)} bytes")
        else:
            print(f"\n[ERROR] ANSI capture failed: {result.stderr}")

        # Step 6: JSON format capture
        print("\n[7] Capturing as JSON (with metadata)...")
        result = subprocess.run(
            [BINARY, "cli", "--grpc", f"127.0.0.1:{port}", "--format", "json", "capture"],
            capture_output=True,
            text=True,
            timeout=10,
        )

        if result.returncode == 0:
            import json
            try:
                data = json.loads(result.stdout)
                print(f"    Width: {data.get('width')}")
                print(f"    Height: {data.get('height')}")
                print(f"    Format: {data.get('format')}")
                print(f"    Content length: {len(data.get('content', ''))} chars")
            except json.JSONDecodeError:
                print(f"    Raw: {result.stdout[:200]}...")

        print("\n" + "=" * 70)
        print("  DEMO COMPLETE - Capture relay is working!")
        print("=" * 70 + "\n")
        return True

    except subprocess.TimeoutExpired:
        print("\n[ERROR] Command timed out - capture relay may not be working")
        return False
    except Exception as e:
        print(f"\n[ERROR] {e}")
        return False
    finally:
        # Cleanup
        if tui_proc:
            tui_proc.terminate()
            try:
                tui_proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                tui_proc.kill()

        if server_proc:
            server_proc.terminate()
            try:
                server_proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                server_proc.kill()


if __name__ == "__main__":
    success = run_demo()
    sys.exit(0 if success else 1)
