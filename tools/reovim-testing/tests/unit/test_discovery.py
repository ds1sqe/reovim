"""Test binary discovery and port finding.

Tests the discovery module's ability to find binaries and ports.
Uses mocking to avoid requiring actual reovim binaries.
"""

from __future__ import annotations

import os
import pytest
from pathlib import Path
from unittest.mock import patch

from reovim.discovery import (
    discover_binary,
    find_free_port,
    find_project_root,
    BinaryInfo,
    Transport,
    BINARY_SEARCH_ORDER,
)
from reovim.errors import BinaryNotFoundError


class TestBinaryInfo:
    """Test BinaryInfo dataclass."""

    def test_str_format(self):
        """String representation should include name, transport, and path."""
        info = BinaryInfo(
            path=Path("/test/reovim-new"),
            transport=Transport.GRPC,
            name="release-new",
        )

        s = str(info)
        assert "release-new" in s
        assert "GRPC" in s
        assert "/test/reovim-new" in s

    def test_tcp_transport(self):
        """Should handle TCP transport correctly."""
        info = BinaryInfo(
            path=Path("/test/reovim"),
            transport=Transport.TCP,
            name="release",
        )

        assert info.transport == Transport.TCP
        assert "TCP" in str(info)


class TestDiscoverBinary:
    """Test binary discovery logic."""

    def test_env_variable_takes_precedence(self, tmp_path: Path):
        """REOVIM_BINARY env var should override all other discovery."""
        binary = tmp_path / "custom-reovim"
        binary.touch()

        with patch.dict(os.environ, {"REOVIM_BINARY": str(binary)}):
            info = discover_binary(tmp_path)

            assert info.path == binary
            assert info.name == "env"

    def test_env_variable_grpc_detection(self, tmp_path: Path):
        """Env var binary name 'reovim-new' should use GRPC transport."""
        binary = tmp_path / "reovim-new"
        binary.touch()

        with patch.dict(os.environ, {"REOVIM_BINARY": str(binary)}):
            info = discover_binary(tmp_path)

            assert info.transport == Transport.GRPC

    def test_env_variable_tcp_detection(self, tmp_path: Path):
        """Env var binary name 'reovim' should use TCP transport."""
        binary = tmp_path / "reovim"
        binary.touch()

        with patch.dict(os.environ, {"REOVIM_BINARY": str(binary)}):
            info = discover_binary(tmp_path)

            assert info.transport == Transport.TCP

    def test_env_variable_nonexistent_file(self, tmp_path: Path):
        """Non-existent env var path should fall through to next search."""
        # Create a valid binary in target/
        (tmp_path / "target" / "release").mkdir(parents=True)
        valid_binary = tmp_path / "target" / "release" / "reovim-new"
        valid_binary.touch()

        with patch.dict(os.environ, {"REOVIM_BINARY": "/nonexistent/path"}):
            info = discover_binary(tmp_path)

            # Should find the target/ binary instead
            assert info.path == valid_binary

    def test_search_order_prefers_release_new(self, tmp_path: Path):
        """Should prefer release/reovim-new over other options."""
        # Create both release and debug
        (tmp_path / "target" / "release").mkdir(parents=True)
        (tmp_path / "target" / "debug").mkdir(parents=True)

        release_new = tmp_path / "target" / "release" / "reovim-new"
        debug_new = tmp_path / "target" / "debug" / "reovim-new"
        release_old = tmp_path / "target" / "release" / "reovim"

        release_new.touch()
        debug_new.touch()
        release_old.touch()

        with patch.dict(os.environ, {"REOVIM_BINARY": ""}, clear=False):
            with patch.dict(os.environ, {"REOVIM_BINARY": ""}):
                os.environ.pop("REOVIM_BINARY", None)
                info = discover_binary(tmp_path)

                assert info.path == release_new
                assert info.name == "release-new"

    def test_not_found_raises_with_searched_paths(self, tmp_path: Path):
        """Should raise BinaryNotFoundError with list of searched paths."""
        # Empty directory with no binaries
        with patch.dict(os.environ, {}, clear=True):
            # Also mock shutil.which to return None
            with patch("reovim.discovery.shutil.which", return_value=None):
                with pytest.raises(BinaryNotFoundError) as exc_info:
                    discover_binary(tmp_path)

                err = exc_info.value
                assert len(err.searched) > 0
                # Should have searched the standard locations
                names = [name for name, _ in err.searched]
                assert "release-new" in names

    def test_system_path_fallback(self, tmp_path: Path):
        """Should fall back to system PATH if target/ not found."""
        # Empty directory
        system_binary = "/usr/local/bin/reovim-new"

        with patch.dict(os.environ, {}, clear=True):
            with patch("reovim.discovery.shutil.which", return_value=system_binary):
                info = discover_binary(tmp_path)

                assert str(info.path) == system_binary
                assert "path" in info.name

    def test_directory_ignored(self, tmp_path: Path):
        """Should ignore directories with binary names."""
        # Create a directory named reovim-new (not a file)
        (tmp_path / "target" / "release").mkdir(parents=True)
        (tmp_path / "target" / "release" / "reovim-new").mkdir()

        # Create actual binary at different location
        (tmp_path / "target" / "debug").mkdir(parents=True)
        actual_binary = tmp_path / "target" / "debug" / "reovim-new"
        actual_binary.touch()

        with patch.dict(os.environ, {}, clear=True):
            with patch("reovim.discovery.shutil.which", return_value=None):
                info = discover_binary(tmp_path)

                # Should skip the directory and find the file
                assert info.path == actual_binary


