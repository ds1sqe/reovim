"""Shared pytest fixtures for reovim testing.

This module provides fixtures and configuration for the pytest test suite.
Fixtures handle binary discovery, Editor lifecycle, and process cleanup.
"""

from __future__ import annotations

import pytest
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from reovim.discovery import BinaryInfo


# =============================================================================
# Pytest Configuration
# =============================================================================


def pytest_configure(config: pytest.Config) -> None:
    """Register custom markers."""
    config.addinivalue_line(
        "markers", "integration: mark test as integration test (requires running server)"
    )
    config.addinivalue_line("markers", "slow: mark test as slow (takes >5 seconds)")
    config.addinivalue_line(
        "markers", "binary: mark test as requiring compiled reovim binary"
    )


def pytest_collection_modifyitems(
    config: pytest.Config, items: list[pytest.Item]
) -> None:
    """Auto-mark integration tests and conditionally skip if binary missing."""
    # Check if binary exists once
    binary_available = _check_binary_available()

    for item in items:
        # Auto-mark tests in integration/ directory
        if "integration" in str(item.fspath):
            item.add_marker(pytest.mark.integration)
            item.add_marker(pytest.mark.binary)

            # Skip if binary not available
            if not binary_available:
                item.add_marker(
                    pytest.mark.skip(
                        reason="Binary not available - run 'cargo build --release -p reovim-app'"
                    )
                )


def _check_binary_available() -> bool:
    """Check if reovim binary is available for integration tests."""
    try:
        from reovim.discovery import discover_binary, find_project_root

        root = find_project_root()
        if root:
            discover_binary(root)
            return True
    except Exception:
        pass
    return False


# =============================================================================
# Fixtures
# =============================================================================


@pytest.fixture(scope="module")
def binary_info() -> "BinaryInfo":
    """Discover reovim binary once per test module.

    This fixture caches the binary discovery result at module scope for efficiency.
    If the binary is not found, tests using this fixture will be skipped.

    Returns:
        BinaryInfo with path, transport, and name

    Raises:
        pytest.skip: If binary cannot be found
    """
    from reovim.discovery import discover_binary, find_project_root
    from reovim.errors import BinaryNotFoundError

    try:
        root = find_project_root()
        if root is None:
            pytest.skip("Could not find project root (no Cargo.toml)")
        return discover_binary(root)
    except BinaryNotFoundError as e:
        pytest.skip(f"Binary not found: {e}")


@pytest.fixture(scope="function")
def free_port() -> int:
    """Get an OS-assigned free port.

    Returns a port number that was free at the time of calling.
    Note: There's a small race window between getting the port and using it.

    Returns:
        Available port number
    """
    from reovim.discovery import find_free_port

    return find_free_port()


@pytest.fixture(scope="function")
def editor(binary_info: "BinaryInfo"):
    """Provide a started Editor instance with automatic cleanup.

    This fixture creates an Editor, starts it, and ensures proper cleanup
    even if the test fails. Use for integration tests that need a running
    editor session.

    Yields:
        Started Editor instance

    Example:
        def test_insert_text(editor):
            editor.keys("iHello<Esc>")
            assert "Hello" in editor.buffer
    """
    from reovim import Editor

    with Editor() as e:
        yield e
    # Context manager ensures cleanup


@pytest.fixture(scope="function")
def server_process(binary_info: "BinaryInfo", free_port: int):
    """Provide a ServerProcess for lower-level process testing.

    This fixture gives direct access to the server process for tests
    that need to verify process lifecycle, cleanup, or error handling.

    Yields:
        Tuple of (ServerProcess, port)

    Example:
        def test_server_cleanup(server_process):
            server, port = server_process
            assert server.is_running
    """
    from reovim.process import ServerProcess

    server = ServerProcess(binary=binary_info, port=free_port)
    port = server.start()
    yield server, port
    server.stop()


# =============================================================================
# Helper Fixtures
# =============================================================================


@pytest.fixture
def tmp_binary(tmp_path: Path) -> Path:
    """Create a fake binary for discovery testing.

    Creates an executable script in tmp_path that can be used to test
    binary discovery without needing the real reovim binary.

    Returns:
        Path to the fake binary
    """
    binary = tmp_path / "reovim"
    binary.write_text("#!/bin/sh\necho 'mock binary'")
    binary.chmod(0o755)
    return binary


@pytest.fixture
def mock_project_root(tmp_path: Path) -> Path:
    """Create a mock project root with Cargo.toml.

    Returns:
        Path to temporary directory containing Cargo.toml
    """
    cargo_toml = tmp_path / "Cargo.toml"
    cargo_toml.write_text('[package]\nname = "test"\nversion = "0.1.0"\n')
    return tmp_path
