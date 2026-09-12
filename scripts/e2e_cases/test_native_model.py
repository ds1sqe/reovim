from .fixture import Scenario


def keys(reply):
    data = reply.data[16:]
    return {int.from_bytes(data[i:i + 8], "little"): data[i + 8]
            for i in range(0, len(data), 9)}


class NativeModel(Scenario):
    def test_semantic_edit_undo_and_retained_code(self):
        library = self.build_c("modules/native-keys/keys.c", "keys")
        module = self.load(library, "native/keys")
        initial = self.call("write", "native/keys",
                            "native.keys.create", "1")
        buffer, history = initial.resources
        metadata = self.call("read", buffer, "kernel.buffer.describe", "1")
        self.assertEqual(metadata.resources, (buffer, history))
        self.assertEqual(metadata.data, (0).to_bytes(8, "little"))
        self.assertEqual(keys(initial), {17: 1, 9001: 0})
        edit = ((0).to_bytes(8, "little") + (9001).to_bytes(8, "little")).hex()
        changed = self.call("write", buffer, "native.keys.toggle", "1",
                            edit, buffer)
        self.assertEqual(keys(changed), {17: 1, 9001: 1})
        metadata = self.call("read", buffer, "kernel.buffer.describe", "1")
        self.assertEqual(metadata.resources, (buffer, history))
        self.assertEqual(metadata.data, (1).to_bytes(8, "little"))
        self.unload(module, success=False)
        self.call("write", history, "kernel.history.undo", "1",
                  (1).to_bytes(8, "little").hex(), buffer)
        restored = self.call("read", buffer, "native.keys", "1")
        self.assertEqual(keys(restored), {17: 1, 9001: 0})
        close = ((2).to_bytes(8, "little") + b"\x01").hex()
        self.call("write", buffer, "kernel.buffer.close", "1", close, buffer)
        self.unload(module)
