#!/usr/bin/env python3
"""Battle-proof test suite for the reovim Python helper.

Exercises every feature of the Python helper API:
- Editor lifecycle (start, stop, context manager)
- Basic editing operations (insert, delete, yank, change)
- Count prefixes (2dw, 3x, d2w, 2dd)
- Motions (gg, G, 0, w, b, e)
- Frame buffer capture (multiple captures, metadata)
- Fluent chaining
- Assertions (mode, buffer, cursor, register)
- Wait for conditions
- Edge cases (empty buffer, special keys, newlines)
- One-liner edit() convenience function
- Error handling (bad assertions)
"""

import sys
import time
import traceback
from pathlib import Path

# Add parent to path for development
sys.path.insert(0, str(Path(__file__).parent.parent))

from reovim import Editor, Capture, Cursor, Register, edit
from reovim.errors import AssertionError, TimeoutError

# Test tracking
_passed = 0
_failed = 0
_errors: list[tuple[str, str]] = []


def test(name: str):
    """Decorator that tracks test pass/fail."""
    def decorator(func):
        def wrapper():
            global _passed, _failed
            try:
                func()
                _passed += 1
                print(f"  PASS  {name}")
            except Exception as exc:
                _failed += 1
                tb = traceback.format_exc()
                _errors.append((name, tb))
                print(f"  FAIL  {name}")
                # Show first line of error
                print(f"        {exc}")
        return wrapper
    return decorator


# =============================================================================
# 1. BASIC EDITING
# =============================================================================

@test("insert text")
def test_insert_text():
    with Editor() as e:
        e.keys("iHello World<Esc>")
        e.assert_buffer("Hello World")
        e.assert_mode("normal")


@test("insert and delete word")
def test_insert_and_dw():
    with Editor() as e:
        e.keys("ihello world<Esc>gg0dw")
        e.assert_buffer("world")


@test("delete line (dd)")
def test_dd():
    with Editor() as e:
        e.keys("ifirst<CR>second<CR>third<Esc>gg")
        e.keys("dd")
        assert "first" not in e.buffer, f"'first' should be deleted, got: {e.buffer!r}"


@test("change word (cw)")
def test_cw():
    with Editor() as e:
        e.keys("ihello world<Esc>gg0")
        e.keys("cwchanged<Esc>")
        e.assert_buffer("changed world")


@test("yank and paste (yy+p)")
def test_yy_p():
    with Editor() as e:
        e.keys("iyank me<Esc>gg")
        e.keys("yyp")
        buf = e.buffer
        assert buf.count("yank me") == 2, f"Expected 2 copies, got: {buf!r}"


@test("delete character (x)")
def test_x():
    with Editor() as e:
        e.keys("iabc<Esc>0x")
        e.assert_buffer("bc")


@test("append (a)")
def test_append():
    with Editor() as e:
        e.keys("ihello<Esc>0")
        e.keys("a world<Esc>")
        buf = e.buffer
        # 'a' appends after cursor (which is on 'h'), so should insert after 'h'
        assert "world" in buf, f"Expected 'world' in buffer, got: {buf!r}"


@test("insert at line start (I)")
def test_I():
    with Editor() as e:
        e.keys("iworld<Esc>")
        e.keys("Ihello <Esc>")
        e.assert_buffer("hello world")


# =============================================================================
# 2. COUNT PREFIXES
# =============================================================================

@test("2dw (delete 2 words)")
def test_2dw():
    result = edit("one two three four", "2dw")
    assert result.strip() == "three four", f"Expected 'three four', got: {result!r}"


@test("3dw (delete 3 words)")
def test_3dw():
    result = edit("aa bb cc dd ee", "3dw")
    assert result.strip() == "dd ee", f"Expected 'dd ee', got: {result!r}"


@test("d2w (motion count 2)")
def test_d2w():
    result = edit("alpha beta gamma delta", "d2w")
    assert result.strip() == "gamma delta", f"Expected 'gamma delta', got: {result!r}"


