"""Commands use PATH while their Resource arguments use CWD."""

from .fixture import Scenario


class Navigation(Scenario):
    def boot_arguments(self):
        args = super().boot_arguments()
        for name in ("env", "tree", "ls", "cd", "pwd", "read", "write", "exit"):
            library = self.build_module("reovim-module-" + name, name)
            mount = "" if name == "env" else "bin/" + name
            args.extend(["--client-module", name, str(library), mount,
                         "--grant", "", "all", "--grant", "modules", "all",
                         "--grant", "models", "all", "--grant", "session", "all",
                         "--client-start", name, "1"])
        return args

    def test_tree_and_bare_commands_remain_independent_of_cwd(self):
        self.rpc("write", "module/env/create", "env.create", "1",
                 b"MY_OPTION\0value".hex())
        self.assertEqual(self.cli("pwd"), "/\n")
        root = self.cli("ls").splitlines()
        self.assertIn("bin", root)
        self.assertNotIn("ls", root)
        self.assertNotIn("pwd", root)
        self.cli("cd", "/env")
        self.assertEqual(self.cli("pwd"), "/env\n")
        self.assertIn("MY_OPTION", self.cli("ls").splitlines())
        self.assertEqual(self.cli("tree"),
                         "/env\n├── CWD\n│   └── info\n└── MY_OPTION\n")
        self.assertTrue(self.cli("tree", "/").startswith("/\n"))
        self.assertIn("bin", self.cli("ls", "/").splitlines())
        self.cli("cd", "/bin")
        self.assertEqual(self.cli("./pwd"), "/bin\n")
        self.assertEqual(self.cli("/bin/pwd"), "/bin\n")

    def test_path_order_empty_path_and_explicit_repair(self):
        library = self.build_module("reovim-module-example", "example")
        self.load(library, "bin/hello")
        self.load(library, "extras/hello")
        self.rpc("write", "module/env/create", "env.create", "1",
                 b"PATH\0/missing:/bin:/extras".hex())
        self.cli("cd", "/models")
        self.assertEqual(self.cli("hello", "Reader"), "Hello, Reader!\n")
        self.assertEqual(int.from_bytes(self.rpc("read", "bin/hello/statistics",
                         "example.calls", "1").data, "little"), 1)
        self.assertEqual(int.from_bytes(self.rpc("read", "extras/hello/statistics",
                         "example.calls", "1").data, "little"), 0)
        self.rpc("write", "env/PATH", "resource.bytes", "1", b"/extras:/bin".hex())
        self.cli("hello")
        self.assertEqual(int.from_bytes(self.rpc("read", "extras/hello/statistics",
                         "example.calls", "1").data, "little"), 1)
        self.rpc("write", "module/env/create", "env.create", "1",
                 b"hello\0not a command".hex())
        self.rpc("write", "env/PATH", "resource.bytes", "1", b"/env:/bin".hex())
        self.cli("hello", success=False)
        self.assertEqual(int.from_bytes(self.rpc("read", "bin/hello/statistics",
                         "example.calls", "1").data, "little"), 1)
        self.rpc("write", "env/PATH", "resource.bytes", "1")
        self.cli("pwd", success=False)
        self.assertEqual(self.cli("/bin/pwd"), "/models\n")
        self.rpc("write", "env/PATH", "resource.bytes", "1", b"relative".hex())
        self.cli("pwd", success=False)
        self.cli("/bin/write", "/env/PATH", "--contract", "resource.bytes", "1",
                 "--hex", b"/bin".hex())
        self.assertEqual(self.cli("pwd"), "/models\n")

    def test_tree_borrowed_root_is_passive_and_depth_is_bounded(self):
        example = self.build_module("reovim-module-example", "example")
        self.load(example, "sample")
        tree = self.build_module("reovim-module-tree", "tree")
        self.load(tree, "borrowed-tree")
        result = self.cli("--refs", "/sample", "/borrowed-tree", "@0")
        self.assertTrue(result.startswith("@0\n"), result)
        self.assertIn("statistics\n", result)
        self.assertEqual(int.from_bytes(self.rpc("read", "sample/statistics",
                         "example.calls", "1").data, "little"), 0)
        self.load(example, "deep/" + "n/" * 20 + "leaf")
        limited = self.cli("tree", "/deep")
        self.assertIn("tree limit reached; children not expanded", limited)
        self.assertNotIn("leaf", limited)
