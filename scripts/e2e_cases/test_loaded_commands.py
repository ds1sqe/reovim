from .fixture import Scenario


class LoadedCommands(Scenario):
    def test_callable_info_awaited_reads_and_reference_handoff(self):
        dmesg = self.build_module("reovim-module-dmesg", "dmesg")
        cat = self.build_module("reovim-module-cat", "cat")
        ls = self.load_command("ls", "ls")
        mount = "extensions/arbitrary-log-reader"
        module = self.load(dmesg, mount,
                           imports="kernel/log")
        info = self.call("read", mount + "/info", "resource.bytes", "1")
        self.assertIn(b"write\tcommand.dmesg\t1\tutf8", info.data)
        listing = self.cli("ls", mount)
        self.assertIn("info\n", listing)
        # Omitted arguments still submit one complete request with empty data.
        reply = self.call("write", mount, "command.dmesg", "1")
        log = reply.data.decode()
        self.assertTrue(log.startswith("lost="), log)
        self.assertIn("next=", log)
        self.assertIn("parent=Some(", log)
        self.assertIn("Started", log)
        self.call("write", mount, "command.dmesg", "1", b"bad".hex(),
                  success=False)
        cat_mount = "extensions/borrowed-source-reader"
        cat_module = self.load(cat, cat_mount)
        copied = self.call("write", cat_mount, "command.cat", "1", "-",
                           mount + "/info")
        self.assertEqual(copied.data, info.data)
        returned = self.call("read", copied.resources[0], "resource.bytes", "1")
        self.assertEqual(returned.data, info.data)
        self.call("write", cat_mount, "command.cat", "1", success=False)
        self.unload(cat_module)
        self.unload(module)
        self.call("read", mount + "/info", "resource.bytes", "1", success=False)
        self.unload(ls)
