from .fixture import Scenario


class NativeHeading(Scenario):
    def test_safe_model_declaration_preserves_source_undo_and_code_lifetime(self):
        library = self.build_module("reovim-module-editing", "heading")
        module = self.load(library, "native/heading")
        source = b"# original\n\xffpreserved"
        initial = self.rpc("write", "native/heading", "heading.initial", "1", source.hex())
        buffer, history = initial.resources
        self.assertEqual(initial.data[16:], source)
        edit = (0).to_bytes(8, "little") + b"replacement"
        changed = self.rpc("write", buffer, "heading.replace", "1", edit.hex(), buffer)
        self.assertEqual(changed.data[16:], b"# replacement\n\xffpreserved")
        self.unload(module, success=False)
        restored = self.rpc("write", history, "kernel.history.undo", "1",
                            (1).to_bytes(8, "little").hex(), buffer)
        observed = self.rpc("read", buffer, "heading.source", "1")
        self.assertEqual(observed.data[16:], source)
        self.assertEqual(observed.data[:8], restored.data[:8])
        self.rpc("write", buffer, "kernel.buffer.close", "1",
                 ((2).to_bytes(8, "little") + b"\x01").hex(), buffer)
        self.unload(module)
