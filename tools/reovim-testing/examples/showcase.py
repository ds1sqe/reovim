#!/usr/bin/env python3
"""Reovim Showcase - Elegant LLM Testing.

This demonstrates the elegant API inspired by type-bridge patterns:
- Zero-config (auto-discovers binary)
- Fluent chaining (every method returns self)
- Context managers (automatic cleanup)
- Type-safe dataclasses (Capture, Cursor, Register)
- One-liner convenience (edit function)
"""

import sys
from pathlib import Path

# Add parent to path for development
sys.path.insert(0, str(Path(__file__).parent.parent))

from reovim import Editor, Capture, edit


def demo_one_liner():
    """Demonstrate one-liner edit function."""
    print("\n" + "=" * 60)
    print("  1. ONE-LINER MAGIC")
    print("=" * 60)
    print()
    print(">>> from reovim import edit")
    print(">>> result = edit('hello world', 'dw')")
    print()

    result = edit("hello world", "dw")
    print(f"Result: '{result}'")
    print()
    print("One line: content in, keys applied, result out.")


def demo_fluent_api():
    """Demonstrate fluent chaining API."""
    print("\n" + "=" * 60)
    print("  2. FLUENT CHAINING")
    print("=" * 60)
    print()
    print(">>> with Editor() as e:")
    print("...     e.keys('iThe quick brown fox<Esc>')")
    print("...      .keys('gg0dw')")
    print("...      .assert_buffer('quick brown fox')")
    print("...      .assert_mode('normal')")
    print()

    with Editor() as e:
        e.keys("iThe quick brown fox<Esc>") \
         .keys("gg0dw") \
         .assert_buffer("quick brown fox") \
         .assert_mode("normal")

        print(f"Buffer: '{e.buffer}'")
        print(f"Mode: {e.mode}")
        print(f"Cursor: {e.cursor}")
    print()
    print("Every method returns self - chain everything.")


def demo_capture():
    """Demonstrate state capture for LLM."""
    print("\n" + "=" * 60)
    print("  3. STATE CAPTURE FOR LLM")
    print("=" * 60)
    print()
    print(">>> snap = e.capture('after delete')")
    print(">>> snap.print_frame()  # ANSI output")
    print(">>> snap.to_json()      # JSON for LLM")
    print()

    with Editor() as e:
        e.keys("iHello, LLM Agent!<Esc>")
        snap = e.capture("after insert")

        print(f"Capture: {snap}")
        print()
        print("JSON (truncated):")
        json_str = snap.to_json()
        # Show first 500 chars
        print(json_str[:500] + "..." if len(json_str) > 500 else json_str)
        print()
        print("Frame buffer captured - perfect for LLM visual verification!")


def demo_assertions():
    """Demonstrate assertions for testing."""
    print("\n" + "=" * 60)
    print("  4. ASSERTIONS")
    print("=" * 60)
    print()
    print(">>> e.assert_mode('normal')")
    print(">>> e.assert_buffer('expected')")
    print(">>> e.assert_cursor(0, 5)")
    print(">>> e.assert_register('\"', 'yanked')")
    print()

    with Editor() as e:
        e.keys("itest<Esc>yaw")  # Yank a word

        # All return self for chaining
        e.assert_mode("normal") \
         .assert_buffer("test")

        print("All assertions passed!")
        print()
        print("Assertions return self - chain them too.")


def demo_wait_for():
    """Demonstrate waiting for conditions."""
    print("\n" + "=" * 60)
    print("  5. WAIT FOR CONDITIONS")
    print("=" * 60)
    print()
    print(">>> e.keys('i').wait_for(lambda s: 'INSERT' in s.mode)")
    print()

    with Editor() as e:
        e.keys("i")
        snap = e.wait_for(lambda s: "INSERT" in s.mode.upper())
        print(f"Waited until INSERT mode: {snap.mode}")
        e.keys("<Esc>")
    print()
    print("Async-friendly - wait for any condition.")


def main():
    """Run all demonstrations."""
    print()
    print("+" + "-" * 58 + "+")
    print("|" + " " * 15 + "REOVIM - Elegant LLM Testing" + " " * 15 + "|")
    print("|" + " " * 10 + "Zero-config, Type-safe, Just Works" + " " * 14 + "|")
    print("+" + "-" * 58 + "+")

    try:
        demo_one_liner()
        demo_fluent_api()
        demo_capture()
        demo_assertions()
        demo_wait_for()

        print("\n" + "=" * 60)
        print("  SHOWCASE COMPLETE")
        print("=" * 60)
        print()
        print("What we demonstrated:")
        print("  - edit() one-liner for quick tests")
        print("  - Fluent chaining with context manager")
        print("  - Rich state capture (frame, mode, cursor, buffer)")
        print("  - Assertions that chain")
        print("  - Wait for async conditions")
        print()
        print("API inspired by type-bridge elegance patterns.")
        print("=" * 60)

    except Exception as e:
        print(f"\nError: {e}")
        print("\nMake sure reovim-new is built with gRPC:")
        print("  cargo build --release -p reovim-bin --features grpc")
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
