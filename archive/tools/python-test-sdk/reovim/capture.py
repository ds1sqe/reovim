"""Capture - Immutable snapshot of editor state."""

from __future__ import annotations

import json
import time
from dataclasses import dataclass, field, asdict
from typing import Any


@dataclass(frozen=True)
class Cursor:
    """Cursor position in a buffer (0-indexed)."""

    line: int
    col: int

    def __str__(self) -> str:
        return f"Cursor(line={self.line}, col={self.col})"


@dataclass(frozen=True)
class Register:
    """Content of a vim register."""

    name: str
    content: str
    type: str = "char"  # "char", "line", "block"

    def __str__(self) -> str:
        preview = self.content[:20] + "..." if len(self.content) > 20 else self.content
        return f"Register({self.name!r}: {preview!r})"


@dataclass(frozen=True)
class Capture:
    """Immutable snapshot of editor state.

    Captures everything: frame buffer, mode, cursor, buffer content, registers.
    Perfect for LLM-driven testing where you need structured state.
    """

    label: str
    frame: str  # Raw ANSI screen content from TUI
    mode: str
    cursor: Cursor
    buffer: str
    registers: dict[str, Register] = field(default_factory=dict)
    timestamp: float = field(default_factory=time.time)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for serialization."""
        return {
            "label": self.label,
            "frame": self.frame,
            "mode": self.mode,
            "cursor": {"line": self.cursor.line, "col": self.cursor.col},
            "buffer": self.buffer,
            "registers": {
                name: {"name": reg.name, "content": reg.content, "type": reg.type}
                for name, reg in self.registers.items()
            },
            "timestamp": self.timestamp,
        }

    def to_json(self, indent: int = 2) -> str:
        """Convert to JSON string."""
        return json.dumps(self.to_dict(), indent=indent)

    def print_frame(self) -> None:
        """Print raw ANSI frame to terminal (for visual inspection)."""
        print(self.frame)

    def save_frame(self, path: str) -> None:
        """Save raw ANSI frame to file for later replay/diff."""
        with open(path, "w") as f:
            f.write(self.frame)

    def __str__(self) -> str:
        """Human-readable summary."""
        return (
            f"Capture({self.label!r}, mode={self.mode}, "
            f"cursor={self.cursor.line}:{self.cursor.col}, "
            f"buffer={len(self.buffer)} chars, "
            f"frame={len(self.frame)} bytes)"
        )
