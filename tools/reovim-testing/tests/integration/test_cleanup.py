"""Test process cleanup and zombie prevention.

Integration tests that verify proper cleanup of server and TUI processes.
"""

from __future__ import annotations

import subprocess
import time
import pytest

from reovim import Editor
from reovim.process import ServerProcess, TuiProcess


class TestEditorCleanup:
    """Test Editor cleanup after normal and exceptional exits."""

    def test_cleanup_on_normal_exit(self, binary_info):
        """Server and TUI should be stopped after context manager exits."""
        server_pid = None
        tui_pid = None

        with Editor() as e:
            # Capture PIDs
            if e._server and e._server._proc:
                server_pid = e._server._proc.pid
            if e._tui and e._tui._proc:
                tui_pid = e._tui._proc.pid

            assert server_pid is not None, "Server should have a PID"
            # TUI might not have PID if it exited quickly

        # After exit, processes should be gone
        time.sleep(0.5)  # Give OS time to clean up

        if server_pid:
            assert not _pid_exists(server_pid), f"Server PID {server_pid} still exists"

    def test_cleanup_on_exception(self, binary_info):
        """Should clean up even when exception occurs."""
        server_pid = None

        try:
            with Editor() as e:
                if e._server and e._server._proc:
                    server_pid = e._server._proc.pid

                raise ValueError("Test exception")
        except ValueError:
            pass  # Expected

        time.sleep(0.5)

        if server_pid:
            assert not _pid_exists(server_pid), f"Server PID {server_pid} still exists after exception"

    def test_multiple_editors_cleanup(self, binary_info):
        """Multiple sequential editors should all clean up."""
        pids = []

        for i in range(3):
            with Editor() as e:
                if e._server and e._server._proc:
                    pids.append(e._server._proc.pid)
                e.keys("itest<Esc>")

        time.sleep(0.5)

        for pid in pids:
            assert not _pid_exists(pid), f"PID {pid} still exists"


class TestServerProcessCleanup:
    """Test ServerProcess stop() behavior."""

    def test_stop_terminates_process(self, binary_info, free_port: int):
        """stop() should terminate the server process."""
        server = ServerProcess(binary=binary_info, port=free_port)
        server.start()

        assert server.is_running
        pid = server._proc.pid

        server.stop()

        time.sleep(0.5)
        assert not _pid_exists(pid)
        assert not server.is_running

    def test_double_stop_is_safe(self, binary_info, free_port: int):
        """Calling stop() twice should not raise."""
        server = ServerProcess(binary=binary_info, port=free_port)
        server.start()

        server.stop()
        server.stop()  # Should not raise


class TestTuiProcessCleanup:
    """Test TuiProcess stop() behavior."""

    def test_stop_terminates_tui(self, binary_info, free_port: int):
        """stop() should terminate the TUI process."""
        # Start a server first
        server = ServerProcess(binary=binary_info, port=free_port)
        server.start()

        try:
            tui = TuiProcess(
                binary=binary_info,
                address=server.address,
                width=80,
                height=24,
            )
            tui.start()

            if tui._proc:
                pid = tui._proc.pid
                tui.stop()

                time.sleep(0.5)
                assert not _pid_exists(pid)
        finally:
            server.stop()


def _pid_exists(pid: int) -> bool:
    """Check if a process with given PID exists."""
    try:
        # Signal 0 doesn't actually send a signal, just checks if process exists
        import os
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        # Process exists but we don't have permission to signal it
        return True
