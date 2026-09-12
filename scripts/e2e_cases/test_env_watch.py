import socket
import time

from .fixture import Reply, Scenario


def snapshot(data):
    revision = int.from_bytes(data[:8], "little")
    fields = []
    offset = 8
    while offset < len(data):
        size = int.from_bytes(data[offset:offset + 4], "little")
        offset += 4
        field = data[offset:offset + size]
        if len(field) != size:
            raise ValueError("truncated snapshot field")
        fields.append(field)
        offset += size
    return revision, dict(zip(fields[::2], fields[1::2], strict=True))


class EnvWatch(Scenario):
    def test_close_command_accepts_returned_read_only_watch_reference(self):
        self.load_command("env", "", imports="")
        library = self.build_module("reovim-module-close", "close")
        self.load(library, "close")
        observed = self.rpc("read", "module/env/watch", "env.observe", "1")
        reference, = observed.resources
        self.cli("--refs", reference, "/close", "@0")
        self.call("read", reference, "env.next", "1", success=False)

    def close(self, resource):
        request = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", resource), "ok")
        self.assertEqual(self.wire("release", request), "ok")

    def observe(self):
        return self.rpc("read", "module/env/watch", "env.observe", "1")

    def next(self, reader):
        reply = self.rpc("read", reader, "env.next", "1")
        return reply.data[0], int.from_bytes(reply.data[1:], "little")

    def test_coherent_native_readers_gap_close_and_unload(self):
        library = self.build_module("reovim-module-env", "env")
        loaded = self.rpc("write", "kernel/modules/load", "host.module.load-with",
                          "1", (str(library) + "\0").encode().hex(), "")
        first = self.observe()
        second = self.observe()
        self.assertEqual(snapshot(first.data), (0, {b"CWD": b""}))
        self.assertEqual(snapshot(second.data), snapshot(first.data))
        one, two = first.resources[0], second.resources[0]
        self.assertNotEqual(one, two)
        self.rpc("write", "module/env/create", "env.create", "1", b"x\0initial".hex())
        self.assertEqual(self.next(one), (0, 1))
        # Rejected mutation cannot advance the revision or create a gap.
        rejected = self.wire("reserve").split("\t")[1]
        failure = self.wire("call", rejected, "write", "module/env/create",
                            "env.create", "1", b"x\0duplicate".hex())
        self.assertTrue(failure.startswith("error\t"), failure)
        self.wire("release", rejected)
        self.rpc("write", "env/x", "resource.bytes", "1", b"changed".hex())
        self.assertEqual(self.next(one), (0, 2))
        self.assertEqual(self.next(two), (1, 2))
        stale = self.wire("reserve").split("\t")[1]
        failure = self.wire("call", stale, "read", two, "env.next", "1")
        self.assertTrue(failure.startswith("error\tfailed\t7\t"), failure)
        self.wire("release", stale)
        renewed = self.observe()
        self.assertEqual(snapshot(renewed.data), (2, {b"CWD": b"", b"x": b"changed"}))
        self.close(two)
        self.close(two)
        self.rpc("write", "module/env/remove", "env.remove", "1", b"x".hex())
        self.assertEqual(self.next(one), (0, 3))
        self.assertEqual(self.next(renewed.resources[0]), (0, 3))
        self.rpc("write", "env/CWD", "resource.bytes", "1", b"modules".hex())
        self.assertEqual(self.next(one), (0, 4))
        # A pending read survives native calls; closing it wakes and fences it.
        request = self.wire("reserve").split("\t")[1]
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as waiting:
            waiting.sendall((f"v1\t{self.client}\tcall\t{request}\tread\t"
                             f"{one}\tenv.next\t1\n").encode())
            reader = self.enterContext(waiting.makefile("r"))
            self.assertEqual(reader.readline(65537), "pending\n")
            self.cli("--contract", "kernel.module.unload", "1",
                     loaded.resources[0], success=False)
            self.close(one)
            self.assertTrue(self.read_final(reader).startswith("error\tfailed\t6\t"))
        self.wire("release", request)
        self.close(renewed.resources[0])
        self.cli("--contract", "kernel.module.unload", "1", loaded.resources[0])

    def test_lost_observation_recovers_one_reader_and_capacity_is_reusable(self):
        library = self.build_module("reovim-module-env", "env")
        payload = (str(library) + "\0").encode().hex()
        loaded = self.rpc("write", "kernel/modules/load", "host.module.load-with",
                          "1", payload, "")
        request = self.wire("reserve").split("\t")[1]
        host, port = self.address.rsplit(":", 1)
        fields = ("read", "module/env/watch", "env.observe", "1")
        frame = "\t".join(("v1", self.client, "call", request, *fields)) + "\n"
        with socket.create_connection((host, int(port)), timeout=5) as stream:
            stream.sendall(frame.encode())
        deadline = time.monotonic() + 5
        while self.wire("status", request) != "ok\tfinished":
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.01)
        self.rpc("write", "env/CWD", "resource.bytes", "1", b"modules".hex())
        first = Reply.parse(self.wire("result", request))
        retried = Reply.parse(self.wire("call", request, *fields))
        self.assertEqual(first, retried)
        self.assertEqual(snapshot(first.data), (0, {b"CWD": b""}))
        self.assertEqual(self.wire("release", request), "ok")
        self.assertEqual(self.next(first.resources[0]), (0, 1))
        readers = [first.resources[0]]
        for _ in range(31):  # env's named per-instance bound is 32 readers.
            readers.append(self.observe().resources[0])
        request = self.wire("reserve").split("\t")[1]
        rejected = self.wire("call", request, *fields)
        self.assertTrue(rejected.startswith("error\tfailed\t5\t"), rejected)
        self.wire("release", request)
        self.close(readers.pop())
        renewed = self.observe()
        self.assertEqual(snapshot(renewed.data), (1, {b"CWD": b"modules"}))
        readers.append(renewed.resources[0])
        peer = self.new_client()
        peer_module = self.rpc("write", "kernel/modules/load", "host.module.load-with",
                               "1", payload, "", client=peer)
        other = self.rpc("read", "module/env/watch", "env.observe", "1", client=peer)
        self.assertEqual(snapshot(other.data), (0, {b"CWD": b""}))
        for reader in readers:
            self.close(reader)
        request = self.wire("reserve", client=peer).split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", other.resources[0],
                                   client=peer), "ok")
        self.wire("release", request, client=peer)
        self.cli("--contract", "kernel.module.unload", "1", loaded.resources[0])
        self.cli("--contract", "kernel.module.unload", "1", peer_module.resources[0],
                 client=peer)

    def test_oversized_snapshot_rejects_without_publishing_a_reader(self):
        library = self.build_module("reovim-module-env", "env")
        loaded = self.rpc("write", "kernel/modules/load", "host.module.load-with",
                          "1", (str(library) + "\0").encode().hex(), "")
        # Value fits a call; its complete snapshot exceeds env's 16 KiB bound.
        self.rpc("write", "env/CWD", "resource.bytes", "1", (b"x" * 16384).hex())
        request = self.wire("reserve").split("\t")[1]
        result = self.wire("call", request, "read", "module/env/watch", "env.observe", "1")
        self.assertTrue(result.startswith("error\tfailed\t5\t"), result)
        self.wire("release", request)
        self.cli("--contract", "kernel.module.unload", "1", loaded.resources[0])
