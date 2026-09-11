from .fixture import Reply, Scenario


class NativeCommands(Scenario):
    def test_separate_crates_use_generic_calls_and_borrowed_references(self):
        mounted = {}
        for name in ("read", "write", "ls", "pwd", "help", "exit", "dmesg",
                     "cat"):
            library = self.build_module("reovim-module-" + name, name)
            payload = (str(library) + "\0tools/" + name).encode().hex()
            if name in ("cat", "exit"):
                result = self.rpc("write", "kernel/modules/load",
                                  "host.module.load", "1", payload)
            else:
                result = self.rpc("write", "kernel/modules/load",
                                  "host.module.load-with", "1", payload, "")
            mounted[name] = result.resources[0]

        activated = Reply.parse(self.cli(
            "/tools/write", "/modules/catalog/counter", "--contract",
            "kernel.module.activate", "1"))
        counter = "/modules/counter/value"
        changed = Reply.parse(self.cli(
            "/tools/write", counter, "--contract", "external.counter", "1",
            "--hex", (7).to_bytes(8, "little").hex()))
        self.assertEqual(int.from_bytes(changed.data, "little"), 7)
        observed = Reply.parse(self.cli(
            "/tools/read", counter, "--contract", "external.counter", "1"))
        self.assertEqual(observed, changed)
        self.cli("/tools/write", counter, "--contract", "external.counter",
                 "1", "--hex", "ff", success=False)

        listing = Reply.parse(self.cli("/tools/write", "/tools/ls"))
        self.assertEqual(listing.contract, "command.ls")
        self.assertIn(b"tools\n", listing.data)
        self.assertEqual(self.cli("/tools/pwd"), "/\n")
        help_text = self.cli("/tools/help", "/tools")
        self.assertIn("command.write", help_text)
        self.assertIn("command.read", help_text)
        self.assertIn("Admitted", self.cli("/tools/dmesg"))

        copied = Reply.parse(self.cli(
            "--refs", "/tools/write/info", "/tools/cat"))
        self.assertIn(b"command.write", copied.data)
        read_back = Reply.parse(self.cli(
            "--refs", copied.resources[0], "/tools/read", "@0"))
        self.assertEqual(read_back.data, copied.data)
        self.assertEqual(self.cli(input="/tools/exit\n/tools/pwd\n"), "")

        self.cli("--refs", activated.resources[0], "/tools/write", "@0",
                 "--contract", "kernel.module.unload", "1")
        self.cli("/tools/read", counter, "--contract", "external.counter",
                 "1", success=False)
        for instance in mounted.values():
            self.cli("--contract", "kernel.module.unload", "1", instance)
        self.cli("/tools/pwd", success=False)