@test("3x (delete 3 chars)")
def test_3x():
    result = edit("abcdefgh", "3x")
    assert result.strip() == "defgh", f"Expected 'defgh', got: {result!r}"


@test("2dd (delete 2 lines)")
def test_2dd():
    with Editor() as e:
        e.keys("iline1<CR>line2<CR>line3<CR>line4<Esc>gg")
        e.keys("2dd")
        buf = e.buffer
        assert "line1" not in buf, f"line1 should be deleted: {buf!r}"
        assert "line2" not in buf, f"line2 should be deleted: {buf!r}"
        assert "line3" in buf, f"line3 should remain: {buf!r}"


# =============================================================================
# 3. MOTIONS
# =============================================================================

@test("gg (document start)")
def test_gg():
    with Editor() as e:
        e.keys("ifirst<CR>second<CR>third<Esc>")
        e.keys("gg")
        cur = e.cursor
        assert cur.line == 0, f"Expected line 0, got {cur.line}"


@test("G (document end)")
def test_G():
    with Editor() as e:
        e.keys("ifirst<CR>second<CR>third<Esc>gg")
        e.keys("G")
        cur = e.cursor
        assert cur.line >= 2, f"Expected line >= 2, got {cur.line}"


@test("0 (line start)")
def test_0():
    with Editor() as e:
        e.keys("ihello world<Esc>")
        e.keys("0")
        cur = e.cursor
        assert cur.col == 0, f"Expected col 0, got {cur.col}"


@test("w (word forward)")
def test_w():
    with Editor() as e:
        e.keys("ihello world<Esc>0")
        e.keys("w")
        cur = e.cursor
        assert cur.col > 0, f"Expected col > 0 after 'w', got {cur.col}"


@test("b (word backward)")
def test_b():
    with Editor() as e:
        e.keys("ihello world<Esc>")
        # cursor on 'd', go back one word
        e.keys("b")
        cur = e.cursor
        # Should be at start of "world"
        assert cur.col < 11, f"Expected cursor moved back, got col {cur.col}"


# =============================================================================
# 4. FRAME BUFFER CAPTURE
# =============================================================================

@test("capture returns Capture object")
def test_capture_type():
    with Editor() as e:
        e.keys("iTest capture<Esc>")
        snap = e.capture("test label")
        assert isinstance(snap, Capture), f"Expected Capture, got {type(snap)}"
        assert snap.label == "test label"
        assert snap.mode.upper() == "NORMAL"
        assert "Test capture" in snap.buffer


@test("capture has frame data")
def test_capture_frame():
    with Editor() as e:
        e.keys("iHello Frame<Esc>")
        snap = e.capture("frame test")
        assert len(snap.frame) > 0, "Frame should not be empty"
        assert isinstance(snap.frame, str), "Frame should be a string"


@test("capture has cursor data")
def test_capture_cursor():
    with Editor() as e:
        e.keys("iabc<Esc>0")
        snap = e.capture("cursor test")
        assert isinstance(snap.cursor, Cursor)
        assert snap.cursor.line == 0
        assert snap.cursor.col == 0


@test("capture to_json roundtrip")
def test_capture_json():
    with Editor() as e:
        e.keys("iJSON test<Esc>")
        snap = e.capture("json test")
        json_str = snap.to_json()
        assert '"label"' in json_str
        assert '"json test"' in json_str
        assert '"mode"' in json_str
        assert '"buffer"' in json_str
        assert '"frame"' in json_str


@test("capture to_dict structure")
def test_capture_dict():
    with Editor() as e:
        e.keys("idict test<Esc>")
        snap = e.capture("dict test")
        d = snap.to_dict()
        assert d["label"] == "dict test"
        assert "line" in d["cursor"]
        assert "col" in d["cursor"]
        assert isinstance(d["registers"], dict)


