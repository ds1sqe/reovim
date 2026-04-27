"""Editor - The heart of reovim testing.

Zero-config, fluent API, context manager - just works.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Callable, Self

from .capture import Capture, Cursor, Register
from .client import Client
from .discovery import BinaryInfo, discover_binary, find_free_port, find_project_root
from .errors import AssertionError, TimeoutError
from .process import ServerProcess, TuiProcess

if TYPE_CHECKING:
    pass


@dataclass
class Editor:
    """Zero-config reovim editor for LLM testing.

    Auto-discovers binary, manages server/TUI lifecycle,
    provides fluent API for testing.

    Example:
        >>> from reovim import Editor
        >>> with Editor() as e:
        ...     e.keys("iHello<Esc>")
        ...     print(e.buffer)  # "Hello"
        ...     e.assert_mode("normal")
    """

    # Configuration (all optional, auto-discovered if not set)
    binary_path: Path | None = None
    width: int = 120
    height: int = 40
    timeout: float = 5.0

    # Internal state (not for external use)
    _binary: BinaryInfo | None = field(default=None, init=False, repr=False)
    _server: ServerProcess | None = field(default=None, init=False, repr=False)
    _tui: TuiProcess | None = field(default=None, init=False, repr=False)
    _client: Client | None = field(default=None, init=False, repr=False)
    _tui_client_id: int = field(default=1, init=False, repr=False)
    _started: bool = field(default=False, init=False, repr=False)

    def __enter__(self) -> Self:
        """Start editor (discover binary, start server, connect TUI)."""
        self._start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Stop editor (cleanup TUI, server)."""
        self._stop()

    def _start(self) -> None:
        """Initialize everything."""
        if self._started:
            return

        # Discover binary
        base_dir = find_project_root()
        if self.binary_path:
            from .discovery import Transport

            transport = Transport.GRPC
            self._binary = BinaryInfo(path=self.binary_path, transport=transport, name="explicit")
        else:
            self._binary = discover_binary(base_dir)

        # Start server on free port
        port = find_free_port()
        self._server = ServerProcess(binary=self._binary, port=port)
        actual_port = self._server.start()

        # Create client
        self._client = Client(
            binary=self._binary,
            address=self._server.address,
            timeout=self.timeout,
        )

        # Start headless TUI for frame capture
        self._tui = TuiProcess(
            binary=self._binary,
            address=self._server.address,
            width=self.width,
            height=self.height,
        )
        self._tui.start()

        # Wait for everything to be ready
        self._wait_ready()
        self._started = True

    def _wait_ready(self, max_attempts: int = 10) -> None:
        """Wait for server and TUI to be ready."""
        for i in range(max_attempts):
            if self._client and self._client.ping():
                # Discover TUI client ID from presence list
                self._discover_tui_client_id()
                return
            time.sleep(0.1)

        raise TimeoutError("server to respond to ping", self.timeout)

    def _discover_tui_client_id(self) -> None:
        """Find the TUI's client ID from presence list."""
        if not self._client:
            return

        # Give TUI time to join presence
        for _ in range(10):
            try:
                clients = self._client.presence_list()
                # Find the headless TUI client (client_type is "headless" for headless TUI)
                for c in clients:
                    if c.get("client_type") in ("headless", "tui"):
                        self._tui_client_id = c.get("client_id", 1)
                        return
                # If no TUI found yet, wait and retry
                time.sleep(0.1)
            except Exception:
                time.sleep(0.1)

        # Default to 1 if discovery fails
        self._tui_client_id = 1

    def _stop(self) -> None:
        """Cleanup everything."""
        if self._tui:
            self._tui.stop()
            self._tui = None

        if self._server:
            self._server.stop()
            self._server = None

        self._client = None
        self._started = False

    # =========================================================================
    # Properties - Instant access to state
    # =========================================================================

    @property
    def mode(self) -> str:
        """Get current mode (NORMAL, INSERT, etc.)."""
        self._ensure_started()
        assert self._client is not None
        return self._client.mode()

    @property
    def buffer(self) -> str:
        """Get current buffer content."""
        self._ensure_started()
        assert self._client is not None
        return self._client.buffer()

    @property
    def cursor(self) -> Cursor:
        """Get current cursor position."""
        self._ensure_started()
        assert self._client is not None
        return self._client.cursor()

    @property
    def screen(self) -> str:
        """Get raw ANSI frame from TUI."""
        self._ensure_started()
        assert self._client is not None
        return self._client.capture(self._tui_client_id, "raw_ansi")

    # =========================================================================
    # Actions - All return self for chaining
    # =========================================================================

    def keys(self, keys: str) -> Self:
        """Send vim keys to editor.

        Args:
            keys: Keys in vim notation (e.g., "iHello<Esc>", "dw", "<C-w>h")

        Returns:
            Self for chaining

        Example:
            >>> e.keys("iHello World<Esc>").keys("gg0dw")
        """
        self._ensure_started()
        assert self._client is not None
        self._client.keys(keys)
        return self

    def type(self, text: str) -> Self:
        """Convenience: Enter insert mode, type text, exit to normal.

        Args:
            text: Text to insert

        Returns:
            Self for chaining

        Example:
            >>> e.type("Hello World")  # Same as e.keys("iHello World<Esc>")
        """
        # Escape any special characters for vim notation
        escaped = text.replace("<", "<LT>")
        return self.keys(f"i{escaped}<Esc>")

    # =========================================================================
    # Capture - Full state snapshot
    # =========================================================================

    def capture(self, label: str = "") -> Capture:
        """Capture current state (frame, mode, cursor, buffer, registers).

        Args:
            label: Optional label for this capture

        Returns:
            Capture with all state data

        Example:
            >>> snap = e.capture("after delete")
            >>> snap.print_frame()  # Show ANSI output
            >>> print(snap.to_json())  # JSON for LLM
        """
        self._ensure_started()
        assert self._client is not None

        return Capture(
            label=label,
            frame=self._client.capture(self._tui_client_id, "raw_ansi"),
            mode=self._client.mode(),
            cursor=self._client.cursor(),
            buffer=self._client.buffer(),
            registers=self._client.registers(),
        )

    # =========================================================================
    # Assertions - Raise on failure, return self for chaining
    # =========================================================================

    def assert_mode(self, expected: str) -> Self:
        """Assert current mode (case-insensitive partial match).

        Args:
            expected: Expected mode (e.g., "normal", "insert")

        Returns:
            Self for chaining

        Raises:
            AssertionError: If mode doesn't match
        """
        actual = self.mode.lower()
        if expected.lower() not in actual:
            raise AssertionError("mode", expected, self.mode)
        return self

    def assert_buffer(self, expected: str) -> Self:
        """Assert buffer content (whitespace-normalized comparison).

        Args:
            expected: Expected buffer content

        Returns:
            Self for chaining

        Raises:
            AssertionError: If buffer doesn't match
        """
        actual = self.buffer.strip()
        if actual != expected.strip():
            raise AssertionError("buffer", expected, actual)
        return self

    def assert_cursor(self, line: int, col: int) -> Self:
        """Assert cursor position (0-indexed).

        Args:
            line: Expected line
            col: Expected column

        Returns:
            Self for chaining

        Raises:
            AssertionError: If cursor doesn't match
        """
        actual = self.cursor
        if actual.line != line or actual.col != col:
            raise AssertionError(
                "cursor",
                f"({line}, {col})",
                f"({actual.line}, {actual.col})",
            )
        return self

    def assert_register(self, name: str, content: str) -> Self:
        """Assert register content.

        Args:
            name: Register name (e.g., '"', 'a')
            content: Expected content

        Returns:
            Self for chaining

        Raises:
            AssertionError: If register doesn't match
        """
        self._ensure_started()
        assert self._client is not None

        registers = self._client.registers()
        reg = registers.get(name)

        if reg is None:
            raise AssertionError(f"register {name!r}", content, "(empty)")

        if reg.content != content:
            raise AssertionError(f"register {name!r}", content, reg.content)

        return self

    # =========================================================================
    # Wait - For async state changes
    # =========================================================================

    def wait_for(
        self,
        condition: Callable[[Capture], bool],
        timeout: float | None = None,
        poll_interval: float = 0.1,
    ) -> Capture:
        """Wait for a condition to be true.

        Args:
            condition: Function that takes Capture and returns True when done
            timeout: Max wait time (default: self.timeout)
            poll_interval: Time between checks

        Returns:
            Capture when condition becomes true

        Raises:
            TimeoutError: If condition not met within timeout

        Example:
            >>> e.keys("i").wait_for(lambda s: "INSERT" in s.mode)
        """
        timeout = timeout or self.timeout
        start = time.time()

        while True:
            snap = self.capture()
            if condition(snap):
                return snap

            elapsed = time.time() - start
            if elapsed >= timeout:
                raise TimeoutError("condition to be true", timeout)

            time.sleep(poll_interval)

    # =========================================================================
    # Internal
    # =========================================================================

    def _ensure_started(self) -> None:
        """Ensure editor is started."""
        if not self._started:
            raise RuntimeError("Editor not started. Use 'with Editor() as e:' or call _start()")


# =============================================================================
# Convenience Functions
# =============================================================================


def edit(content: str, keys: str) -> str:
    """One-liner: apply keys to content, return result.

    Args:
        content: Initial buffer content
        keys: Vim keys to apply

    Returns:
        Final buffer content

    Example:
        >>> edit("hello world", "dw")
        'world'
    """
    with Editor() as e:
        e.type(content).keys(f"gg0{keys}")
        return e.buffer


def test(content: str = "") -> Editor:
    """Create a test editor with initial content.

    Args:
        content: Initial buffer content

    Returns:
        Started Editor instance (use as context manager)

    Example:
        >>> with test("hello world") as e:
        ...     e.keys("dw").assert_buffer("world")
    """
    editor = Editor()
    editor._start()
    if content:
        editor.type(content).keys("gg0")
    return editor
