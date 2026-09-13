import tempfile
from pathlib import Path

from .fixture import ROOT, Reply, Scenario
from .test_native_source import u64


class FileSelection(Scenario):
    def boot_arguments(self):
        directory = self.enterContext(tempfile.TemporaryDirectory(
            prefix="file-selection-", dir=ROOT / "tmp"))
        self.root = Path(directory)
        self.path = self.root / "draft"
        self.path.write_bytes(b"before\xff\r\n")
        (self.root / "alias").hardlink_to(self.path)
        scratch = self.enterContext(tempfile.TemporaryDirectory(
            prefix="file-selection-", dir="/dev/shm"))
        return [*super().boot_arguments(), "--files", "files", directory,
                directory, "--files", "unsupported", directory, scratch]

    def select(self, path):
        return self.call("write", "modules/files", "host.files.select", "1",
                         path.encode().hex())

    def release(self, selected, success=True):
        self.call("write", "modules/files", "host.files.release", "1",
                  selected.data.hex(), success=success)

    def open(self, selected):
        reply = self.call("write", "native/source", "kernel.buffer.open", "1",
                          "", selected.resources[0])
        self.assertEqual(reply.data, u64(0))
        self.assertEqual(reply.resources[2], selected.resources[0])
        return reply.resources[:2]

    def retain(self, buffer, revision=0):
        return self.call("read", buffer, "kernel.buffer.retain", "1",
                         u64(revision).hex()).resources[1]

    def read(self, snapshot):
        value = self.call("read", snapshot, "source.bytes", "1",
                          (u64(0) + u64(9)).hex())
        self.assertEqual(value.data[:16], u64(0) + u64(9))
        return value.data[16:]

    def close(self, resource):
        receipt = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", receipt, "close", resource), "ok")
        self.assertEqual(self.wire("release", receipt), "ok")

    def close_buffer(self, buffer):
        self.call("write", buffer, "kernel.buffer.close", "1",
                  (u64(0) + b"\0").hex(), buffer)

    def test_shared_selection_backed_open_and_explicit_idle_release(self):
        model = self.load(
            self.build_module("reovim-module-source", "source_open"),
            "native/source", imports="")
        selected = self.select("draft")
        repeated = self.select("alias")
        self.assertEqual(selected, repeated)
        first, first_history = self.open(selected)
        old = self.retain(first)
        self.path.write_bytes(b"after!\xfe\r\n")
        changed = self.select("draft")
        self.assertEqual(changed, selected)
        second, second_history = self.open(changed)
        current = self.retain(second)
        self.assertNotEqual(first, second)
        self.assertNotEqual(first_history, second_history)
        self.assertEqual(self.read(old), b"before\xff\r\n")
        self.assertEqual(self.read(current), b"after!\xfe\r\n")
        self.release(selected, success=False)
        self.close_buffer(first)
        self.release(selected, success=False)
        self.unload("modules/host/files-0", success=False)
        self.close_buffer(second)
        self.release(selected)
        self.assertEqual(self.read(old), b"before\xff\r\n")
        self.assertEqual(self.read(current), b"after!\xfe\r\n")
        renewed = self.select("draft")
        self.assertNotEqual(renewed.resources, selected.resources)
        self.release(selected, success=False)
        self.release(renewed)
        self.unload("modules/host/files-0")
        self.assertEqual(self.read(current), b"after!\xfe\r\n")
        self.close(old)
        self.close(current)
        self.unload(model)

    def test_path_replacement_failure_and_request_replay(self):
        request = self.wire("reserve").split("\t")[1]
        fields = ("call", request, "write", "modules/files",
                  "host.files.select", "1", b"draft".hex())
        original = self.wire(*fields)
        self.path.rename(self.root / "moved")
        self.path.write_bytes(b"new file!")
        self.assertEqual(self.wire(*fields), original)
        self.assertEqual(self.wire("release", request), "ok")
        old = Reply.parse(original)
        self.assertEqual(self.select("moved"), old)
        replaced = self.select("draft")
        self.assertNotEqual(replaced.resources, old.resources)
        for path in ("missing", "../draft", "/draft", "", "draft\0ignored"):
            self.call("write", "modules/files", "host.files.select", "1",
                      path.encode().hex(), success=False)
        self.call("write", "modules/unsupported", "host.files.select", "1",
                  b"draft".hex(), success=False)
        self.call("write", "modules/unsupported", "host.files.release", "1",
                  old.data.hex(), success=False)
        self.assertEqual(self.select("draft"), replaced)
        self.release(old)
        self.release(replaced)
        self.unload("modules/host/files-0")
        self.unload("modules/host/files-1")
