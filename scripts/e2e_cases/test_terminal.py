"""Interactive prompt failures must leave absolute-path recovery available."""

import os
import pty
import select
import subprocess
import time

from .fixture import Scenario


class Terminal(Scenario):
    def test_invalid_cwd_can_be_repaired_without_exiting_the_terminal(self):
        for name in ("env", "cd", "pwd", "exit"):
            self.load_command(name, "" if name == "env" else name)
        self.cli("--contract", "resource.bytes", "1", "--hex", "ff", "/env/CWD")
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        process = subprocess.Popen(
            [str(self.binaries / "reovim-cli"), self.address,
             "--client", self.client], stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        self.addCleanup(self.stop_process, process)
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
        output = bytearray()
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
        self.transcript.write(f"TTY: {bytes(output)!r}\n")
        self.transcript.flush()
        return bytes(output)
