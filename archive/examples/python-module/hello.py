"""
Hello World Python Module for reovim.

This is the simplest possible Python module demonstrating:
- Module identity (id, name, version)
- Lifecycle (init, exit)

To install:
    cp hello.py ~/.local/share/reovim/modules/
"""

from reovim import Module, ModuleId, ProbeResult


class HelloModule(Module):
    """A simple hello world module."""

    def id(self) -> ModuleId:
        return ModuleId("hello-python")

    def name(self) -> str:
        return "Hello Python Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        print(f"Hello from Python! Data dir: {ctx['data_dir']}")
        return ProbeResult.Success()

    def exit(self):
        print("Goodbye from Python!")
