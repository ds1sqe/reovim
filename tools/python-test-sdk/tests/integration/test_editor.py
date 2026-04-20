"""Test Editor class lifecycle and basic operations.

Integration tests requiring a running reovim server.
"""

from __future__ import annotations

import pytest

from reovim import Editor
from reovim.capture import Capture, Cursor
from reovim.errors import AssertionError as ReovimAssertionError


class TestEditorLifecycle:
    """Test Editor start/stop and context manager."""

    def test_context_manager_creates_and_cleans_up(self, binary_info):
        """Editor should start on enter and clean up on exit."""
        with Editor() as e:
            # Should be able to query state
            mode = e.mode
            assert mode is not None

        # After exit, internal state should be cleaned
        assert e._server is None
        assert e._tui is None
        assert e._client is None

    def test_nested_editors_independent(self, binary_info):
        """Multiple editors should be independent."""
        with Editor() as e1:
            e1.keys("ieditor one<Esc>")

            with Editor() as e2:
                e2.keys("ieditor two<Esc>")
                assert "editor two" in e2.buffer

            # e1 should still have its content
            assert "editor one" in e1.buffer

    def test_editor_not_started_raises(self, binary_info):
        """Accessing state before start should raise RuntimeError."""
        e = Editor()

        with pytest.raises(RuntimeError, match="not started"):
            _ = e.mode


class TestModeTransitions:
    """Test mode transitions via keys."""

    def test_starts_in_normal_mode(self, editor: Editor):
        """Editor should start in normal mode."""
        editor.assert_mode("normal")

    def test_i_enters_insert_mode(self, editor: Editor):
        """'i' should enter insert mode."""
        editor.keys("i")
        editor.assert_mode("insert")
        editor.keys("<Esc>")

    def test_escape_returns_to_normal(self, editor: Editor):
        """<Esc> should return to normal mode."""
        editor.keys("i")
        editor.keys("<Esc>")
        editor.assert_mode("normal")

    def test_v_enters_visual_mode(self, editor: Editor):
        """'v' should enter visual mode."""
        editor.keys("isome text<Esc>0")
        editor.keys("v")
        mode = editor.mode.lower()
        assert "visual" in mode or "select" in mode


class TestBasicEditing:
    """Test basic editing operations."""

    def test_insert_text(self, editor: Editor):
        """Should insert text with 'i'."""
        editor.keys("iHello World<Esc>")
        editor.assert_buffer("Hello World")

    def test_delete_word(self, editor: Editor):
        """Should delete word with 'dw'."""
        editor.keys("ihello world<Esc>gg0dw")
        editor.assert_buffer("world")

    def test_delete_line(self, editor: Editor):
        """Should delete line with 'dd'."""
        editor.keys("ifirst<CR>second<CR>third<Esc>ggdd")
        buf = editor.buffer
        assert "first" not in buf
        assert "second" in buf

    def test_change_word(self, editor: Editor):
        """Should change word with 'cw'."""
        editor.keys("iold text<Esc>gg0cwnew<Esc>")
        assert "new" in editor.buffer
        assert "old" not in editor.buffer

    def test_yank_and_paste(self, editor: Editor):
        """Should yank and paste with 'yy' and 'p'."""
        editor.keys("iyank me<Esc>ggyyp")
        buf = editor.buffer
        assert buf.count("yank me") == 2


class TestCursorMovement:
    """Test cursor movement commands."""

    def test_gg_goes_to_start(self, editor: Editor):
        """'gg' should go to document start."""
        editor.keys("iline1<CR>line2<CR>line3<Esc>gg")
        editor.assert_cursor(0, 0)

    def test_zero_goes_to_line_start(self, editor: Editor):
        """'0' should go to line start."""
        editor.keys("ihello world<Esc>0")
        cursor = editor.cursor
        assert cursor.col == 0

    def test_w_moves_word_forward(self, editor: Editor):
        """'w' should move to next word."""
        editor.keys("ihello world<Esc>0w")
        cursor = editor.cursor
        # Should be at start of "world"
        assert cursor.col > 0


class TestCaptureFeature:
    """Test capture functionality."""

    def test_capture_returns_capture_object(self, editor: Editor):
        """capture() should return Capture with all state."""
        editor.keys("iCapture test<Esc>")
        snap = editor.capture("test label")

        assert isinstance(snap, Capture)
        assert snap.label == "test label"
        assert "NORMAL" in snap.mode.upper()
        assert "Capture test" in snap.buffer
        assert isinstance(snap.cursor, Cursor)

    def test_capture_has_frame(self, editor: Editor):
        """Capture should include ANSI frame from TUI."""
        editor.keys("iFrame content<Esc>")
        snap = editor.capture("frame test")

        assert len(snap.frame) > 0
        assert isinstance(snap.frame, str)

    def test_capture_to_json(self, editor: Editor):
        """Capture should serialize to JSON."""
        editor.keys("iJSON test<Esc>")
        snap = editor.capture("json test")
        json_str = snap.to_json()

        assert '"label"' in json_str
        assert '"mode"' in json_str
        assert '"buffer"' in json_str


class TestAssertions:
    """Test assertion methods."""

    def test_assert_mode_passes_correct(self, editor: Editor):
        """assert_mode should pass for correct mode."""
        result = editor.assert_mode("normal")
        assert result is editor  # Returns self for chaining

    def test_assert_mode_fails_incorrect(self, editor: Editor):
        """assert_mode should raise for incorrect mode."""
        with pytest.raises(ReovimAssertionError):
            editor.assert_mode("insert")

    def test_assert_mode_case_insensitive(self, editor: Editor):
        """assert_mode should be case-insensitive."""
        editor.assert_mode("NORMAL")
        editor.assert_mode("Normal")
        editor.assert_mode("normal")

    def test_assert_buffer_passes_correct(self, editor: Editor):
        """assert_buffer should pass for correct content."""
        editor.keys("ihello<Esc>")
        result = editor.assert_buffer("hello")
        assert result is editor

    def test_assert_buffer_fails_incorrect(self, editor: Editor):
        """assert_buffer should raise for incorrect content."""
        editor.keys("ihello<Esc>")
        with pytest.raises(ReovimAssertionError):
            editor.assert_buffer("wrong")

    def test_assert_cursor_passes_correct(self, editor: Editor):
        """assert_cursor should pass for correct position."""
        editor.keys("iabc<Esc>0")
        result = editor.assert_cursor(0, 0)
        assert result is editor

    def test_assert_cursor_fails_incorrect(self, editor: Editor):
        """assert_cursor should raise for incorrect position."""
        editor.keys("iabc<Esc>0")
        with pytest.raises(ReovimAssertionError):
            editor.assert_cursor(99, 99)


class TestFluentChaining:
    """Test fluent API chaining."""

    def test_chain_multiple_keys(self, editor: Editor):
        """Should chain multiple keys() calls."""
        result = (
            editor.keys("ihello world<Esc>")
            .keys("gg0")
            .keys("dw")
        )

        assert result is editor
        editor.assert_buffer("world")

    def test_chain_keys_with_assertions(self, editor: Editor):
        """Should chain keys with assertions."""
        (
            editor.keys("ihello world<Esc>")
            .assert_mode("normal")
            .keys("gg0dw")
            .assert_buffer("world")
        )

    def test_type_method_convenience(self, editor: Editor):
        """type() should be a convenience for insert+escape."""
        editor.type("hello world")
        assert "hello world" in editor.buffer
        editor.assert_mode("normal")
