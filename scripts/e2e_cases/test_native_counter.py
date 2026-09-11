from .fixture import Scenario


class NativeCounter(Scenario):
    def test_live_loading_duplicate_rejection_and_replacement(self):
        library = self.build_c("modules/native-counter/counter.c", "counter")
        path = "native/value"
        module = self.load(library, path)
        changed = self.call("write", path, "native.counter", "1", "07")
        self.assertEqual(int.from_bytes(changed.data, "little"), 7)
        duplicate = self.call("write", "kernel/modules/load", "host.module.load",
                              "1", (str(library) + "\0" + path).encode().hex())
        self.assertEqual(duplicate.data, b"readiness-error\t3")
        self.assertNotEqual(duplicate.resources[0], module)
        self.unload(duplicate.resources[0])
        value = self.call("read", path, "native.counter", "1")
        self.assertEqual(int.from_bytes(value.data, "little"), 7)
        self.unload(module)
        self.call("read", path, "native.counter", "1", success=False)
        replacement = self.load(library, path)
        self.assertNotEqual(replacement, module)
        value = self.call("read", path, "native.counter", "1")
        self.assertEqual(int.from_bytes(value.data, "little"), 0)
        self.unload(replacement)
