"""Startup recovery must never turn missing evidence into another Client."""

import secrets
import socket
import time

from .fixture import Scenario


class Opening(Scenario):
    def boot_arguments(self):
        return []

    def handshake(self, action, key, *, discard=False):
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as stream:
            stream.sendall(f"v1\t{action}\t{key}\n".encode())
            if discard:
                return None
            with stream.makefile("r") as reader:
                return self.read_final(reader).rstrip("\n")

    def test_lost_open_recovers_same_client_and_retired_evidence_stays_missing(self):
        key = secrets.token_hex(16)
        self.handshake("open", key, discard=True)
        deadline = time.monotonic() + 5
        while True:
            reply = self.handshake("recover", key)
            if reply.startswith("ok\t"):
                break
            self.assertIn("opening evidence unavailable", reply)
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.01)
        self.assertEqual(self.handshake("recover", key), reply)
        self.assertEqual(self.handshake("open", key), reply)
        _, client, receipt = reply.split("\t")
        self.assertEqual(self.wire("input", receipt, client=client),
                         "ok\t" + b"boot".hex())
        self.assertEqual(self.wire("release", receipt, client=client), "ok")
        self.assertIn("unavailable request", self.handshake("recover", key))
        self.disconnect(client)
        self.assertIn("opening evidence unavailable",
                      self.handshake("recover", key))

    def test_unknown_recovery_does_not_allocate_client_capacity(self):
        for _ in range(20):
            self.assertIn("opening evidence unavailable",
                          self.handshake("recover", secrets.token_hex(16)))
        client = self.new_client()
        self.assertEqual(self.wire("requests", client=client), "ok\t")
        self.disconnect(client)
