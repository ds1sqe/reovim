import os
import tempfile
from pathlib import Path

from .fixture import ROOT, Reply, Scenario
from .test_native_source import u64


class FileCapture(Scenario):
    def boot_arguments(self):
        self.directory = Path(self.enterContext(tempfile.TemporaryDirectory(
            prefix="file-capture-", dir=ROOT / "tmp")))
        self.root = self.directory / "files"
        self.root.mkdir()
        self.original = b"# before\n\xff"
        (self.root / "draft").write_bytes(self.original)
        (self.directory / "outside").write_bytes(b"outside grant")
        (self.root / "link").symlink_to(self.directory / "outside")
        (self.root / "escape").symlink_to(self.directory, target_is_directory=True)
        os.mkfifo(self.root / "fifo")
        scratch = self.enterContext(tempfile.TemporaryDirectory(
            prefix="file-capture-", dir="/dev/shm"))
        return [*super().boot_arguments(),
                "--files", "files", str(self.root), str(self.directory),
                "--files", "unsupported", str(self.root), scratch]

    def close_access(self, resource):
        request = self.wire("reserve").split("\t")[1]
        self.assertEqual(self.wire("call", request, "close", resource), "ok")
        self.assertEqual(self.wire("release", request), "ok")

    def capture(self, path):
        return self.call("write", "modules/files", "host.files.capture", "1",
                         path.encode().hex()).resources[0]

    def window(self, resource, size):
        return self.call("read", resource, "resource.snapshot", "1",
                         (u64(0) + u64(size)).hex()).data

    def test_fresh_capture_scoped_paths_owned_cleanup_and_request_replay(self):
        for path in ("../outside", "/outside", "link", "escape/outside",
                     "fifo", "draft/child", "", "draft\0ignored"):
            self.call("write", "modules/files", "host.files.capture", "1",
                      path.encode().hex(), success=False)
        failed = self.wire("reserve").split("\t")[1]
        result = self.wire("call", failed, "write", "modules/unsupported",
                           "host.files.capture", "1", b"draft".hex())
        self.assertTrue(result.startswith("error\tfailed\t4\t"), result)
        self.assertEqual(self.wire("release", failed), "ok")
        request = self.wire("reserve").split("\t")[1]
        fields = ("call", request, "write", "modules/files",
                  "host.files.capture", "1", b"draft".hex())
        first = self.wire(*fields)
        self.assertEqual(self.wire(*fields), first)
        self.assertEqual(self.wire("release", request), "ok")
        old = Reply.parse(first).resources[0]
        self.assertEqual(self.window(old, len(self.original)), self.original)
        self.unload("modules/host/files-0", success=False)
        current = b"# changed\n"
        (self.root / "draft").write_bytes(current)
        new = self.capture("draft")
        self.assertNotEqual(old, new)
        self.assertEqual(self.window(new, len(current)), current)
        self.assertEqual(self.window(old, len(self.original)), self.original)
        module = self.load(self.build_module("reovim-module-source", "source_open"),
                           "native/source", imports="")
        opened = self.call("write", "native/source", "source.open", "1", "", old)
        buffer, _ = opened.resources
        held = self.call("read", buffer, "kernel.buffer.retain", "1",
                         u64(0).hex()).resources[1]
        self.call("write", buffer, "kernel.buffer.close", "1",
                  (u64(0) + b"\x01").hex(), buffer)
        value = self.call("read", held, "source.bytes", "1",
                          (u64(0) + u64(len(self.original))).hex())
        self.assertEqual(value.data[16:], self.original)
        self.close_access(held)
        self.unload(module)
        self.close_access(old)
        self.close_access(new)
        self.call("read", old, "resource.snapshot", "1", success=False)
        self.call("read", new, "resource.snapshot", "1", success=False)
        self.unload("modules/host/files-0")
