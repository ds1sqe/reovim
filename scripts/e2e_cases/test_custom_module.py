from .fixture import Reply, Scenario


class CustomModule(Scenario):
    def test_own_command_and_resources_through_the_safe_rust_macro(self):
        library = self.build_module("reovim-module-example", "example")
        mount = "extensions/formatter"
        payload = (str(library) + "\0" + mount).encode().hex()
        loaded = Reply.parse(self.cli(
            "--contract", "host.module.load", "1", "--hex", payload,
            "/kernel/modules/load"))
        baseline = self.rpc("read", mount + "/statistics", "example.calls", "1")
        self.assertEqual(int.from_bytes(baseline.data, "little"), 0)
        info = self.rpc("read", mount + "/info", "resource.bytes", "1")
        self.assertIn(b"example.greet", info.data)
        self.assertEqual(self.cli("/" + mount), "Hello, world!\n")
        self.assertEqual(self.cli("/" + mount, "Ada"), "Hello, Ada!\n")
        statistics = self.rpc("read", mount + "/statistics", "example.calls", "1")
        self.assertEqual(int.from_bytes(statistics.data, "little"), 2)
        self.cli("--contract", "kernel.module.unload", "1", loaded.resources[0])
        self.cli("/" + mount, success=False)
