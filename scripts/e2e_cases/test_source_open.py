import json
import tempfile
from pathlib import Path

from .fixture import ROOT, Scenario
from .test_native_source import u64


class SourceOpen(Scenario):
    def boot_arguments(self):
        directory = self.enterContext(tempfile.TemporaryDirectory(
            prefix="source-open-", dir=ROOT / "tmp"))
        self.source = Path(directory) / "source"
        self.length = 1024 * 1024 * 1024
        with self.source.open("wb") as file:
            file.write(b"head")
            file.seek(self.length - 4)
            file.write(b"\xffend")
        return [*super().boot_arguments(), "--snapshot", "snapshots/input",
                str(self.source), directory]

    def memory(self, stage):
        pid = self.server.pid
        for name in ("status", "io"):
            data = Path(f"/proc/{pid}/{name}").read_text()
            (self.evidence / f"{stage}-{name}.txt").write_text(data)

    def read(self, snapshot, start, end):
        reply = self.call("read", snapshot, "source.bytes", "1",
                          (u64(start) + u64(end)).hex())
        self.assertEqual(reply.data[:16], u64(start) + u64(end))
        return reply.data[16:]

    def retain(self, buffer, revision):
        return self.call("read", buffer, "kernel.buffer.retain", "1",
                         u64(revision).hex()).resources[1]

    def close_access(self, resource):
        request = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", resource), "ok")
        self.assertEqual(self.wire("release", request), "ok")

    def text_prefix(self, buffer):
        library = self.build_module("reovim-module-text", "text")
        module = self.load(library, "native/text")
        view = self.rpc("write", "native/text", "text.attach", "1", "", buffer).resources[0]
        observed = self.rpc("read", view, "text.observe", "1")
        window = self.rpc("read", observed.resources[4], "text.window", "1",
                          (u64(0) + u64(4)).hex())
        expected = b"".join(b"\0" + value.to_bytes(4, "little") for value in b"head")
        self.assertEqual(window.data[17:], expected)
        for access in (view, observed.resources[4], observed.resources[5]):
            self.close_access(access)
        self.unload(module)

    def test_sparse_file_open_edit_undo_and_immutable_external_change(self):
        library = self.build_module("reovim-module-source", "source_open")
        module = self.load(library, "native/source", imports="")
        self.memory("before-open")
        opened = self.call("write", "native/source", "source.open", "1", "",
                           "modules/snapshots/input")
        buffer, history = opened.resources
        metadata = self.call("read", buffer, "kernel.buffer.describe", "1")
        self.assertEqual(metadata.resources, (buffer, history))
        self.assertEqual(metadata.data, u64(0))
        self.assertEqual(opened.data[16:], u64(self.length))
        self.call("read", "modules/snapshots/input", "resource.snapshot", "1",
                  (u64(0) + u64(32769)).hex(), success=False)
        self.call("read", "modules/snapshots/input", "resource.snapshot", "1",
                  (u64(self.length) + u64(self.length + 1)).hex(), success=False)
        self.call("write", "modules/snapshots/input", "resource.snapshot", "1",
                  "00", success=False)
        self.memory("after-open")
        old = self.retain(buffer, 0)
        self.source.write_bytes(b"externally replaced and truncated")
        self.assertEqual(self.read(old, 0, 8), b"head\0\0\0\0")
        self.assertEqual(self.read(old, self.length - 4, self.length), b"\xffend")
        self.text_prefix(buffer)
        splice = u64(0) + u64(self.length - 3) + u64(self.length) + b"NEW"
        changed = self.call("write", buffer, "source.splice", "1",
                            splice.hex(), buffer)
        self.assertEqual(changed.data[:8], u64(1))
        current = self.retain(buffer, 1)
        self.assertEqual(self.read(current, self.length - 4, self.length), b"\xffNEW")
        self.assertEqual(self.read(old, self.length - 4, self.length), b"\xffend")
        self.call("write", history, "kernel.history.undo", "1", u64(1).hex(), buffer)
        restored = self.retain(buffer, 2)
        self.assertEqual(self.read(restored, self.length - 4, self.length), b"\xffend")
        self.call("write", buffer, "kernel.buffer.close", "1",
                  (u64(2) + b"\x01").hex(), buffer)
        self.assertEqual(self.read(old, self.length - 4, self.length), b"\xffend")
        self.unload(module, success=False)
        for snapshot in (old, current, restored):
            self.close_access(snapshot)
        self.unload(module)
        self.memory("after-close")
        (self.evidence / "workload.json").write_text(json.dumps({
            "logical_bytes": self.length,
            "initial_extents": [[0, "head"], [self.length - 4, "ff656e64"]],
            "storage_profile": "Linux atomic clone; immutable 32KiB windows",
        }, indent=2))
