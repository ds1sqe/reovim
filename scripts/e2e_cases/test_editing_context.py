from .fixture import Scenario
from .test_native_source import u64


def initial(active):
    name = b"editing.context.initial"
    return ((1).to_bytes(4, "little") + len(name).to_bytes(4, "little")
            + name + u64(active)).hex()


class EditingContext(Scenario):
    def boot_arguments(self):
        context = self.build_module("reovim-module-context", "context")
        return ["--module", "context", str(context), "models/context",
                "--start", "editing.context.model", "1",
                "--session", "models/context", "editing.context.initial", "1",
                u64(2**64 - 1).hex()]

    def context(self, subject="session", client=None):
        return self.rpc("read", subject, "editing.context", "1", client=client)

    def release(self, observed, client=None):
        self.rpc("write", observed.resources[1], "kernel.session.release", "1",
                 client=client)

    def test_shared_buffers_have_independent_focus_and_retained_context(self):
        source = self.load(self.build_module("reovim-module-source", "source"),
                           "native/source")
        one = self.rpc("write", "native/source", "source.initial", "1", b"one".hex())
        two = self.rpc("write", "native/source", "source.initial", "1", b"two".hex())
        buffers = (one.resources[0], two.resources[0])
        before = self.context()
        changed = self.rpc("write", before.resources[0], "editing.context.update", "1",
                           u64(1).hex(), ",".join((*before.resources[:2], *buffers)))
        self.assertEqual(changed.resources[2:], buffers)
        self.assertEqual(changed.data, u64(1))
        self.call("write", before.resources[0], "editing.context.update", "1",
                  u64(0).hex(), ",".join((*before.resources[:2], *buffers)), success=False)
        self.call("write", changed.resources[0], "editing.context.update", "1",
                  u64(0).hex(), ",".join((*changed.resources[:2], buffers[0], buffers[0])),
                  success=False)
        self.call("write", changed.resources[0], "editing.context.update", "1",
                  u64(2).hex(), ",".join((*changed.resources[:2], *buffers)), success=False)
        peer = self.new_client()
        shared = self.context(client=peer)
        self.assertEqual(shared.data, u64(1))
        self.assertEqual(self.rpc("read", shared.resources[2], "source.bytes", "1",
                                  client=peer).data[16:], u64(3))
        request = self.wire("reserve", client=peer).split("\t")[1]
        denied = self.wire("call", request, "write", shared.resources[2], "source.splice",
                           "1", (u64(0) + u64(0) + u64(0) + b"x").hex(),
                           shared.resources[2], client=peer)
        self.assertTrue(denied.startswith("error\trejected\t1\t"), denied)
        self.wire("release", request, client=peer)
        own = self.rpc("write", "models/context", "kernel.session.create", "1",
                       initial(0), shared.resources[3], client=peer)
        self.assertEqual(own.resources[2:], (shared.resources[3],))
        self.assertEqual(own.data, u64(0))
        unchanged = self.context()
        self.assertEqual(unchanged.data, u64(1))
        self.assertEqual(unchanged.resources[2:], buffers)
        empty = self.rpc("write", changed.resources[0], "editing.context.update", "1",
                         u64(2**64 - 1).hex(), ",".join(changed.resources[:2]))
        retained = self.rpc("read", changed.resources[1], "editing.context", "1")
        self.assertEqual(retained.resources, buffers)
        self.assertEqual(retained.data, u64(1))
        self.rpc("write", own.resources[0], "kernel.session.close", "1", client=peer)
        old = self.rpc("read", own.resources[1], "editing.context", "1", client=peer)
        self.assertEqual(old.resources, (shared.resources[3],))
        for buffer in buffers:
            descriptor = self.rpc("read", buffer, "kernel.buffer.describe", "1")
            self.assertEqual(descriptor.data, u64(0))
            self.rpc("write", buffer, "kernel.buffer.close", "1",
                     (u64(0) + b"\x01").hex(), buffer)
        self.call("write", empty.resources[0], "editing.context.update", "1",
                  u64(0).hex(), ",".join((*empty.resources[:2], buffers[0])), success=False)
        self.unload(source, success=False)
        for snapshot in (before, changed, unchanged, empty):
            self.release(snapshot)
        self.release(shared, peer)
        self.release(own, peer)
        self.unload(source)
