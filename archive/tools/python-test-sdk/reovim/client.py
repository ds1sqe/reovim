"""Client - Wraps CLI calls to server."""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path

from .capture import Cursor, Register
from .discovery import BinaryInfo, Transport
from .errors import ConnectionError, ServerError


@dataclass
class Client:
    """Wraps CLI calls to server.

    Provides type-safe methods for all CLI operations.
    """

    binary: BinaryInfo
    address: str
    timeout: float = 5.0

    def keys(self, keys: str) -> None:
        """Send vim keys to editor."""
        self._cli("keys", keys)

    def mode(self) -> str:
        """Get current mode."""
        data = self._cli_json("mode")
        return data.get("display", data.get("name", "unknown"))

    def cursor(self) -> Cursor:
        """Get cursor position."""
        data = self._cli_json("cursor")
        return Cursor(
            line=data.get("line", 0),
            col=data.get("column", 0),
        )

    def buffer(self) -> str:
        """Get buffer content."""
        data = self._cli_json("buffer")
        lines = data.get("lines", [])
        return "\n".join(lines)

    def registers(self) -> dict[str, Register]:
        """Get all non-empty registers."""
        data = self._cli_json("registers")
        result: dict[str, Register] = {}

        for reg_data in data.get("registers", []):
            if reg_data.get("content"):
                reg = Register(
                    name=reg_data.get("name", ""),
                    content=reg_data.get("content", ""),
                    type=reg_data.get("yank_type", "char"),
                )
                result[reg.name] = reg

        return result

    def capture(self, client_id: int, format: str = "raw_ansi") -> str:
        """Capture screen content from TUI.

        Args:
            client_id: Target TUI client ID (required)
            format: "raw_ansi", "plain_text", or "cell_grid"

        Returns:
            Screen content as string
        """
        return self._cli_raw("capture", "--client", str(client_id), "--capture-format", format)

    def presence_list(self) -> list[dict]:
        """List all connected clients.

        Returns:
            List of client info dicts with 'client_id', 'display_name', 'client_type'
        """
        data = self._cli_json("presence", "list")
        return data.get("clients", [])

    def ping(self) -> bool:
        """Check if server is responding."""
        try:
            self._cli("ping")
            return True
        except Exception:
            return False

    def _cli(self, *args: str) -> str:
        """Run CLI command and return stdout."""
        cmd = self._build_command(*args)
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.timeout,
        )
        if result.returncode != 0:
            raise ServerError(
                f"CLI command failed: {' '.join(args)}",
                result.stderr,
            )
        return result.stdout

    def _cli_json(self, *args: str) -> dict:
        """Run CLI command with JSON output."""
        cmd = self._build_command("--format", "json", *args)
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.timeout,
        )
        if result.returncode != 0:
            raise ServerError(
                f"CLI command failed: {' '.join(args)}",
                result.stderr,
            )
        try:
            return json.loads(result.stdout)
        except json.JSONDecodeError:
            return {}

    def _cli_raw(self, *args: str) -> str:
        """Run CLI command without JSON format (raw output)."""
        cmd = self._build_command(*args)
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.timeout,
        )
        if result.returncode != 0:
            raise ServerError(
                f"CLI command failed: {' '.join(args)}",
                result.stderr,
            )
        return result.stdout

    def _build_command(self, *args: str) -> list[str]:
        """Build CLI command with transport-specific address flag."""
        if self.binary.transport == Transport.GRPC:
            return [str(self.binary.path), "cli", "--grpc", self.address, *args]
        else:
            return [str(self.binary.path), "cli", "--tcp", self.address, *args]
