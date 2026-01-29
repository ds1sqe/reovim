"""Process management - Server and TUI lifecycle."""

from __future__ import annotations

import subprocess
import time
from dataclasses import dataclass, field

from .discovery import BinaryInfo, Transport
from .errors import ServerError, TimeoutError


@dataclass
class ServerProcess:
    """Manages server lifecycle with proper cleanup."""

    binary: BinaryInfo
    port: int
    _proc: subprocess.Popen | None = field(default=None, init=False)
    _actual_port: int | None = field(default=None, init=False)

    def start(self) -> int:
        """Start server and wait for ready signal.

        Returns:
            Actual port the server is listening on
        """
        cmd = self._build_command()

        # Start server with stderr/stdout to devnull (we'll ping for readiness)
        self._proc = subprocess.Popen(
            cmd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        # Wait for server to be ready via ping
        self._actual_port = self.port
        self._wait_for_ready(timeout=5.0)
        return self._actual_port

    def _build_command(self) -> list[str]:
        """Build command based on transport type."""
        if self.binary.transport == Transport.GRPC:
            return [str(self.binary.path), "server", "--grpc", str(self.port)]
        else:
            return [str(self.binary.path), "server", "--tcp", str(self.port)]

    def _wait_for_ready(self, timeout: float) -> None:
        """Wait for server to respond to ping."""
        import socket

        start = time.time()
        while time.time() - start < timeout:
            # Check if process died
            if self._proc and self._proc.poll() is not None:
                raise ServerError(f"Server exited with code {self._proc.returncode}")

            # Try to connect
            try:
                with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                    s.settimeout(0.5)
                    s.connect(("127.0.0.1", self.port))
                    return  # Connected!
            except (socket.timeout, ConnectionRefusedError, OSError):
                time.sleep(0.1)

        self.stop()
        raise TimeoutError("server to accept connections", timeout)

    def stop(self) -> None:
        """Graceful shutdown with timeout, then force kill."""
        if self._proc is None:
            return

        # Try SIGTERM first
        self._proc.terminate()
        try:
            self._proc.wait(timeout=2.0)
        except subprocess.TimeoutExpired:
            # Force kill
            self._proc.kill()
            self._proc.wait(timeout=1.0)

        self._proc = None
        self._actual_port = None

    @property
    def address(self) -> str:
        """Get server address (host:port)."""
        port = self._actual_port or self.port
        return f"127.0.0.1:{port}"

    @property
    def is_running(self) -> bool:
        """Check if server process is still running."""
        return self._proc is not None and self._proc.poll() is None


@dataclass
class TuiProcess:
    """Manages headless TUI for frame capture."""

    binary: BinaryInfo
    address: str
    width: int = 120
    height: int = 40
    _proc: subprocess.Popen | None = field(default=None, init=False)

    def start(self) -> None:
        """Connect headless TUI to server."""
        cmd = self._build_command()

        self._proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Give TUI time to connect
        time.sleep(0.3)

        # Check if still running
        if self._proc.poll() is not None:
            stderr = ""
            if self._proc.stderr:
                stderr = self._proc.stderr.read().decode("utf-8", errors="replace")
            raise ServerError("Headless TUI exited immediately", stderr)

    def _build_command(self) -> list[str]:
        """Build command based on transport type."""
        if self.binary.transport == Transport.GRPC:
            return [
                str(self.binary.path),
                "tui",
                "--grpc",
                self.address,
                "--headless",
                "--width",
                str(self.width),
                "--height",
                str(self.height),
            ]
        else:
            return [
                str(self.binary.path),
                "tui",
                "--tcp",
                self.address,
                "--headless",
            ]

    def stop(self) -> None:
        """Graceful disconnect."""
        if self._proc is None:
            return

        self._proc.terminate()
        try:
            self._proc.wait(timeout=2.0)
        except subprocess.TimeoutExpired:
            self._proc.kill()
            self._proc.wait(timeout=1.0)

        self._proc = None

    @property
    def is_running(self) -> bool:
        """Check if TUI process is still running."""
        return self._proc is not None and self._proc.poll() is None
