"""Test capture dataclasses.

Tests the Cursor, Register, and Capture dataclass methods.
"""

from __future__ import annotations

import json
import pytest
import time
from pathlib import Path
from tempfile import NamedTemporaryFile

from reovim.capture import Cursor, Register, Capture


class TestCursor:
    """Test Cursor dataclass."""

    def test_creation(self):
        """Should create cursor with line and col."""
        cursor = Cursor(line=5, col=10)

        assert cursor.line == 5
        assert cursor.col == 10

    def test_immutable(self):
        """Should be frozen (immutable)."""
        cursor = Cursor(line=0, col=0)

        with pytest.raises(AttributeError):
            cursor.line = 1  # type: ignore

    def test_str_format(self):
        """String should show line and col."""
        cursor = Cursor(line=3, col=7)

        s = str(cursor)

        assert "3" in s
        assert "7" in s
        assert "line" in s.lower()
        assert "col" in s.lower()

    def test_equality(self):
        """Equal cursors should be equal."""
        c1 = Cursor(line=1, col=2)
        c2 = Cursor(line=1, col=2)
        c3 = Cursor(line=1, col=3)

        assert c1 == c2
        assert c1 != c3

    def test_hashable(self):
        """Should be usable in sets and dicts."""
        c1 = Cursor(line=1, col=2)
        c2 = Cursor(line=1, col=2)

        # Should be able to use as dict key
        d = {c1: "value"}
        assert d[c2] == "value"

        # Should be able to use in set
        s = {c1, c2}
        assert len(s) == 1  # Same cursor


class TestRegister:
    """Test Register dataclass."""

    def test_creation(self):
        """Should create register with name and content."""
        reg = Register(name="a", content="hello")

        assert reg.name == "a"
        assert reg.content == "hello"
        assert reg.type == "char"  # default

    def test_custom_type(self):
        """Should accept custom yank type."""
        reg = Register(name="b", content="line\n", type="line")

        assert reg.type == "line"

    def test_immutable(self):
        """Should be frozen (immutable)."""
        reg = Register(name="a", content="test")

        with pytest.raises(AttributeError):
            reg.content = "new"  # type: ignore

    def test_str_short_content(self):
        """String should show full content if short."""
        reg = Register(name="a", content="hello")

        s = str(reg)

        assert "a" in s
        assert "hello" in s

    def test_str_long_content_truncated(self):
        """String should truncate long content."""
        long_content = "x" * 50
        reg = Register(name="b", content=long_content)

        s = str(reg)

        assert "..." in s
        assert len(s) < 100  # Should be truncated

    def test_equality(self):
        """Equal registers should be equal."""
        r1 = Register(name="a", content="x", type="char")
        r2 = Register(name="a", content="x", type="char")
        r3 = Register(name="a", content="y", type="char")

        assert r1 == r2
        assert r1 != r3


