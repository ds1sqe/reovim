from .fixture import Scenario


def u64(value):
    return value.to_bytes(8, "little")


class NativeSource(Scenario):
    def test_native_source_splice_windows_history_and_retained_lifetime(self):
        library = self.build_module("reovim-module-source", "source")
        module = self.load(library, "native/source")
        original = b"# Draft\r\n\xff\x00tail"
        first = self.rpc("write", "native/source", "source.initial", "1",
                         original.hex())
        buffer, history = first.resources
        self.assertEqual(first.data[16:], u64(len(original)))
        old = self.retain(buffer, 0)
        replacement = "한글 🦀".encode()
        payload = u64(0) + u64(2) + u64(7) + replacement
        changed = self.rpc("write", buffer, "source.splice", "1",
                           payload.hex(), buffer)
        expected = b"# " + replacement + b"\r\n\xff\x00tail"
        self.assertEqual(changed.data[16:], u64(len(expected)))
        current = self.retain(buffer, 1)
        self.assertEqual(self.window(current, 1, len(expected) - 1),
                         expected[1:-1])
        self.assertEqual(self.window(old, 0, len(original)), original)
        self.call("write", buffer, "source.splice", "1", payload.hex(),
                  buffer, success=False)
        invalid = u64(1) + u64(len(expected) + 1) + u64(len(expected) + 1)
        self.call("write", buffer, "source.splice", "1", invalid.hex(),
                  buffer, success=False)
        self.rpc("write", history, "kernel.history.undo", "1", u64(1).hex(), buffer)
        restored = self.retain(buffer, 2)
        self.assertEqual(self.window(restored, 0, len(original)), original)
        self.rpc("write", history, "kernel.history.redo", "1", u64(2).hex(), buffer)
        redone = self.retain(buffer, 3)
        self.assertEqual(self.window(redone, 0, len(expected)), expected)
        self.rpc("write", buffer, "kernel.buffer.close", "1",
                 (u64(3) + b"\x01").hex(), buffer)
        self.unload(module, success=False)
        self.assertEqual(self.window(old, 0, len(original)), original)
        for snapshot in (old, current, restored, redone):
            self.release_snapshot(snapshot)
        self.unload(module)

    def test_document_grows_past_frame_limit_while_reads_remain_windowed(self):
        library = self.build_module("reovim-module-source", "source")
        module = self.load(library, "native/source")
        initial = self.rpc("write", "native/source", "source.initial", "1")
        buffer, _ = initial.resources
        blocks = [bytes([value]) * 20000 for value in (0, 255, 128, 10)]
        size = 0
        for revision, block in enumerate(blocks):
            payload = u64(revision) + u64(size) + u64(size) + block
            changed = self.rpc("write", buffer, "source.splice", "1",
                               payload.hex(), buffer)
            size += len(block)
            self.assertEqual(changed.data[16:], u64(size))
        snapshot = self.retain(buffer, 4)
        expected = b"".join(blocks)
        for start in (0, 19998, 39998, 59998, 79990):
            self.assertEqual(self.window(snapshot, start, start + 10),
                             expected[start:start + 10])
        self.assertEqual(self.window(snapshot, size, size), b"")
        self.release_snapshot(snapshot)
        self.rpc("write", buffer, "kernel.buffer.close", "1",
                 (u64(4) + b"\x01").hex(), buffer)
        self.unload(module)

    def release_snapshot(self, snapshot):
        request = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", snapshot), "ok")
        self.assertEqual(self.wire("release", request), "ok")

    def retain(self, buffer, revision):
        reply = self.rpc("read", buffer, "kernel.buffer.retain", "1",
                         u64(revision).hex())
        self.assertEqual(reply.data[:8], u64(revision))
        return reply.resources[1]

    def window(self, snapshot, start, end):
        reply = self.rpc("read", snapshot, "source.bytes", "1",
                         (u64(start) + u64(end)).hex())
        self.assertEqual(reply.data[:16], u64(start) + u64(end))
        return reply.data[16:]
