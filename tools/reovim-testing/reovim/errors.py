"""Custom exceptions for reovim - helpful, not cryptic."""

from __future__ import annotations


class ReovimError(Exception):
    """Base error with context."""


class BinaryNotFoundError(ReovimError):
    """Binary not found - includes search paths tried."""

    def __init__(self, searched: list[tuple[str, str]]) -> None:
        self.searched = searched
        paths = "\n  ".join(f"[{name}] {path}" for name, path in searched)
        super().__init__(
            f"Could not find reovim binary.\n\n"
            f"Searched locations:\n  {paths}\n\n"
            f"Solutions:\n"
            f"  1. Build: cargo build --release -p reovim-app --features grpc\n"
            f"  2. Set REOVIM_BINARY=/path/to/binary\n"
            f"  3. Add reovim to PATH"
        )


class ServerError(ReovimError):
    """Server failed to start - includes stderr."""

    def __init__(self, message: str, stderr: str = "") -> None:
        self.stderr = stderr
        full_message = message
        if stderr:
            full_message += f"\n\nServer stderr:\n{stderr}"
        super().__init__(full_message)


class ConnectionError(ReovimError):
    """Failed to connect to server."""


class TimeoutError(ReovimError):
    """Operation timed out - includes what was waiting for."""

    def __init__(self, operation: str, timeout: float) -> None:
        self.operation = operation
        self.timeout = timeout
        super().__init__(f"Timeout after {timeout}s waiting for: {operation}")


class AssertionError(ReovimError):
    """Assertion failed - includes expected vs actual."""

    def __init__(self, what: str, expected: str, actual: str) -> None:
        self.what = what
        self.expected = expected
        self.actual = actual
        super().__init__(f"{what} mismatch:\n  expected: {expected!r}\n  actual:   {actual!r}")
