"""Discovery - Auto-find binary and ports."""

from __future__ import annotations

import os
import shutil
import socket
from dataclasses import dataclass
from enum import Enum, auto
from pathlib import Path

from .errors import BinaryNotFoundError


class Transport(Enum):
    """Transport protocol for server communication."""

    GRPC = auto()  # reovim (gRPC transport)
    TCP = auto()  # reserved for future TCP transport


@dataclass
class BinaryInfo:
    """Information about a discovered binary."""

    path: Path
    transport: Transport
    name: str

    def __str__(self) -> str:
        return f"{self.name} ({self.transport.name}): {self.path}"


# Search order: prefer release, then debug
BINARY_SEARCH_ORDER: list[tuple[str, str, Transport]] = [
    ("release", "target/release/reovim", Transport.GRPC),
    ("debug", "target/debug/reovim", Transport.GRPC),
]


def discover_binary(base_dir: Path | None = None) -> BinaryInfo:
    """Find reovim binary in order of preference.

    Search order:
    1. REOVIM_BINARY environment variable
    2. ./target/release/reovim
    3. ./target/debug/reovim
    4. System PATH (reovim)

    Args:
        base_dir: Base directory for relative paths (default: cwd)

    Returns:
        BinaryInfo with path and transport type

    Raises:
        BinaryNotFoundError: If no binary found
    """
    searched: list[tuple[str, str]] = []

    # Try to find project root (Cargo.toml) first for target/ directory
    base = base_dir or find_project_root() or Path.cwd()

    # 1. Environment variable
    env_binary = os.environ.get("REOVIM_BINARY")
    if env_binary:
        path = Path(env_binary)
        searched.append(("env:REOVIM_BINARY", str(path)))
        if path.exists() and path.is_file():
            return BinaryInfo(path=path, transport=Transport.GRPC, name="env")

    # 2-5. Search in target directories
    for name, rel_path, transport in BINARY_SEARCH_ORDER:
        path = base / rel_path
        searched.append((name, str(path)))
        if path.exists() and path.is_file():
            return BinaryInfo(path=path, transport=transport, name=name)

    # 4. System PATH
    for bin_name in ["reovim"]:
        which = shutil.which(bin_name)
        searched.append((f"path:{bin_name}", which or "(not found)"))
        if which:
            return BinaryInfo(path=Path(which), transport=Transport.GRPC, name=f"path:{bin_name}")

    raise BinaryNotFoundError(searched)


def find_free_port() -> int:
    """Get an OS-assigned free port.

    Uses the socket trick: bind to port 0, OS assigns a free port,
    then close the socket and return the port number.
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        s.listen(1)
        port = s.getsockname()[1]
    return port


def find_project_root(start: Path | None = None) -> Path | None:
    """Find project root by looking for Cargo.toml."""
    current = start or Path.cwd()
    for parent in [current] + list(current.parents):
        if (parent / "Cargo.toml").exists():
            return parent
    return None
