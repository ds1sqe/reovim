from .fixture import Scenario


class Diagnostics(Scenario):
    def test_transitions_cursor_and_responsive_root(self):
        dmesg = self.load_command("dmesg", "dmesg", imports="kernel/log")
        close_command = self.load_command("close", "close")
        module = self.activate("editing")
        initial = self.call("write", "models/heading", "heading.initial", "1",
                            b"# diagnostic source".hex())
        buffer, _ = initial.resources
        self.call("write", buffer, "heading.replace", "1",
                  (1).to_bytes(8, "little").hex() + b"stale".hex(),
                  buffer, success=False)
        log = self.cli("dmesg")
        self.assertTrue(log.startswith("lost="), log)
        self.assertIn("Accepted", log)
        self.assertIn("Failed", log)
        cursor = log.splitlines()[-1].removeprefix("next=")
        later = self.cli("dmesg", cursor)
        for event in later.splitlines()[1:-1]:
            self.assertGreater(int(event.split()[0]), int(cursor))
        self.cli("close", "/", success=False)
        self.call("read", "kernel/log", "kernel.diagnostics", "1")
        close = ((0).to_bytes(8, "little") + b"\x01").hex()
        self.call("write", buffer, "kernel.buffer.close", "1", close, buffer)
        self.unload(module)
        self.unload(close_command)
        self.unload(dmesg)
