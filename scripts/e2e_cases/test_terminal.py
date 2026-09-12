"""Interactive prompt failures must leave absolute-path recovery available."""

import os
import pty
import select
import subprocess
import time

from .fixture import Scenario


class Terminal(Scenario):
    def open_terminal(self):
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        process = subprocess.Popen(
            [str(self.binaries / "reovim-cli"), self.address,
             "--client", self.client], stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color"})
        os.close(slave)
        self.addCleanup(self.stop_process, process)
        return master, process

    def test_tab_history_cursor_editing_and_interrupt_in_a_real_terminal(self):
        for name in ("env", "cd", "pwd", "ls"):
            self.load_command(name, "" if name == "env" else "bin/" + name)
        self.load_command("example", "bin/hello")
        self.rpc("write", "module/env/create", "env.create", "1",
                 b"TARGET\0data".hex())
        master, process = self.open_terminal()
        self.read_prompt(master, b"resource:/> ")
        os.write(master, b"he\t")
        self.read_prompt(master, b"hello")
        self.assert_calls(0)  # Completion itself performs no command write.
        os.write(master, b" Reader\n")
        self.read_prompt(master, b"Hello, Reader!")
        self.read_prompt(master, b"resource:/> ")
        os.write(master, b"\x1b[A\n")  # Up recalls the prior command.
        self.read_prompt(master, b"Hello, Reader!")
        self.read_prompt(master, b"resource:/> ")
        self.assert_calls(2)
        os.write(master, b"h\x1b[A\x1b[B\t Again\n")
        self.read_prompt(master, b"Hello, Again!")  # Down restores the draft.
        self.read_prompt(master, b"resource:/> ")
        self.assert_calls(3)
        os.write(master, b"pwdx\x1b[D\x1b[3~\n")  # Left, Delete, Enter.
        self.read_prompt(master, b"\r\n/\r\n")
        self.read_prompt(master, b"resource:/> ")
        os.write(master, b"ls /en\t\n")
        self.read_prompt(master, b"TARGET\r\n")
        self.read_prompt(master, b"resource:/> ")
        os.write(master, b"cd /env\n")
        self.read_prompt(master, b"resource:/env> ")
        os.write(master, b"p\t\n")
        self.read_prompt(master, b"\r\n/env\r\n")
        self.read_prompt(master, b"resource:/env> ")
        os.write(master, b"hello cancelled\x03")
        self.read_prompt(master, b"resource:/env> ")
        self.assert_calls(3)
        self.assertIsNone(process.poll())
        os.write(master, b"\x04")
        self.assertEqual(process.wait(timeout=5), 0)

    def assert_calls(self, expected):
        observed = self.rpc("read", "bin/hello/statistics", "example.calls", "1")
        self.assertEqual(int.from_bytes(observed.data, "little"), expected)

    def test_invalid_cwd_can_be_repaired_without_exiting_the_terminal(self):
        for name in ("env", "cd", "pwd", "exit"):
            self.load_command(name, "" if name == "env" else name)
        self.cli("--contract", "resource.bytes", "1", "--hex", "ff", "/env/CWD")
        master, process = self.open_terminal()
        self.read_prompt(master, b"resource:?> ")
        self.assertIsNone(process.poll())
        os.write(master, b"/cd /\n")
        self.read_prompt(master, b"resource:/> ")
        os.write(master, b"/pwd\n")
        output = self.read_prompt(master, b"resource:/> ")
        self.assertIn(b"\r\n/\r\n", output)
        os.write(master, b"/exit\n")
        self.assertEqual(process.wait(timeout=5), 0)
        self.assertEqual(self.rpc("read", "env/CWD", "resource.bytes", "1").data,
                         b"")

    def read_prompt(self, master, expected):
        output = bytearray(getattr(self, "pending_terminal", b""))
        deadline = time.monotonic() + 5
        while expected not in output:
            remaining = deadline - time.monotonic()
            self.assertGreater(remaining, 0, bytes(output))
            self.assertLess(len(output), 65536)
            ready, _, _ = select.select([master], [], [], remaining)
            self.assertTrue(ready, bytes(output))
            chunk = os.read(master, 4096)
            self.assertTrue(chunk, bytes(output))
            output.extend(chunk)
        end = output.index(expected) + len(expected)
        self.pending_terminal = bytes(output[end:])
        output = output[:end]
        self.transcript.write(f"TTY: {bytes(output)!r}\n")
        self.transcript.flush()
        return bytes(output)
