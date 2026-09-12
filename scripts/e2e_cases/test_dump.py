import socket

from .fixture import Reply, Scenario


class Dump(Scenario):
    def boot_arguments(self):
        self.exports = self.work / "exports"
        self.exports.mkdir()
        return [*super().boot_arguments(), "--export-root", str(self.exports)]

    def test_finite_subtree_bytes_unsupported_parent_and_exclusive_output(self):
        dump = self.load_command("dump", "dump")
        env = self.load_command("env", "", imports="")
        payload = b"\0\xffopaque\n"
        self.rpc("write", "module/env/create", "env.create", "1",
                 (b"payload\0" + payload).hex())
        report = self.cli("--refs", "env,host/files", "/dump", "tree")
        self.assertTrue(report.startswith("partial\n"), report)
        self.assertIn("unsupported\t\t", report)
        self.assertIn("written\tchildren/payload/\t", report)
        root, = self.exports.glob("run-*/client-*/tree")
        self.assertEqual((root / "children/payload/value.bin").read_bytes(), payload)
        self.assertEqual((root / "children/CWD/value.bin").read_bytes(), b"")
        repeated = self.cli("--refs", "env,host/files", "/dump", "tree")
        self.assertTrue(repeated.startswith("partial\nincomplete\t"), repeated)
        self.assertEqual((root / "children/payload/value.bin").read_bytes(), payload)
        peer = self.new_client()
        request = self.wire("reserve").split("\t")[1]
        frame = "\t".join(("v1", self.client, "call", request, "write", "dump",
                           "command.dump", "1", b"tree".hex(), "env,host/files"))
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as stream:
            stream.sendall((frame + "\n").encode())
        observed = self.rpc("read", "modules/catalog/counter",
                            "kernel.module.info", "1", client=peer)
        self.assertEqual(observed.data[4:], b"counter")
        failed = Reply.parse(self.wire("result", request))
        self.assertTrue(failed.data.startswith(b"partial\nincomplete\t"))
        self.assertEqual(self.wire("release", request), "ok")
        self.disconnect(peer)
        complete = self.cli("--refs", "env/payload,host/files", "/dump", "single")
        self.assertTrue(complete.startswith("complete\nwritten\t\t"), complete)
        self.assertEqual((root.parent / "single/value.bin").read_bytes(), payload)
        self.cli("--refs", "env,host/files", "/dump", "../escape", success=False)
        self.assertFalse((root.parent / "escape").exists())
        self.assertEqual(self.rpc("read", "env/payload", "resource.bytes", "1").data,
                         payload)
        self.unload(env)
        self.unload(dump)

    def test_capture_pressure_rejects_before_output(self):
        self.load_command("dump", "dump")
        self.load_command("env", "", imports="")
        self.rpc("write", "module/env/create", "env.create", "1",
                 (b"large\0" + b"x" * 16385).hex())
        self.cli("--refs", "env/large,host/files", "/dump", "large", success=False)
        root, = self.exports.glob("run-*")
        self.assertFalse((root / "client-1").exists())
