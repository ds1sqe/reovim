from .fixture import Scenario


class NativeBacking(Scenario):
    def test_granted_backing_survives_buffer_close_and_rejects_changed_bytes(self):
        env = self.build_module("reovim-module-env", "env")
        self.load(env, "", imports="")
        created = self.rpc("write", "module/env/create", "env.create", "1",
                           b"BACKING\0I".hex())
        source = created.resources[0]
        library = self.build_c("apps/server/tests/native/backed.c", "backed")
        module = self.load(library, "native/backed", imports="")
        opened = self.call("write", "native/backed", "backed.initial", "1",
                           "", source)
        buffer, _ = opened.resources
        self.assertEqual(opened.data[16:], b"I")
        snapshot = self.call("read", buffer, "kernel.buffer.retain", "1",
                             (0).to_bytes(8, "little").hex()).resources[1]
        self.call("write", buffer, "kernel.buffer.close", "1",
                  ((0).to_bytes(8, "little") + b"\x01").hex(), buffer)
        self.unload(module, success=False)
        self.assertEqual(self.call("read", snapshot, "backed.value", "1").data,
                         b"I")
        self.rpc("write", source, "resource.bytes", "1", b"J".hex())
        self.call("read", snapshot, "backed.value", "1", success=False)
        self.rpc("write", source, "resource.bytes", "1", b"I".hex())
        self.assertEqual(self.call("read", snapshot, "backed.value", "1").data,
                         b"I")
        request = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", snapshot), "ok")
        self.assertEqual(self.wire("release", request), "ok")
        self.unload(module)
