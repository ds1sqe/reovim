from .fixture import Reply, Scenario
from .test_native_source import u64


def state(reply):
    size = int.from_bytes(reply.data[12:16], "little")
    name = reply.data[16:16 + size]
    return int.from_bytes(reply.data[:8], "little"), name, reply.data[16 + size:]


class ContextWatch(Scenario):
    def boot_arguments(self):
        context = self.build_module("reovim-module-context", "context")
        return ["--module", "context", str(context), "models/context",
                "--start", "editing.context.model", "1",
                "--session", "models/context", "editing.context.initial", "1",
                u64(2**64 - 1).hex()]

    def observe(self, subject="session", client=None):
        value = self.rpc("read", subject, "kernel.session.observe", "1", client=client)
        self.assertEqual(state(value)[1], b"editing.context")
        return value

    def close_watch(self, observed, client=None):
        self.rpc("write", observed.resources[1], "kernel.session.release", "1", client=client)
        request = self.wire("reserve", client=client).split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", observed.resources[2],
                                   client=client), "ok")
        self.wire("release", request, client=client)

    def test_native_context_watch_gap_renewal_replay_and_read_only_peer(self):
        source = self.load(self.build_module("reovim-module-source", "source"), "native/source")
        buffer = self.rpc("write", "native/source", "source.initial", "1",
                          b"shared".hex()).resources[0]
        slow = self.observe()
        fast = self.observe()
        self.assertEqual(state(slow), state(fast))
        current = self.rpc("read", "session", "editing.context", "1")
        revision = state(fast)[0]
        for index in range(34):
            active = 0 if index % 2 else 2**64 - 1
            changed = self.rpc("write", current.resources[0], "editing.context.update", "1",
                               u64(active).hex(), ",".join((*current.resources[:2], buffer)))
            self.rpc("write", current.resources[1], "kernel.session.release", "1")
            current = changed
            event = self.rpc("read", fast.resources[2], "kernel.session.next", "1", "00")
            self.assertEqual(event.data[0], 0)
            self.assertEqual(int.from_bytes(event.data[9:17], "little"), revision)
            revision = int.from_bytes(event.data[17:25], "little")
        self.assertEqual(self.rpc("read", slow.resources[2], "kernel.session.next", "1", "00").data,
                         b"\x01")
        renewed = self.observe()
        self.assertEqual(state(renewed), (revision, b"editing.context", current.data))
        self.assertEqual(renewed.resources[3:], (buffer,))
        peer = self.new_client()
        remote = self.observe(client=peer)
        descriptor = self.rpc("read", remote.resources[3], "kernel.buffer.describe", "1", client=peer)
        self.assertEqual(descriptor.data, u64(0))
        request = self.wire("reserve", client=peer).split("\t")[1]
        denied = self.wire("call", request, "write", remote.resources[3], "source.splice", "1",
                           (u64(0) + u64(0) + u64(0) + b"x").hex(), remote.resources[3], client=peer)
        self.assertTrue(denied.startswith("error\trejected\t1\t"), denied)
        self.wire("release", request, client=peer)
        self.close_watch(remote, peer)
        request = self.wire("reserve").split("\t")[1]
        fields = ("call", request, "read", "session", "kernel.session.observe", "1")
        first = self.wire(*fields)
        changed = self.rpc("write", current.resources[0], "editing.context.update", "1",
                           u64(2**64 - 1).hex(), ",".join((*current.resources[:2], buffer)))
        self.assertEqual(self.wire(*fields), first)
        self.wire("release", request)
        lost = Reply.parse(first)
        event = self.rpc("read", lost.resources[2], "kernel.session.next", "1", "00")
        self.assertEqual(int.from_bytes(event.data[9:17], "little"), state(lost)[0])
        empty = self.rpc("write", changed.resources[0], "editing.context.update", "1",
                         u64(2**64 - 1).hex(), ",".join(changed.resources[:2]))
        for item in (current, changed, empty):
            self.rpc("write", item.resources[1], "kernel.session.release", "1")
        for observed in (slow, fast, renewed, lost):
            self.close_watch(observed)
        self.rpc("write", buffer, "kernel.buffer.close", "1", (u64(0) + b"\x01").hex(), buffer)
        self.unload(source)