class TestCapture:
    """Test Capture dataclass."""

    @pytest.fixture
    def sample_capture(self) -> Capture:
        """Create a sample capture for testing."""
        return Capture(
            label="test",
            frame="\x1b[32mHello\x1b[0m",
            mode="NORMAL",
            cursor=Cursor(line=0, col=0),
            buffer="hello world",
            registers={"a": Register(name="a", content="yanked")},
            timestamp=1000.0,
        )

    def test_creation(self, sample_capture: Capture):
        """Should create capture with all fields."""
        assert sample_capture.label == "test"
        assert sample_capture.mode == "NORMAL"
        assert sample_capture.cursor.line == 0
        assert sample_capture.buffer == "hello world"
        assert "a" in sample_capture.registers

    def test_immutable(self, sample_capture: Capture):
        """Should be frozen (immutable)."""
        with pytest.raises(AttributeError):
            sample_capture.mode = "INSERT"  # type: ignore

    def test_default_timestamp(self):
        """Should auto-generate timestamp if not provided."""
        before = time.time()
        capture = Capture(
            label="test",
            frame="",
            mode="NORMAL",
            cursor=Cursor(line=0, col=0),
            buffer="",
        )
        after = time.time()

        assert before <= capture.timestamp <= after

    def test_default_empty_registers(self):
        """Should default to empty registers dict."""
        capture = Capture(
            label="test",
            frame="",
            mode="NORMAL",
            cursor=Cursor(line=0, col=0),
            buffer="",
        )

        assert capture.registers == {}

    def test_to_dict(self, sample_capture: Capture):
        """Should convert to dictionary."""
        d = sample_capture.to_dict()

        assert d["label"] == "test"
        assert d["mode"] == "NORMAL"
        assert d["cursor"] == {"line": 0, "col": 0}
        assert d["buffer"] == "hello world"
        assert d["timestamp"] == 1000.0
        assert "a" in d["registers"]
        assert d["registers"]["a"]["content"] == "yanked"

    def test_to_json(self, sample_capture: Capture):
        """Should convert to valid JSON string."""
        json_str = sample_capture.to_json()

        # Should be valid JSON
        parsed = json.loads(json_str)

        assert parsed["label"] == "test"
        assert parsed["mode"] == "NORMAL"

    def test_to_json_with_indent(self, sample_capture: Capture):
        """Should respect indent parameter."""
        json_no_indent = sample_capture.to_json(indent=None)
        json_with_indent = sample_capture.to_json(indent=4)

        # Indented version should be longer (has whitespace)
        assert len(json_with_indent) > len(json_no_indent)

    def test_str_summary(self, sample_capture: Capture):
        """String should be human-readable summary."""
        s = str(sample_capture)

        assert "test" in s
        assert "NORMAL" in s
        assert "0:0" in s  # cursor
        assert "11 chars" in s or "11" in s  # buffer length
        assert "frame" in s.lower()

    def test_save_frame(self, sample_capture: Capture, tmp_path: Path):
        """Should save frame to file."""
        file_path = tmp_path / "frame.txt"

        sample_capture.save_frame(str(file_path))

        assert file_path.exists()
        content = file_path.read_text()
        assert content == sample_capture.frame

    def test_print_frame(self, sample_capture: Capture, capsys):
        """Should print frame to stdout."""
        sample_capture.print_frame()

        captured = capsys.readouterr()
        assert sample_capture.frame in captured.out


class TestCaptureEdgeCases:
    """Test edge cases for Capture."""

    def test_empty_frame(self):
        """Should handle empty frame."""
        capture = Capture(
            label="empty",
            frame="",
            mode="NORMAL",
            cursor=Cursor(line=0, col=0),
            buffer="",
        )

        d = capture.to_dict()
        assert d["frame"] == ""

    def test_multiline_buffer(self):
        """Should handle multiline buffer."""
        capture = Capture(
            label="multiline",
            frame="",
            mode="NORMAL",
            cursor=Cursor(line=0, col=0),
            buffer="line1\nline2\nline3",
        )

        d = capture.to_dict()
        assert d["buffer"] == "line1\nline2\nline3"

        # JSON should escape newlines properly
        json_str = capture.to_json()
        parsed = json.loads(json_str)
        assert parsed["buffer"] == "line1\nline2\nline3"

    def test_special_characters_in_frame(self):
        """Should handle ANSI escape codes in frame."""
        frame = "\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m"
        capture = Capture(
            label="ansi",
            frame=frame,
            mode="NORMAL",
            cursor=Cursor(line=0, col=0),
            buffer="",
        )

        # Should preserve escape codes in dict
        d = capture.to_dict()
        assert "\x1b[31m" in d["frame"]

        # JSON should escape properly
        json_str = capture.to_json()
        parsed = json.loads(json_str)
        assert parsed["frame"] == frame

    def test_multiple_registers(self):
        """Should handle multiple registers."""
        capture = Capture(
            label="regs",
            frame="",
            mode="NORMAL",
            cursor=Cursor(line=0, col=0),
            buffer="",
            registers={
                "a": Register(name="a", content="alpha"),
                "b": Register(name="b", content="beta", type="line"),
                "0": Register(name="0", content="zero"),
            },
        )

        d = capture.to_dict()
        assert len(d["registers"]) == 3
        assert d["registers"]["a"]["content"] == "alpha"
        assert d["registers"]["b"]["type"] == "line"
