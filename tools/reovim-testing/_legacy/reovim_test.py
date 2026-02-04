"""
Reovim LLM Agent Test System

Orchestrates reovim server and CLI for LLM-driven interactive testing.
Provides structured state capture and assertions for programmatic validation.

Example:
    >>> with TestSession().start("hello world") as t:
    ...     t.keys("dw")
    ...     t.assert_buffer("world")
    ...     t.assert_mode("normal")
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import time
from dataclasses import dataclass, asdict, field
from pathlib import Path
from typing import Callable, Optional


@dataclass
class CursorPos:
    """Cursor position in a buffer."""
    line: int  # 0-indexed
    col: int   # 0-indexed


@dataclass
class RegisterEntry:
    """Content of a vim register."""
    name: str
    content_type: str  # "text", "macro", "empty"
    content: str
    yank_type: str     # "char", "line", "block" for text; empty for macro


@dataclass
class StateCapture:
    """Captured editor state at a point in time."""
    label: Optional[str]
    frame: str              # Raw ANSI frame buffer from TUI
    mode: str
    cursor: CursorPos
    buffer: str
    registers: dict[str, RegisterEntry]
    timestamp: float

    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps(self.to_dict(), indent=2)

    def to_dict(self) -> dict:
        """Convert to dictionary."""
        return {
            "label": self.label,
            "frame": self.frame,
            "mode": self.mode,
            "cursor": {"line": self.cursor.line, "col": self.cursor.col},
            "buffer": self.buffer,
            "registers": {
                name: asdict(entry) for name, entry in self.registers.items()
            },
            "timestamp": self.timestamp,
        }

    def print_frame(self) -> None:
        """Print the raw ANSI frame to terminal (for visual inspection)."""
        print(self.frame)

    def save_frame(self, path: str) -> None:
        """Save raw ANSI frame to file for later replay/diff."""
        with open(path, 'w') as f:
            f.write(self.frame)


class TestSessionError(Exception):
    """Error during test session execution."""
    pass


class TestSession:
    """
    Orchestrates reovim server, CLI, and TUI for LLM testing.

    Provides a fluent interface for:
    - Spawning and managing reovim server
    - Sending keys and commands via CLI
    - Capturing and asserting editor state
    - Generating test reports

    Example:
        >>> with TestSession().start("hello world") as t:
        ...     t.keys("dw")
        ...     t.assert_buffer("world")
        ...     print(t.capture().to_json())
    """

    def __init__(
        self,
        binary_path: str = "./target/release/reovim",
        timeout: float = 5.0,
    ):
        """
        Initialize a test session.

        Args:
            binary_path: Path to reovim binary (default: release build)
            timeout: Default timeout for operations in seconds
        """
        self.binary = Path(binary_path)
        self.timeout = timeout
        self.server_proc: Optional[subprocess.Popen] = None
        self.tui_proc: Optional[subprocess.Popen] = None
        self.server_port: Optional[int] = None
        self.checkpoints: list[StateCapture] = []

    def __enter__(self) -> "TestSession":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.stop()

    # === Lifecycle ===

    def start(self, content: str = "") -> "TestSession":
        """
        Start server, headless TUI, and initialize with content.

        Args:
            content: Initial buffer content (optional)

        Returns:
            Self for method chaining
        """
        if not self.binary.exists():
            # Try debug build
            debug_binary = Path("./target/debug/reovim")
            if debug_binary.exists():
                self.binary = debug_binary
            else:
                raise TestSessionError(
                    f"Binary not found: {self.binary}\n"
                    "Run 'cargo build --release' or 'cargo build' first"
                )

        # Start server on random port (port 0 = OS assigns)
        self.server_proc = subprocess.Popen(
            [str(self.binary), "server", "--tcp", "0"],
            stderr=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            bufsize=1,  # Line buffered for reliable readline()
        )

        # Extract port from stderr (server prints "Listening on 127.0.0.1:PORT")
        self.server_port = self._extract_port()

        if self.server_port is None:
            self.stop()
            raise TestSessionError("Failed to extract server port from output")

        # Wait for server to be ready
        self._wait_for_ready()

        # Spawn headless TUI for frame capture support
        self._spawn_headless_tui()

        # Initialize buffer with content if provided
        if content:
            self._init_buffer(content)

        return self

    def stop(self) -> None:
        """Stop server, TUI, and cleanup."""
        # Stop headless TUI first
        if self.tui_proc is not None:
            self.tui_proc.terminate()
            try:
                self.tui_proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.tui_proc.kill()
            self.tui_proc = None

        # Stop server
        if self.server_proc is not None:
            try:
                # Try graceful shutdown via CLI
                self._cli("kill")
            except Exception:
                pass

            # Give it a moment to shutdown
            try:
                self.server_proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                # Force kill if still running
                self.server_proc.send_signal(signal.SIGTERM)
                try:
                    self.server_proc.wait(timeout=1.0)
                except subprocess.TimeoutExpired:
                    self.server_proc.kill()

            self.server_proc = None
            self.server_port = None

    # === Input ===

    def keys(self, keys: str) -> "TestSession":
        """
        Send vim keys. Returns self for chaining.

        Args:
            keys: Keys in vim notation (e.g., "iHello<Esc>", "dw", "<C-w>h")

        Returns:
            Self for method chaining
        """
        result = self._cli("keys", keys)
        # Small delay to allow state to settle
        time.sleep(0.01)
        return self

    def type_text(self, text: str) -> "TestSession":
        """
        Enter insert mode, type text, return to normal.

        Args:
            text: Text to insert

        Returns:
            Self for method chaining
        """
        return self.keys(f"i{text}<Esc>")

    # === State Capture ===

    def capture(self, label: Optional[str] = None) -> StateCapture:
        """
        Capture current state including TUI frame buffer.

        Args:
            label: Optional label for this checkpoint

        Returns:
            StateCapture with frame, mode, cursor, buffer, registers
        """
        state = StateCapture(
            label=label,
            frame=self._capture_frame_buffer(),
            mode=self._query_mode(),
            cursor=self._query_cursor(),
            buffer=self._query_buffer(),
            registers=self._query_registers(),
            timestamp=time.time(),
        )
        self.checkpoints.append(state)
        return state

    def screen(self, format: str = "raw_ansi") -> str:
        """
        Capture the TUI frame buffer (server-side rendering).

        Args:
            format: Output format - "raw_ansi", "plain_text", or "cell_grid"

        Returns:
            Screen content as string (with ANSI codes if raw_ansi)
        """
        return self._cli_raw("content", format)

    def print_screen(self, format: str = "raw_ansi") -> "TestSession":
        """
        Capture and print the TUI frame buffer to terminal.

        Returns:
            Self for method chaining
        """
        print(self.screen(format))
        return self

    def wait_for(
        self,
        condition: Callable[[StateCapture], bool],
        timeout: Optional[float] = None,
    ) -> StateCapture:
        """
        Wait until condition is true, return final state.

        Args:
            condition: Callable that takes StateCapture and returns bool
            timeout: Max wait time in seconds (default: session timeout)

        Returns:
            StateCapture when condition is met

        Raises:
            TimeoutError: If condition not met within timeout
        """
        timeout = timeout or self.timeout
        start = time.time()
        while time.time() - start < timeout:
            state = self.capture()
            if condition(state):
                return state
            time.sleep(0.05)
        raise TimeoutError(f"Condition not met within {timeout}s")

    # === Assertions ===

    def assert_mode(self, expected: str) -> "TestSession":
        """
        Assert current mode matches.

        Args:
            expected: Expected mode name (case-insensitive partial match)

        Returns:
            Self for method chaining

        Raises:
            AssertionError: If mode doesn't match
        """
        state = self.capture()
        if expected.lower() not in state.mode.lower():
            raise AssertionError(
                f"Expected mode '{expected}', got '{state.mode}'"
            )
        return self

    def assert_buffer(self, expected: str) -> "TestSession":
        """
        Assert buffer content matches.

        Args:
            expected: Expected buffer content (whitespace-normalized)

        Returns:
            Self for method chaining

        Raises:
            AssertionError: If buffer doesn't match
        """
        state = self.capture()
        if state.buffer.strip() != expected.strip():
            raise AssertionError(
                f"Buffer mismatch:\nExpected: {expected!r}\nGot: {state.buffer!r}"
            )
        return self

    def assert_cursor(self, line: int, col: int) -> "TestSession":
        """
        Assert cursor position.

        Args:
            line: Expected line (0-indexed)
            col: Expected column (0-indexed)

        Returns:
            Self for method chaining

        Raises:
            AssertionError: If cursor doesn't match
        """
        state = self.capture()
        if (state.cursor.line, state.cursor.col) != (line, col):
            raise AssertionError(
                f"Cursor at ({state.cursor.line}, {state.cursor.col}), "
                f"expected ({line}, {col})"
            )
        return self

    def assert_register(self, name: str, expected: str) -> "TestSession":
        """
        Assert register content matches.

        Args:
            name: Register name (e.g., "a", '"', "0")
            expected: Expected content

        Returns:
            Self for method chaining

        Raises:
            AssertionError: If register doesn't match
        """
        state = self.capture()
        reg = state.registers.get(name)
        if reg is None:
            raise AssertionError(
                f"Register '{name}' is empty, expected: {expected!r}"
            )
        if reg.content != expected:
            raise AssertionError(
                f"Register '{name}' mismatch:\n"
                f"Expected: {expected!r}\nGot: {reg.content!r}"
            )
        return self

    # === Checkpoint Events ===

    def checkpoint_after(
        self,
        keys: str,
        label: Optional[str] = None,
    ) -> StateCapture:
        """
        Send keys and capture state immediately after.

        Args:
            keys: Keys to send
            label: Optional label for the checkpoint

        Returns:
            StateCapture after keys processed
        """
        self.keys(keys)
        return self.capture(label or f"after:{keys}")

    # === Output ===

    def report(self) -> str:
        """
        Generate markdown report of all checkpoints.

        Returns:
            Markdown-formatted report string
        """
        lines = ["# Test Session Report\n"]
        for i, cp in enumerate(self.checkpoints):
            lines.append(f"## Checkpoint {i}: {cp.label or 'unnamed'}")
            lines.append(f"- Mode: `{cp.mode}`")
            lines.append(f"- Cursor: ({cp.cursor.line}, {cp.cursor.col})")
            lines.append(f"- Buffer:\n```\n{cp.buffer}\n```\n")
        return "\n".join(lines)

    # === Private Methods ===

    def _extract_port(self) -> Optional[int]:
        """Extract port number from server stderr output."""
        if self.server_proc is None or self.server_proc.stderr is None:
            return None

        import threading
        import queue

        port_queue: queue.Queue[Optional[int]] = queue.Queue()

        def read_stderr():
            """Read stderr in a thread to avoid blocking."""
            try:
                for line in iter(self.server_proc.stderr.readline, ''):
                    if "Listening on" in line:
                        # Parse "Listening on 127.0.0.1:PORT"
                        try:
                            port_str = line.split(":")[-1].strip()
                            port_queue.put(int(port_str))
                            return
                        except (IndexError, ValueError):
                            continue
            except Exception:
                pass
            port_queue.put(None)

        # Start reader thread
        reader = threading.Thread(target=read_stderr, daemon=True)
        reader.start()

        # Wait for port with timeout
        try:
            port = port_queue.get(timeout=5.0)
            return port
        except queue.Empty:
            return None

    def _wait_for_ready(self, max_attempts: int = 10) -> None:
        """Wait for server to be ready to accept connections."""
        for _ in range(max_attempts):
            try:
                # Use 'mode' command to check server is responding
                self._cli("mode")
                return
            except Exception:
                time.sleep(0.1)
        raise TestSessionError("Server not responding")

    def _spawn_headless_tui(self) -> None:
        """Spawn headless TUI for frame capture support."""
        if self.server_port is None:
            raise TestSessionError("Server not running")

        self.tui_proc = subprocess.Popen(
            [str(self.binary), "tui",
             "--tcp", f"127.0.0.1:{self.server_port}",
             "--headless"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        # Wait for TUI to connect
        time.sleep(0.3)

    def _capture_frame_buffer(self, format: str = "raw_ansi") -> str:
        """
        Capture TUI frame buffer via CLI relay.

        Requires headless TUI to be connected.

        The capture flow is: CLI → Server → TUI → Server → CLI
        - CLI sends GetScreenContent RPC to Server
        - Server sends capture_request notification to TUI
        - TUI captures frame and calls SubmitCaptureResponse RPC
        - Server returns frame content to CLI

        Args:
            format: "raw_ansi" (with colors), "plain_text", or "cell_grid"

        Returns:
            Frame buffer content as string
        """
        try:
            return self._cli_raw("capture", "--capture-format", format)
        except TestSessionError:
            # TUI might not be connected, return empty
            return ""

    def _init_buffer(self, content: str) -> None:
        """Initialize buffer with content."""
        # Use insert mode to add content, then return to normal
        # Escape special characters for vim notation
        escaped = content.replace("<", "<lt>")
        self.keys(f"i{escaped}<Esc>gg0")

    def _cli(self, *args: str) -> str:
        """Run CLI command with JSON format and return stdout."""
        if self.server_port is None:
            raise TestSessionError("Server not running")

        cmd = [
            str(self.binary), "cli",
            "--tcp", f"127.0.0.1:{self.server_port}",
            "--format", "json",
            *args,
        ]
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.timeout,
        )
        if result.returncode != 0:
            raise TestSessionError(
                f"CLI command failed: {' '.join(args)}\n"
                f"stderr: {result.stderr}"
            )
        return result.stdout

    def _cli_raw(self, *args: str) -> str:
        """Run CLI command without JSON format (raw output)."""
        if self.server_port is None:
            raise TestSessionError("Server not running")

        cmd = [
            str(self.binary), "cli",
            "--tcp", f"127.0.0.1:{self.server_port}",
            *args,
        ]
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.timeout,
        )
        if result.returncode != 0:
            raise TestSessionError(
                f"CLI command failed: {' '.join(args)}\n"
                f"stderr: {result.stderr}"
            )
        return result.stdout

    def _query_mode(self) -> str:
        """Query current mode."""
        try:
            data = json.loads(self._cli("mode"))
            return data.get("display", data.get("name", "unknown"))
        except (json.JSONDecodeError, KeyError):
            return "unknown"

    def _query_cursor(self) -> CursorPos:
        """Query cursor position."""
        try:
            data = json.loads(self._cli("cursor"))
            return CursorPos(
                line=data.get("line", 0),
                col=data.get("column", 0),
            )
        except (json.JSONDecodeError, KeyError):
            return CursorPos(line=0, col=0)

    def _query_buffer(self) -> str:
        """Query buffer content."""
        try:
            data = json.loads(self._cli("buffer"))
            # CLI returns {"content": "..."} for buffer
            return data.get("content", "")
        except (json.JSONDecodeError, KeyError):
            return ""

    def _query_registers(self) -> dict[str, RegisterEntry]:
        """Query all non-empty registers."""
        try:
            data = json.loads(self._cli("registers"))
            registers = {}

            # Handle unnamed register
            unnamed = data.get("unnamed")
            if unnamed and unnamed.get("content"):
                entry = RegisterEntry(
                    name=unnamed.get("name", '"'),
                    content_type="text",
                    content=unnamed.get("content", ""),
                    yank_type=unnamed.get("yank_type", "char"),
                )
                registers[entry.name] = entry

            # Handle named registers
            for reg in data.get("named", []):
                if reg.get("content"):
                    entry = RegisterEntry(
                        name=reg.get("name", ""),
                        content_type="text",
                        content=reg.get("content", ""),
                        yank_type=reg.get("yank_type", "char"),
                    )
                    registers[entry.name] = entry

            return registers
        except (json.JSONDecodeError, KeyError):
            return {}


# Convenience function for quick tests
def test_session(
    content: str = "",
    binary_path: str = "./target/release/reovim",
) -> TestSession:
    """
    Create and start a test session.

    Args:
        content: Initial buffer content
        binary_path: Path to reovim binary

    Returns:
        Started TestSession (use as context manager)

    Example:
        >>> with test_session("hello world") as t:
        ...     t.keys("dw")
        ...     assert t.capture().buffer == "world"
    """
    return TestSession(binary_path=binary_path).start(content)