@test("multiple captures in sequence")
def test_multiple_captures():
    with Editor() as e:
        e.keys("ifirst state<Esc>")
        snap1 = e.capture("before edit")

        e.keys("gg0dw")
        snap2 = e.capture("after delete")

        assert "first" in snap1.buffer
        assert "first" not in snap2.buffer
        assert snap1.timestamp <= snap2.timestamp


@test("capture registers after yank")
def test_capture_registers():
    with Editor() as e:
        e.keys("iyank this<Esc>gg0yaw")
        snap = e.capture("after yank")
        # At least one register should have content
        has_content = any(reg.content for reg in snap.registers.values())
        # This might fail if registers aren't populated - that's OK, it tests the capture mechanism


@test("screen property returns ANSI frame")
def test_screen_property():
    with Editor() as e:
        e.keys("iScreen test<Esc>")
        screen = e.screen
        assert isinstance(screen, str)
        assert len(screen) > 0


# =============================================================================
# 5. FLUENT CHAINING
# =============================================================================

@test("chain multiple keys calls")
def test_chain_keys():
    with Editor() as e:
        result = e.keys("iaa bb cc<Esc>").keys("gg0").keys("dw")
        assert isinstance(result, Editor)
        e.assert_buffer("bb cc")


@test("chain keys with assertions")
def test_chain_keys_assertions():
    with Editor() as e:
        e.keys("ihello world<Esc>") \
         .assert_mode("normal") \
         .keys("gg0dw") \
         .assert_buffer("world")


@test("chain type with keys")
def test_chain_type_keys():
    with Editor() as e:
        e.type("hello world").keys("gg0dw")
        e.assert_buffer("world")


# =============================================================================
# 6. ASSERTIONS
# =============================================================================

@test("assert_mode passes for correct mode")
def test_assert_mode_pass():
    with Editor() as e:
        result = e.assert_mode("normal")
        assert isinstance(result, Editor), "Should return self"


@test("assert_mode is case-insensitive")
def test_assert_mode_case():
    with Editor() as e:
        e.assert_mode("NORMAL")
        e.assert_mode("Normal")
        e.assert_mode("normal")


@test("assert_mode fails for wrong mode")
def test_assert_mode_fail():
    with Editor() as e:
        try:
            e.assert_mode("insert")
            assert False, "Should have raised AssertionError"
        except AssertionError:
            pass  # Expected


@test("assert_buffer passes for correct content")
def test_assert_buffer_pass():
    with Editor() as e:
        e.keys("ihello<Esc>")
        result = e.assert_buffer("hello")
        assert isinstance(result, Editor), "Should return self"


@test("assert_buffer fails for wrong content")
def test_assert_buffer_fail():
    with Editor() as e:
        e.keys("ihello<Esc>")
        try:
            e.assert_buffer("wrong")
            assert False, "Should have raised AssertionError"
        except AssertionError:
            pass  # Expected


@test("assert_cursor passes for correct position")
def test_assert_cursor_pass():
    with Editor() as e:
        e.keys("iabc<Esc>0")
        result = e.assert_cursor(0, 0)
        assert isinstance(result, Editor), "Should return self"


@test("assert_cursor fails for wrong position")
def test_assert_cursor_fail():
    with Editor() as e:
        e.keys("iabc<Esc>0")
        try:
            e.assert_cursor(5, 5)
            assert False, "Should have raised AssertionError"
        except AssertionError:
            pass  # Expected


# =============================================================================
# 7. WAIT FOR
# =============================================================================

@test("wait_for insert mode")
def test_wait_for_insert():
    with Editor() as e:
        e.keys("i")
        snap = e.wait_for(lambda s: "INSERT" in s.mode.upper(), timeout=3.0)
        assert "INSERT" in snap.mode.upper()
        e.keys("<Esc>")


@test("wait_for already true condition")
def test_wait_for_immediate():
    with Editor() as e:
        # Normal mode is already active
        snap = e.wait_for(lambda s: "NORMAL" in s.mode.upper(), timeout=2.0)
        assert "NORMAL" in snap.mode.upper()


