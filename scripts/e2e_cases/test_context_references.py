from .fixture import Scenario
from .test_native_source import u64


class ContextReferences(Scenario):
    def test_c_reference_views_are_copied_and_subjects_survive_model_unload(self):
        model = self.load(self.build_c("apps/server/tests/native/references.c", "references"),
                          "native/references", imports="")
        source = self.load(self.build_module("reovim-module-source", "source"),
                           "native/source")
        buffer = self.call("write", "native/source", "source.initial", "1",
                           b"retained".hex()).resources[0]
        name = b"native.references.initial"
        create = (1).to_bytes(4, "little") + len(name).to_bytes(4, "little") + name + b"\x11"
        old = self.call("write", "native/references", "kernel.session.create", "1",
                        create.hex(), buffer)
        self.assertEqual(old.resources[2:], (buffer,))
        changed = self.call("write", old.resources[0], "native.references.update", "1",
                            "23", ",".join((*old.resources[:2], buffer)))
        self.assertEqual(changed.resources[2:], (buffer,))
        self.assertEqual(changed.data, b"\x23")
        self.call("write", old.resources[0], "kernel.session.close", "1")
        self.unload(model)
        retained = self.call("read", old.resources[1], "native.references", "1")
        self.assertEqual(retained.data, b"\x11")
        self.assertEqual(retained.resources, (buffer,))
        self.call("write", buffer, "kernel.buffer.close", "1",
                  (u64(0) + b"\x01").hex(), buffer)
        self.unload(source, success=False)
        for observed in (old, changed):
            self.call("write", observed.resources[1], "kernel.session.release", "1")
        self.unload(source)