class TestFindFreePort:
    """Test free port discovery."""

    def test_returns_valid_port(self):
        """Should return a port in valid range."""
        port = find_free_port()

        assert isinstance(port, int)
        assert 1024 <= port <= 65535

    def test_returns_different_ports(self):
        """Multiple calls should return different ports."""
        ports = [find_free_port() for _ in range(5)]

        # Should be unique (high probability)
        assert len(set(ports)) == len(ports)

    def test_port_is_usable(self):
        """Returned port should be bindable."""
        import socket

        port = find_free_port()

        # Try to bind to the port
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            # Should not raise
            s.bind(("127.0.0.1", port))


class TestFindProjectRoot:
    """Test project root discovery."""

    def test_finds_cargo_toml(self, tmp_path: Path):
        """Should find directory containing Cargo.toml."""
        (tmp_path / "Cargo.toml").touch()
        subdir = tmp_path / "src" / "deep" / "nested"
        subdir.mkdir(parents=True)

        result = find_project_root(subdir)

        assert result == tmp_path

    def test_returns_none_if_no_cargo_toml(self, tmp_path: Path):
        """Should return None if no Cargo.toml found."""
        subdir = tmp_path / "src"
        subdir.mkdir()

        result = find_project_root(subdir)

        assert result is None

    def test_finds_immediate_cargo_toml(self, tmp_path: Path):
        """Should find Cargo.toml in start directory."""
        (tmp_path / "Cargo.toml").touch()

        result = find_project_root(tmp_path)

        assert result == tmp_path

    def test_uses_cwd_if_no_start(self, tmp_path: Path, monkeypatch):
        """Should use current working directory if no start provided."""
        (tmp_path / "Cargo.toml").touch()
        monkeypatch.chdir(tmp_path)

        result = find_project_root()

        assert result == tmp_path


class TestBinarySearchOrder:
    """Test the search order configuration."""

    def test_search_order_not_empty(self):
        """Search order should have entries."""
        assert len(BINARY_SEARCH_ORDER) > 0

    def test_grpc_comes_first(self):
        """GRPC binaries should be searched before TCP."""
        grpc_indices = [
            i
            for i, (_, _, transport) in enumerate(BINARY_SEARCH_ORDER)
            if transport == Transport.GRPC
        ]
        tcp_indices = [
            i
            for i, (_, _, transport) in enumerate(BINARY_SEARCH_ORDER)
            if transport == Transport.TCP
        ]

        # First GRPC should come before first TCP
        assert min(grpc_indices) < min(tcp_indices)

    def test_release_before_debug(self):
        """Release builds should be searched before debug."""
        # Find indices of release and debug entries
        for i, (name, _, _) in enumerate(BINARY_SEARCH_ORDER):
            if "release" in name:
                release_idx = i
                break

        for i, (name, _, _) in enumerate(BINARY_SEARCH_ORDER):
            if "debug" in name:
                debug_idx = i
                break

        assert release_idx < debug_idx