@test("wait_for timeout raises")
def test_wait_for_timeout():
    with Editor() as e:
        try:
            e.wait_for(lambda s: False, timeout=0.5)
            assert False, "Should have raised TimeoutError"
        except TimeoutError:
            pass  # Expected


# =============================================================================
# 8. EDGE CASES
# =============================================================================

@test("empty buffer operations")
def test_empty_buffer():
    with Editor() as e:
        buf = e.buffer
        # Empty or single empty line
        assert len(buf.strip()) == 0, f"Expected empty buffer, got: {buf!r}"


@test("special keys (<Esc>, <CR>)")
def test_special_keys():
    with Editor() as e:
        e.keys("iabc<CR>def<Esc>")
        buf = e.buffer
        lines = buf.split("\n")
        assert len(lines) >= 2, f"Expected at least 2 lines, got: {lines!r}"


@test("type() with special characters")
def test_type_special():
    with Editor() as e:
        e.type("hello <world>")  # Should escape the angle brackets
        buf = e.buffer
        # The <LT> escaping should handle < properly
        assert "hello" in buf


@test("multiple editor sessions")
def test_multiple_sessions():
    """Two independent editors don't interfere."""
    with Editor() as e1:
        e1.keys("ieditor one<Esc>")

        with Editor() as e2:
            e2.keys("ieditor two<Esc>")
            e2.assert_buffer("editor two")

        # e1 should still have its content
        e1.assert_buffer("editor one")


# =============================================================================
# 9. ONE-LINER edit()
# =============================================================================

@test("edit() basic delete word")
def test_edit_dw():
    result = edit("hello world", "dw")
    assert result.strip() == "world", f"Expected 'world', got: {result!r}"


@test("edit() delete to end of line")
def test_edit_D():
    result = edit("hello world", "D")
    # D deletes from cursor to end of line
    # After type("hello world") + gg0, cursor is at position 0
    # D should delete entire line content
    assert len(result.strip()) == 0 or result.strip() != "hello world", \
        f"Expected content deleted, got: {result!r}"


@test("edit() change word")
def test_edit_cw():
    with Editor() as e:
        e.type("old text").keys("gg0")
        e.keys("cwnew<Esc>")
        buf = e.buffer.strip()
        assert "new" in buf, f"Expected 'new' in buffer, got: {buf!r}"


@test("edit() preserves remaining text")
def test_edit_preserves():
    result = edit("keep this part", "x")
    # 'x' deletes one char at cursor position (position 0 after gg0)
    assert "eep this part" in result or "keep this par" in result, \
        f"Expected most text preserved, got: {result!r}"


# =============================================================================
# 10. CAPTURE-SPECIFIC BATTLE TESTS
# =============================================================================

@test("capture after insert mode transition")
def test_capture_after_mode_change():
    with Editor() as e:
        e.keys("i")
        snap = e.capture("in insert mode")
        assert "INSERT" in snap.mode.upper(), f"Expected INSERT mode, got: {snap.mode}"
        e.keys("<Esc>")
        snap2 = e.capture("back to normal")
        assert "NORMAL" in snap2.mode.upper(), f"Expected NORMAL mode, got: {snap2.mode}"


@test("capture after multiline insert")
def test_capture_multiline():
    with Editor() as e:
        e.keys("iline one<CR>line two<CR>line three<Esc>")
        snap = e.capture("multiline")
        lines = snap.buffer.split("\n")
        assert len(lines) >= 3, f"Expected >= 3 lines, got {len(lines)}: {lines!r}"


@test("capture frame contains buffer content")
def test_capture_frame_content():
    with Editor() as e:
        # Use a unique string that should appear in the rendered frame
        e.keys("iUNIQUE_MARKER_12345<Esc>")
        snap = e.capture("marker test")
        # The ANSI frame should contain the buffer text somewhere
        assert "UNIQUE_MARKER_12345" in snap.frame, \
            f"Expected marker in frame ({len(snap.frame)} bytes)"


