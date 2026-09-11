from .fixture import Scenario


class Editing(Scenario):
    def test_source_fidelity_stale_rejection_undo_and_close(self):
        exit_command = self.load_command("exit", "exit")
        module = self.call("write", "catalog/editing",
                           "kernel.module.activate", "1").resources[0]
        source = b"# original\n\xffopaque"
        initial = self.call("write", "editing/heading", "heading.initial",
                            "1", source.hex())
        buffer, history = initial.resources
        self.assertEqual(initial.data[16:], source)
        revision = (0).to_bytes(8, "little").hex()
        self.cli(input=(
            f"--contract heading.replace 1 --hex {revision}{b'changed'.hex()} "
            f"--refs {buffer} {buffer}\n/exit\n"))
        read = self.call("read", buffer, "heading.source", "1")
        self.assertEqual(read.data[16:], b"# changed\n\xffopaque")
        self.call("write", buffer, "heading.replace", "1",
                  revision + b"stale".hex(), buffer, success=False)
        self.unload(module, success=False)
        undone = self.call("write", history, "kernel.history.undo", "1",
                           (1).to_bytes(8, "little").hex(), buffer)
        self.assertEqual(int.from_bytes(undone.data[:8], "little"), 2)
        read = self.call("read", buffer, "heading.source", "1")
        self.assertEqual(read.data[16:], source)
        close = ((2).to_bytes(8, "little") + b"\x01").hex()
        self.call("write", buffer, "kernel.buffer.close", "1", close, buffer)
        self.unload(module)
        self.call("read", buffer, "heading.source", "1", success=False)
        self.unload(exit_command)
