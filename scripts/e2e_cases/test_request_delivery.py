import socket
import subprocess
import time

from .fixture import Reply, Scenario


class RequestDelivery(Scenario):
    def reserve(self):
        reply = self.wire("reserve")
        self.assertTrue(reply.startswith("ok\t"), reply)
        return reply.split("\t")[1]

    def send_without_reading(self, request, *fields):
        host, port = self.address.rsplit(":", 1)
        frame = "\t".join(("v1", self.client, "call", request, *fields)) + "\n"
        with socket.create_connection((host, int(port)), timeout=5) as stream:
            stream.sendall(frame.encode())
        self.transcript.write(f"LOST REPLY {frame!r}\n")
        self.transcript.flush()

    def finished(self, request):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if self.wire("status", request) == "ok\tfinished":
                return
            time.sleep(0.01)
        self.fail("original request did not finish")

    def test_released_reservations_never_admit_late_calls(self):
        counter = self.rpc("write", "modules/catalog/counter",
                           "kernel.module.activate", "1")
        request = self.reserve()
        self.assertEqual(self.wire("status", request), "ok\treserved")
        self.assertEqual(self.wire("release", request), "ok")
        late = self.wire("call", request, "write", "modules/counter/value",
                         "external.counter", "1", (7).to_bytes(8, "little").hex())
        self.assertTrue(late.startswith("error\t"), late)
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "modules/counter/value", "external.counter", "1").data,
            "little"), 0)
        self.cli("--contract", "kernel.module.unload", "1", counter.resources[0])

    def test_lost_creation_reply_keeps_the_original_instance(self):
        library = self.build_c("modules/native-counter/counter.c", "counter")
        payload = (str(library) + "\0state").encode().hex()
        request = self.reserve()
        fields = ("write", "kernel/modules/load", "host.module.load", "1", payload)
        self.send_without_reading(request, *fields)
        self.finished(request)
        self.assertIn(f"{request}=finished", self.cli("--requests"))
        info = self.cli("--request-info", request).strip().split("\t")[1]
        self.assertEqual(bytes.fromhex(info).decode(), "\t".join(fields))
        first = Reply.parse(self.wire("result", request))
        second = Reply.parse(self.wire("call", request, *fields))
        self.assertEqual(first, second)
        self.assertEqual(first.data, b"ready")
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "state", "native.counter", "1").data, "little"), 0)
        changed = self.wire("call", request, "write", "kernel/modules/load",
                            "host.module.load", "1", payload + "00")
        self.assertTrue(changed.startswith("error\t"), changed)
        self.assertEqual(Reply.parse(self.wire("result", request)), first)
        self.assertEqual(self.wire("release", request), "ok")
        self.assertTrue(self.wire("call", request, *fields).startswith("error\t"))
        self.cli("--contract", "kernel.module.unload", "1", first.resources[0])

    def test_additive_effect_is_not_repeated_and_buffer_references_are_stable(self):
        counter = self.rpc("write", "modules/catalog/counter",
                           "kernel.module.activate", "1")
        request = self.reserve()
        fields = ("write", "modules/counter/value", "external.counter", "1",
                  (7).to_bytes(8, "little").hex())
        self.send_without_reading(request, *fields)
        self.finished(request)
        first = Reply.parse(self.wire("result", request))
        self.assertEqual(Reply.parse(self.wire("call", request, *fields)), first)
        self.assertEqual(int.from_bytes(first.data, "little"), 7)
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "modules/counter/value", "external.counter", "1").data,
            "little"), 7)
        self.wire("release", request)
        editing = self.rpc("write", "modules/catalog/editing",
                           "kernel.module.activate", "1")
        request = self.reserve()
        source = b"# Retained\nunchanged source\n"
        self.send_without_reading(request, "write", "models/heading",
                                  "heading.initial", "1", source.hex())
        self.finished(request)
        first = Reply.parse(self.wire("result", request))
        second = Reply.parse(self.wire("result", request))
        self.assertEqual(first, second)
        self.assertEqual(first.data[16:], source)
        self.assertEqual(self.wire("release", request), "ok")
        observed = self.rpc("read", first.resources[0], first.contract, "1")
        self.assertEqual(observed.resources, first.resources)
        self.assertEqual(observed.data, first.data)
        revision = first.data[:8] + b"\x01"
        self.rpc("write", first.resources[0], "kernel.buffer.close", "1",
                 revision.hex(), first.resources[0])
        for instance in (counter.resources[0], editing.resources[0]):
            self.cli("--contract", "kernel.module.unload", "1", instance)

    def test_a_waiting_module_and_stalled_socket_do_not_block_other_clients(self):
        library = self.build_module("reovim-module-example", "example")
        loaded = self.rpc("write", "kernel/modules/load", "host.module.load",
                          "1", (str(library) + "\0example").encode().hex())
        counter = self.rpc("write", "modules/catalog/counter",
                           "kernel.module.activate", "1")
        second = self.new_client()
        request = self.reserve()
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as waiting:
            with socket.create_connection((host, int(port)), timeout=5) as stalled:
                stalled.sendall(b"v1\t")
                frame = (f"v1\t{self.client}\tcall\t{request}\twrite\t"
                         "example/wait\texample.wait\t1\n")
                waiting.sendall(frame.encode())
                waiting.shutdown(socket.SHUT_WR)
                reader = self.enterContext(waiting.makefile("r"))
                self.assertEqual(reader.readline(65537), "pending\n")
                deadline = time.monotonic() + 1
                while self.wire("status", request, timeout=1) != "ok\tpending":
                    self.assertLess(time.monotonic(), deadline)
                changed = Reply.parse(self.cli(
                    "--contract", "external.counter", "1", "--hex",
                    (5).to_bytes(8, "little").hex(), "/modules/counter/value",
                    client=second))
                self.assertEqual(int.from_bytes(changed.data, "little"), 5)
                self.assertEqual(self.wire("status", request), "ok\tpending")
                self.assertTrue(self.wire("release", request).startswith("error\t"))
                blocked = self.reserve()
                failed = self.wire("call", blocked, "write", loaded.resources[0],
                                   "kernel.module.unload", "1")
                self.assertTrue(failed.startswith("error\tfailed\t8\t"), failed)
                self.wire("release", blocked)
                self.rpc("write", "example/signal", "example.signal", "1")
                reply = Reply.parse(self.read_final(reader))
                self.assertEqual(reply.data, b"released")
        self.assertEqual(Reply.parse(self.wire("result", request)), reply)
        self.wire("release", request)
        for instance in (loaded.resources[0], counter.resources[0]):
            self.cli("--contract", "kernel.module.unload", "1", instance)

    def test_cli_keeps_waiting_through_pending_response_frames(self):
        library = self.build_module("reovim-module-example", "example")
        loaded = self.rpc("write", "kernel/modules/load", "host.module.load",
                          "1", (str(library) + "\0example").encode().hex())
        output_path = self.evidence / "waiting-cli.stdout"
        errors_path = self.evidence / "waiting-cli.stderr"
        output = self.enterContext(output_path.open("w"))
        errors = self.enterContext(errors_path.open("w"))
        process = subprocess.Popen([
            str(self.binaries / "reovim-cli"), self.address, "--client", self.client,
            "--contract", "example.wait", "1", "/example/wait",
        ], stdout=output, stderr=errors)
        self.addCleanup(self.stop_process, process)
        deadline = time.monotonic() + 5
        pending = False
        while time.monotonic() < deadline:
            entries = self.wire("requests").split("\t")[1:]
            pending = any(entry.endswith("=pending") for entry in entries)
            if pending:
                break
            time.sleep(0.01)
        self.assertTrue(pending, "CLI did not submit its waiting request")
        time.sleep(0.7)  # Cross the transport's pending-frame interval.
        self.assertIsNone(process.poll(), errors_path.read_text())
        self.rpc("write", "example/signal", "example.signal", "1")
        self.assertEqual(process.wait(timeout=5), 0, errors_path.read_text())
        self.assertEqual(errors_path.read_text(), "")
        self.assertEqual(Reply.parse(output_path.read_text()).data, b"released")
        self.cli("--contract", "kernel.module.unload", "1", loaded.resources[0])

    def test_lost_env_creation_returns_the_same_key_without_recreation(self):
        library = self.build_module("reovim-module-env", "env")
        loaded = self.rpc("write", "kernel/modules/load", "host.module.load-with",
                          "1", (str(library) + "\0").encode().hex(), "")
        request = self.reserve()
        value = b"\0\nbinary"
        fields = ("write", "module/env/create", "env.create", "1",
                  (b"LOST\0" + value).hex())
        self.send_without_reading(request, *fields)
        self.finished(request)
        first = Reply.parse(self.wire("result", request))
        second = Reply.parse(self.wire("call", request, *fields))
        self.assertEqual(first, second)
        self.assertEqual(self.rpc("read", first.resources[0], "resource.bytes", "1").data,
                         value)
        self.wire("release", request)
        self.assertEqual(self.rpc("read", "env/LOST", "resource.bytes", "1").data,
                         value)
        self.cli("/module/env/remove", "LOST")
        self.cli("--contract", "kernel.module.unload", "1", loaded.resources[0])
