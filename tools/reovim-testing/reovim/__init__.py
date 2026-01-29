"""Elegant Testing system for reovim

Zero-config, type-safe, just works.

Example:
    >>> from reovim import Editor, edit
    >>>
    >>> # One-liner magic
    >>> result = edit("hello world", "dw")  # Returns "world"
    >>>
    >>> # Full control
    >>> with Editor() as e:
    ...     e.keys("iHello<Esc>")
    ...     print(e.mode)    # "NORMAL"
    ...     print(e.buffer)  # "Hello"
    ...     e.assert_mode("normal").assert_buffer("Hello")
"""

from reovim.capture import Capture, Cursor, Register
from reovim.editor import Editor, edit, test
from reovim.errors import (
    AssertionError,
    BinaryNotFoundError,
    ConnectionError,
    ReovimError,
    ServerError,
    TimeoutError,
)

__version__ = "1.0.0"

__all__ = [
    # Main class
    "Editor",
    # Types
    "Capture",
    "Cursor",
    "Register",
    # Errors
    "ReovimError",
    "BinaryNotFoundError",
    "ConnectionError",
    "ServerError",
    "TimeoutError",
    "AssertionError",
    # Convenience
    "edit",
    "test",
]
