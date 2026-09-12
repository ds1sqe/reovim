from .fixture import Scenario


class Counter(Scenario):
    def test_activation_navigation_rejection_and_unload(self):
        commands = [self.load_command("env", "")]
        commands.extend(self.load_command(name, name)
                        for name in ("cd", "pwd", "ls", "read", "write", "exit"))
        path, contract = "modules/counter/value", "external.counter"
        self.call("read", path, contract, "1", success=False)
        instance = self.activate("counter")
        self.assertTrue(instance.startswith("@"))
        initial = self.call("read", path, contract, "1")
        self.assertEqual(int.from_bytes(initial.data, "little"), 0)
        added = self.call("write", path, contract, "1",
                          (7).to_bytes(8, "little").hex())
        self.assertEqual(int.from_bytes(added.data, "little"), 7)
        self.call("write", path, contract, "1", "ff", success=False)
        self.call("write", path, contract, "2",
                  (1).to_bytes(8, "little").hex(), success=False)
        unchanged = self.call("read", path, contract, "1")
        self.assertEqual(int.from_bytes(unchanged.data, "little"), 7)
        output = self.cli(input=(
            "/cd modules/counter\n/pwd\n/ls\n"
            "/read value --contract external.counter 1\n"
            "/write value --contract external.counter 1 --hex 0200000000000000\n"
            "/cd ../catalog\n/pwd\n/ls\n/exit\n"))
        self.assertIn("/modules/counter", output)
        self.assertIn("0700000000000000", output)
        self.assertIn("0900000000000000", output)
        self.assertIn("/modules/catalog", output)
        changed = self.call("read", path, contract, "1")
        self.assertEqual(int.from_bytes(changed.data, "little"), 9)
        self.unload(instance)
        self.call("read", path, contract, "1", success=False)
        self.cli("/ls", "/modules/catalog")
        for command in reversed(commands):
            self.unload(command)
