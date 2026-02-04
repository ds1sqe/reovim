"""Test error message formatting.

Verifies that all custom exceptions produce helpful, informative messages.
"""

from __future__ import annotations

import pytest

from reovim.errors import (
    ReovimError,
    BinaryNotFoundError,
    ServerError,
    ConnectionError,
    TimeoutError,
    AssertionError,
)


class TestReovimError:
    """Test base error class."""

    def test_is_exception(self):
        """Base class should be a proper exception."""
        assert issubclass(ReovimError, Exception)

    def test_can_raise_and_catch(self):
        """Should be raisable and catchable."""
        with pytest.raises(ReovimError):
            raise ReovimError("test message")


class TestBinaryNotFoundError:
    """Test binary not found error formatting."""

    def test_message_includes_searched_paths(self):
        """Error message should list all searched locations."""
        searched = [
            ("release-new", "/project/target/release/reovim-new"),
            ("debug-new", "/project/target/debug/reovim-new"),
        ]
        err = BinaryNotFoundError(searched)

        msg = str(err)
        assert "release-new" in msg
        assert "/project/target/release/reovim-new" in msg
        assert "debug-new" in msg

    def test_message_includes_solutions(self):
        """Error message should include helpful solutions."""
        err = BinaryNotFoundError([("test", "/test/path")])

        msg = str(err)
        assert "cargo build" in msg
        assert "REOVIM_BINARY" in msg
        assert "PATH" in msg

    def test_stores_searched_list(self):
        """Should store the searched list for programmatic access."""
        searched = [("a", "/path/a"), ("b", "/path/b")]
        err = BinaryNotFoundError(searched)

        assert err.searched == searched

    def test_empty_searched_list(self):
        """Should handle empty search list gracefully."""
        err = BinaryNotFoundError([])

        msg = str(err)
        assert "Could not find" in msg

    def test_is_reovim_error(self):
        """Should be catchable as ReovimError."""
        with pytest.raises(ReovimError):
            raise BinaryNotFoundError([])


class TestServerError:
    """Test server error formatting."""

    def test_message_only(self):
        """Should work with just a message."""
        err = ServerError("Server crashed")

        msg = str(err)
        assert "Server crashed" in msg

    def test_with_stderr(self):
        """Should include stderr when provided."""
        err = ServerError("Server crashed", "segfault at 0x0")

        msg = str(err)
        assert "Server crashed" in msg
        assert "segfault" in msg
        assert "stderr" in msg.lower()

    def test_stores_stderr(self):
        """Should store stderr for programmatic access."""
        stderr = "detailed error info"
        err = ServerError("msg", stderr)

        assert err.stderr == stderr

    def test_empty_stderr(self):
        """Empty stderr should not appear in message."""
        err = ServerError("Server crashed", "")

        msg = str(err)
        assert "stderr" not in msg.lower()


class TestConnectionError:
    """Test connection error."""

    def test_basic_message(self):
        """Should pass through message."""
        err = ConnectionError("Failed to connect to 127.0.0.1:12345")

        assert "127.0.0.1" in str(err)

    def test_is_reovim_error(self):
        """Should be catchable as ReovimError."""
        with pytest.raises(ReovimError):
            raise ConnectionError("test")


class TestTimeoutError:
    """Test timeout error formatting."""

    def test_includes_operation_and_time(self):
        """Should show what timed out and how long."""
        err = TimeoutError("connection", 5.0)

        msg = str(err)
        assert "connection" in msg
        assert "5" in msg

    def test_stores_attributes(self):
        """Should store operation and timeout for programmatic access."""
        err = TimeoutError("ping", 10.5)

        assert err.operation == "ping"
        assert err.timeout == 10.5

    def test_decimal_timeout(self):
        """Should handle decimal timeouts."""
        err = TimeoutError("test", 0.5)

        assert "0.5" in str(err)


class TestAssertionError:
    """Test assertion error formatting."""

    def test_shows_expected_vs_actual(self):
        """Should clearly show expected and actual values."""
        err = AssertionError("mode", "NORMAL", "INSERT")

        msg = str(err)
        assert "NORMAL" in msg
        assert "INSERT" in msg
        assert "expected" in msg.lower()
        assert "actual" in msg.lower()

    def test_stores_attributes(self):
        """Should store what, expected, actual for programmatic access."""
        err = AssertionError("buffer", "hello", "world")

        assert err.what == "buffer"
        assert err.expected == "hello"
        assert err.actual == "world"

    def test_quotes_values(self):
        """Values should be repr'd for clarity."""
        err = AssertionError("test", "a", "b")

        msg = str(err)
        # repr adds quotes
        assert "'a'" in msg or '"a"' in msg

    def test_multiline_values(self):
        """Should handle multiline values."""
        expected = "line1\nline2"
        actual = "line1\nline3"
        err = AssertionError("buffer", expected, actual)

        # Should not crash and should contain values
        msg = str(err)
        assert "line1" in msg
