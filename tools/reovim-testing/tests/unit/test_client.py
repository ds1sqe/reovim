"""Test CLI client wrapper.

Tests the Client class methods, JSON parsing, and error handling.
Uses mocking to avoid requiring a running server.
"""

from __future__ import annotations

import json
import subprocess
import pytest
from pathlib import Path
from unittest.mock import MagicMock, patch

from reovim.client import Client
from reovim.capture import Cursor, Register
from reovim.discovery import BinaryInfo, Transport
from reovim.errors import ServerError


@pytest.fixture
def mock_binary() -> BinaryInfo:
    """Create a mock binary info for testing."""
    return BinaryInfo(
        path=Path("/usr/bin/reovim-new"),
        transport=Transport.GRPC,
        name="test",
    )


@pytest.fixture
def client(mock_binary: BinaryInfo) -> Client:
    """Create a client with mock binary."""
    return Client(
        binary=mock_binary,
        address="127.0.0.1:12345",
        timeout=1.0,
    )


class TestClientConstruction:
    """Test Client initialization."""

    def test_creates_with_required_args(self, mock_binary: BinaryInfo):
        """Should create client with binary and address."""
        client = Client(binary=mock_binary, address="localhost:8080")

        assert client.binary == mock_binary
        assert client.address == "localhost:8080"
        assert client.timeout == 5.0  # default

    def test_custom_timeout(self, mock_binary: BinaryInfo):
        """Should accept custom timeout."""
        client = Client(binary=mock_binary, address="localhost:8080", timeout=10.0)

        assert client.timeout == 10.0


class TestBuildCommand:
    """Test command building with different transports."""

    def test_grpc_transport(self, mock_binary: BinaryInfo):
        """GRPC transport should use --grpc flag."""
        client = Client(binary=mock_binary, address="127.0.0.1:50051")

        cmd = client._build_command("mode")

        assert "--grpc" in cmd
        assert "127.0.0.1:50051" in cmd
        assert "--tcp" not in cmd

    def test_tcp_transport(self):
        """TCP transport should use --tcp flag."""
        tcp_binary = BinaryInfo(
            path=Path("/usr/bin/reovim"),
            transport=Transport.TCP,
            name="test-tcp",
        )
        client = Client(binary=tcp_binary, address="127.0.0.1:12345")

        cmd = client._build_command("mode")

        assert "--tcp" in cmd
        assert "--grpc" not in cmd

    def test_command_includes_args(self, client: Client):
        """Should include all positional args."""
        cmd = client._build_command("keys", "iHello<Esc>")

        assert "keys" in cmd
        assert "iHello<Esc>" in cmd


class TestModeMethod:
    """Test mode() method."""

    def test_returns_display_name(self, client: Client):
        """Should return display name from JSON."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = json.dumps({"display": "NORMAL", "name": "normal"})
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            mode = client.mode()

        assert mode == "NORMAL"

    def test_falls_back_to_name(self, client: Client):
        """Should fall back to name if no display."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = json.dumps({"name": "insert"})
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            mode = client.mode()

        assert mode == "insert"

    def test_returns_unknown_on_empty(self, client: Client):
        """Should return 'unknown' for empty response."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "{}"
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            mode = client.mode()

        assert mode == "unknown"


class TestCursorMethod:
    """Test cursor() method."""

    def test_returns_cursor_object(self, client: Client):
        """Should return Cursor with line and col."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = json.dumps({"line": 5, "column": 10})
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            cursor = client.cursor()

        assert isinstance(cursor, Cursor)
        assert cursor.line == 5
        assert cursor.col == 10

    def test_defaults_to_zero(self, client: Client):
        """Should default to 0,0 if missing."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "{}"
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            cursor = client.cursor()

        assert cursor.line == 0
        assert cursor.col == 0


class TestBufferMethod:
    """Test buffer() method."""

    def test_joins_lines(self, client: Client):
        """Should join lines with newlines."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = json.dumps({"lines": ["line1", "line2", "line3"]})
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            content = client.buffer()

        assert content == "line1\nline2\nline3"

    def test_empty_buffer(self, client: Client):
        """Should handle empty buffer."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = json.dumps({"lines": []})
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            content = client.buffer()

        assert content == ""


class TestRegistersMethod:
    """Test registers() method."""

    def test_returns_non_empty_registers(self, client: Client):
        """Should return only registers with content."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = json.dumps({
            "registers": [
                {"name": "a", "content": "hello", "yank_type": "char"},
                {"name": "b", "content": "", "yank_type": "line"},  # empty
                {"name": "c", "content": "world", "yank_type": "line"},
            ]
        })
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            regs = client.registers()

        assert "a" in regs
        assert "b" not in regs  # empty content filtered
        assert "c" in regs
        assert regs["a"].content == "hello"
        assert regs["c"].type == "line"

    def test_empty_registers(self, client: Client):
        """Should handle no registers."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = json.dumps({"registers": []})
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            regs = client.registers()

        assert regs == {}


class TestErrorHandling:
    """Test error handling in CLI calls."""

    def test_nonzero_return_raises_server_error(self, client: Client):
        """Should raise ServerError on non-zero exit."""
        mock_result = MagicMock()
        mock_result.returncode = 1
        mock_result.stdout = ""
        mock_result.stderr = "Connection refused"

        with patch("subprocess.run", return_value=mock_result):
            with pytest.raises(ServerError) as exc_info:
                client.mode()

        assert "Connection refused" in str(exc_info.value)

    def test_invalid_json_returns_empty_dict(self, client: Client):
        """Should return empty dict on invalid JSON."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "not valid json"
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            # _cli_json should return {} on parse error
            result = client._cli_json("mode")

        assert result == {}

    def test_timeout_propagates(self, client: Client):
        """Should propagate subprocess timeout."""
        with patch("subprocess.run", side_effect=subprocess.TimeoutExpired("cmd", 1.0)):
            with pytest.raises(subprocess.TimeoutExpired):
                client.mode()


class TestPingMethod:
    """Test ping() method."""

    def test_returns_true_on_success(self, client: Client):
        """Should return True when server responds."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "pong"
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result):
            assert client.ping() is True

    def test_returns_false_on_failure(self, client: Client):
        """Should return False when server fails."""
        mock_result = MagicMock()
        mock_result.returncode = 1
        mock_result.stdout = ""
        mock_result.stderr = "Connection refused"

        with patch("subprocess.run", return_value=mock_result):
            assert client.ping() is False

    def test_returns_false_on_exception(self, client: Client):
        """Should return False on any exception."""
        with patch("subprocess.run", side_effect=subprocess.TimeoutExpired("cmd", 1.0)):
            assert client.ping() is False


class TestCaptureMethod:
    """Test capture() method."""

    def test_default_format(self, client: Client):
        """Should use raw_ansi format by default."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "\x1b[32mHello\x1b[0m"
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result) as mock_run:
            result = client.capture()

        assert result == "\x1b[32mHello\x1b[0m"
        # Check format flag was passed
        call_args = mock_run.call_args[0][0]
        assert "raw_ansi" in call_args

    def test_plain_text_format(self, client: Client):
        """Should support plain_text format."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "Hello"
        mock_result.stderr = ""

        with patch("subprocess.run", return_value=mock_result) as mock_run:
            result = client.capture(format="plain_text")

        call_args = mock_run.call_args[0][0]
        assert "plain_text" in call_args
