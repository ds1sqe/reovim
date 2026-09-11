import socket

from .fixture import Reply, Scenario


class ClientTeardown(Scenario):
    def test_disconnect_settles_watch_retains_receipts_and_preserves_peer(self):
        library = self.build_module("reovim-module-env", "env")
        peer = self.new_client()
        payload = (str(library) + "\0").encode().hex()
        for client in (self.client, peer):
            self.rpc("write", "kernel/modules/load", "host.module.load-with",
                     "1", payload, "", client=client)
        self.rpc("write", "env/CWD", "resource.bytes", "1", b"models".hex(), client=peer)
        observed = self.rpc("read", "module/env/watch", "env.observe", "1")
        request = self.wire("reserve").split("\t")[1]
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as waiting:
            waiting.sendall((f"v1\t{self.client}\tcall\t{request}\tread\t"
                             f"{observed.resources[0]}\tenv.next\t1\n").encode())
            reader = self.enterContext(waiting.makefile("r"))
            self.assertEqual(reader.readline(65537), "pending\n")
            self.cli("--disconnect", success=False)
            result = self.read_final(reader).rstrip("\n")
            self.assertTrue(result.startswith("error\tfailed\t6\t"), result)
        self.assertEqual(self.wire("result", request), result)
        self.assertEqual(self.wire("status", request), "ok\tfinished")
        # Teardown cannot forget an unadopted creation result either.
        created = self.wire("reserve").split("\t")[1]
        original = self.wire("call", created, "write", "module/env/create",
                             "env.create", "1", b"retained\0value".hex())
        self.assertTrue(self.wire("disconnect").startswith("error\tclient busy;"))
        self.assertEqual(self.wire("result", created), original)
        key = Reply.parse(original).resources[0]
        self.assertEqual(self.rpc("read", key, "resource.bytes", "1").data, b"value")
        for receipt in (request, created):
            self.wire("release", receipt)
        # Retire snapshots of the shared Session owned by the departing client.
        shared = self.rpc("read", "session", "heading.source", "1")
        self.assertEqual(shared.data, b"# Shared session\n")
        self.disconnect(self.client)
        self.assertEqual(self.cli("--disconnect"), "ok\n")
        self.assertTrue(self.wire("reserve").startswith("error\tunavailable client"))
        self.assertEqual(self.rpc("read", "env/CWD", "resource.bytes", "1", client=peer).data,
                         b"models")
        still_shared = self.rpc("read", "session", "heading.source", "1", client=peer)
        self.assertEqual(still_shared.data, shared.data)
        self.disconnect(peer)
        # Closed clients release live-client capacity; a fresh owner is usable.
        fresh = self.new_client()
        self.assertNotIn(fresh, (self.client, peer))
        current = self.rpc("read", "session", "heading.source", "1", client=fresh)
        self.assertEqual(current.data, shared.data)
        self.disconnect(fresh)

    def test_disconnect_requires_explicit_private_buffer_close(self):
        created = self.rpc("write", "models/session", "heading.initial", "1",
                           b"# Keep my edits\n".hex())
        result = self.wire("disconnect")
        self.assertIn("client cleanup: Resource error 8", result)
        observed = self.rpc("read", created.resources[0], created.contract, "1")
        self.assertEqual(observed.data, created.data)
        self.assertEqual(observed.resources, created.resources)
        close = created.data[:8] + b"\x01"
        self.rpc("write", created.resources[0], "kernel.buffer.close", "1",
                 close.hex(), created.resources[0])
        self.disconnect(self.client)