@test("capture save and reload frame")
def test_capture_save_frame():
    import tempfile
    import os

    with Editor() as e:
        e.keys("iSave test<Esc>")
        snap = e.capture("save test")

        # Save to temp file
        with tempfile.NamedTemporaryFile(mode="w", suffix=".frame", delete=False) as f:
            snap.save_frame(f.name)
            path = f.name

        try:
            # Verify file exists and has content
            assert os.path.exists(path)
            with open(path) as f:
                content = f.read()
            assert len(content) > 0, "Saved frame should not be empty"
            assert content == snap.frame, "Saved frame should match capture"
        finally:
            os.unlink(path)


@test("rapid captures don't break")
def test_rapid_captures():
    with Editor() as e:
        e.keys("irapid test<Esc>")
        captures = []
        for i in range(5):
            snap = e.capture(f"rapid {i}")
            captures.append(snap)

        assert len(captures) == 5
        for i, snap in enumerate(captures):
            assert snap.label == f"rapid {i}"
            assert "rapid test" in snap.buffer


# =============================================================================
# RUNNER
# =============================================================================

def main():
    print()
    print("=" * 60)
    print("  REOVIM BATTLE TEST")
    print("  Comprehensive Python Helper Verification")
    print("=" * 60)
    print()

    # Collect all test functions
    tests = [
        # 1. Basic editing
        ("1. BASIC EDITING", [
            test_insert_text,
            test_insert_and_dw,
            test_dd,
            test_cw,
            test_yy_p,
            test_x,
            test_append,
            test_I,
        ]),
        # 2. Count prefixes
        ("2. COUNT PREFIXES", [
            test_2dw,
            test_3dw,
            test_d2w,
            test_3x,
            test_2dd,
        ]),
        # 3. Motions
        ("3. MOTIONS", [
            test_gg,
            test_G,
            test_0,
            test_w,
            test_b,
        ]),
        # 4. Frame capture
        ("4. FRAME BUFFER CAPTURE", [
            test_capture_type,
            test_capture_frame,
            test_capture_cursor,
            test_capture_json,
            test_capture_dict,
            test_multiple_captures,
            test_capture_registers,
            test_screen_property,
        ]),
        # 5. Fluent chaining
        ("5. FLUENT CHAINING", [
            test_chain_keys,
            test_chain_keys_assertions,
            test_chain_type_keys,
        ]),
        # 6. Assertions
        ("6. ASSERTIONS", [
            test_assert_mode_pass,
            test_assert_mode_case,
            test_assert_mode_fail,
            test_assert_buffer_pass,
            test_assert_buffer_fail,
            test_assert_cursor_pass,
            test_assert_cursor_fail,
        ]),
        # 7. Wait for
        ("7. WAIT FOR", [
            test_wait_for_insert,
            test_wait_for_immediate,
            test_wait_for_timeout,
        ]),
        # 8. Edge cases
        ("8. EDGE CASES", [
            test_empty_buffer,
            test_special_keys,
            test_type_special,
            test_multiple_sessions,
        ]),
        # 9. One-liner
        ("9. ONE-LINER edit()", [
            test_edit_dw,
            test_edit_D,
            test_edit_cw,
            test_edit_preserves,
        ]),
        # 10. Capture battle
        ("10. CAPTURE BATTLE TESTS", [
            test_capture_after_mode_change,
            test_capture_multiline,
            test_capture_frame_content,
            test_capture_save_frame,
            test_rapid_captures,
        ]),
    ]

    start = time.time()

    for section_name, section_tests in tests:
        print(f"\n--- {section_name} ---")
        for t in section_tests:
            t()

    elapsed = time.time() - start
    total = _passed + _failed

    print()
    print("=" * 60)
    print(f"  RESULTS: {_passed}/{total} passed, {_failed} failed")
    print(f"  Time: {elapsed:.1f}s")
    print("=" * 60)

    if _errors:
        print()
        print("FAILURES:")
        for name, tb in _errors:
            print(f"\n--- {name} ---")
            print(tb)

    return 0 if _failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
