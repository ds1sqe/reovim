from .fixture import Scenario
from .test_native_source import u64


class BufferWatch(Scenario):
    def test_native_source_observation_tracks_edits_undo_redo_and_end(self):
        library = self.build_module("reovim-module-source", "source")
        module = self.load(library, "native/source")
        original = b"Draft\r\n\xff"
        initial = self.rpc("write", "native/source", "source.initial", "1",
                           original.hex())
        buffer, history = initial.resources
        observed = self.rpc("read", buffer, "kernel.buffer.observe", "1")
        self.assertEqual(observed.data, u64(0) + u64(0))
        self.assertEqual(observed.resources[:2], (buffer, history))
        snapshot, watch = observed.resources[2:]
        self.assertEqual(self.next(watch), b"\x03")
        payload = u64(0) + u64(0) + u64(5) + b"Plan"
        self.rpc("write", buffer, "source.splice", "1", payload.hex(), buffer)
        edited = self.edge(watch, 0, 0, 1, 0, 1)
        window = self.rpc("read", snapshot, "source.bytes", "1",
                          (u64(0) + u64(len(original))).hex())
        self.assertEqual(window.data[16:], original)
        self.rpc("write", history, "kernel.history.undo", "1", u64(1).hex(), buffer)
        undone = self.edge(watch, 1, 1, 2, 1, 0)
        self.assertNotEqual(edited, undone)
        self.rpc("write", history, "kernel.history.redo", "1", u64(2).hex(), buffer)
        self.edge(watch, 2, 2, 3, 0, 1)
        self.rpc("write", buffer, "kernel.buffer.close", "1",
                 (u64(3) + b"\x01").hex(), buffer)
        self.assertEqual(self.next(watch), b"\x02\x00")
        self.unload(module, success=False)
        self.close_access(snapshot)
        self.unload(module)  # Watch metadata does not retain model code.
        self.assertEqual(self.next(watch), b"\x02\x00")
        self.close_access(watch)

    def next(self, watch):
        return self.rpc("read", watch, "kernel.buffer.next", "1", "00").data

    def edge(self, watch, kind, before, after, source, target):
        event = self.next(watch)
        self.assertEqual(event[:2], bytes([0, kind]))
        self.assertEqual(event[10:], u64(before) + u64(after) + u64(source) + u64(target))
        return event[2:10]

    def close_access(self, access):
        request = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", access), "ok")
        self.assertEqual(self.wire("release", request), "ok")
