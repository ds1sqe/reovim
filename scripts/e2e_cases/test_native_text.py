import socket
import time

from .fixture import Reply, Scenario
from .test_native_source import u64


def scalar(text):
    return [(0, ord(c)) for c in text]


def units(data):
    if len(data) % 5:
        raise AssertionError("incomplete Text unit")
    return [(data[i], int.from_bytes(data[i + 1:i + 5], "little"))
            for i in range(0, len(data), 5)]


class NativeText(Scenario):
    def models(self, original):
        source = self.build_module("reovim-module-source", "source")
        source_module = self.load(source, "native/source")
        text = self.build_module("reovim-module-text", "text")
        text_module = self.load(text, "native/text")
        initial = self.rpc("write", "native/source", "source.initial", "1", original.hex())
        return source_module, text_module, initial.resources

    def attach(self, buffer, writable=True):
        name = "text.attach" if writable else "text.attach.read"
        return self.rpc("write", "native/text", name, "1", "", buffer).resources[0]

    def observe(self, view):
        reply = self.rpc("read", view, "text.observe", "1")
        self.assertEqual(reply.resources[:2], (view, view))
        return reply

    def window(self, observed, start=0, count=1024):
        reply = self.rpc("read", observed.resources[4], "text.window", "1",
                         (u64(start) + u64(count)).hex())
        self.assertEqual(reply.data[1:9], u64(start))
        return units(reply.data[17:]), reply.data[0]

    def replace(self, view, observed, start, end, text):
        return self.rpc("write", view, "text.replace", "1",
                        (u64(start) + u64(end) + text.encode()).hex(), observed.resources[4])

    def close_access(self, resource):
        request = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", resource), "ok")
        self.assertEqual(self.wire("release", request), "ok")

    def test_unicode_opaque_edits_foreign_space_undo_and_detached_snapshots(self):
        original = "# 한글 🦀\r\n".encode() + b"\xff\x00tail"
        source_module, text_module, (buffer, history) = self.models(original)
        view = self.attach(buffer)
        first = self.observe(view)
        expected = scalar("# 한글 🦀\r\n") + [(1, 255)] + scalar("\x00tail")
        self.assertEqual(self.window(first), (expected, 1))
        changed = self.replace(view, first, 2, 4, "é漢")
        self.assertEqual(changed.data[:8], u64(1))
        second = self.observe(view)
        expected_after = scalar("# é漢 🦀\r\n") + [(1, 255)] + scalar("\x00tail")
        self.assertEqual(self.window(second), (expected_after, 1))
        self.assertEqual(self.window(first), (expected, 1))
        stale = u64(0) + u64(0) + b"stale"
        self.call("write", view, "text.replace", "1", stale.hex(), first.resources[4], success=False)
        other = self.attach(buffer)
        foreign = self.observe(other)
        self.call("write", view, "text.replace", "1", stale.hex(), foreign.resources[4], success=False)
        readonly = self.attach(buffer, False)
        passive = self.observe(readonly)
        self.call("write", readonly, "text.replace", "1", stale.hex(), passive.resources[4], success=False)
        self.call("write", view, "text.replace", "1", (u64(0) + u64(0) + b"\xff").hex(), second.resources[4], success=False)
        event = self.rpc("read", first.resources[5], "text.next", "1", "00")
        self.assertEqual(event.data[:2], b"\x00\x00")
        self.assertEqual(event.data[10:26], u64(0) + u64(1))
        self.rpc("write", history, "kernel.history.undo", "1", u64(1).hex(), buffer)
        restored = self.observe(view)
        self.assertEqual(self.window(restored), (expected, 1))
        self.close_access(view)
        ended = self.rpc("read", first.resources[5], "text.next", "1", "00")
        self.assertEqual(ended.data, b"\x02\x01")
        self.assertEqual(self.window(first), (expected, 1))
        self.unload(text_module, success=False)
        self.close_access(other)
        self.close_access(readonly)
        for observed in (first, second, foreign, passive, restored):
            self.close_access(observed.resources[4])
            self.close_access(observed.resources[5])
        self.unload(text_module)
        self.rpc("write", buffer, "kernel.buffer.close", "1", (u64(2) + b"\x01").hex(), buffer)
        self.unload(source_module)

    def test_split_utf8_prefixes_and_incomplete_eof_preserve_each_source_unit(self):
        original = b"a" * 4095 + "🦀é".encode() + b"\xe0\x80\x80\xf0\x9f"
        source_module, text_module, (buffer, _) = self.models(original)
        view = self.attach(buffer)
        observed = self.observe(view)
        expected = scalar("a🦀é") + [(1, b) for b in b"\xe0\x80\x80\xf0\x9f"]
        self.assertEqual(self.window(observed, 4094, 20), (expected, 1))
        changed = self.replace(view, observed, 4098, 4099, "!")
        fresh = self.observe(view)
        self.assertEqual(self.window(fresh, 4095, 20),
                         (scalar("🦀é!") + [(1, b) for b in b"\x80\x80\xf0\x9f"], 1))
        self.close_access(view)
        for item in (observed, fresh):
            self.close_access(item.resources[4])
            self.close_access(item.resources[5])
        self.unload(text_module)
        self.rpc("write", buffer, "kernel.buffer.close", "1", changed.data[:8].hex() + "01", buffer)
        self.unload(source_module)

    def test_slow_text_watch_reports_gap_and_renews_at_current_source(self):
        source_module, text_module, (buffer, _) = self.models(b"end")
        view = self.attach(buffer)
        slow = self.observe(view)
        for revision in range(33):
            current = self.observe(view)
            self.replace(view, current, 0, 0, "x")
            event = self.rpc("read", current.resources[5], "text.next", "1", "00")
            self.assertEqual(event.data[10:26], u64(revision) + u64(revision + 1))
            self.close_access(current.resources[4])
            self.close_access(current.resources[5])
        for _ in range(2):
            self.assertEqual(self.rpc("read", slow.resources[5], "text.next", "1", "00").data, b"\x01")
        renewed = self.observe(view)
        self.assertEqual(renewed.data[:8], u64(33))
        self.assertEqual(self.window(renewed), (scalar("x" * 33 + "end"), 1))
        self.close_access(view)
        for observed in (slow, renewed):
            self.close_access(observed.resources[4])
            self.close_access(observed.resources[5])
        self.unload(text_module)
        self.rpc("write", buffer, "kernel.buffer.close", "1", (u64(33) + b"\x01").hex(), buffer)
        self.unload(source_module)

    def test_lost_text_observation_recovers_its_original_cut_and_owners(self):
        source_module, text_module, (buffer, _) = self.models(b"before")
        view = self.attach(buffer)
        request = self.wire("reserve").split("\t")[1]
        fields = ("read", view, "text.observe", "1")
        frame = "\t".join(("v1", self.client, "call", request, *fields)) + "\n"
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as stream:
            stream.sendall(frame.encode())
        deadline = time.monotonic() + 5
        while self.wire("status", request) != "ok\tfinished":
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.01)
        self.rpc("write", buffer, "source.splice", "1",
                 (u64(0) + u64(0) + u64(0) + b"x").hex(), buffer)
        original = Reply.parse(self.wire("result", request))
        recovered = Reply.parse(self.wire("call", request, *fields))
        self.assertEqual(original, recovered)
        self.assertEqual(self.wire("release", request), "ok")
        self.assertEqual(original.data[:8], u64(0))
        self.assertEqual(self.window(original), (scalar("before"), 1))
        event = self.rpc("read", original.resources[5], "text.next", "1", "00")
        self.assertEqual(event.data[10:26], u64(0) + u64(1))
        self.close_access(view)
        self.close_access(original.resources[4])
        self.close_access(original.resources[5])
        self.unload(text_module)
        self.rpc("write", buffer, "kernel.buffer.close", "1", (u64(1) + b"\x01").hex(), buffer)
        self.unload(source_module)
