"""
Counter Module with Hot Reload Support.

Demonstrates:
- State preservation across hot reload
- Using pickle for serialization
- Custom module methods

To install:
    cp counter.py ~/.local/share/reovim/modules/
"""

import pickle

from reovim import Module, ModuleId, ProbeResult


class CounterModule(Module):
    """A counter module demonstrating hot reload with state preservation."""

    def __init__(self):
        super().__init__()
        self._count = 0

    def id(self) -> ModuleId:
        return ModuleId("counter-python")

    def name(self) -> str:
        return "Python Counter Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        print(f"Counter initialized at: {self._count}")
        return ProbeResult.Success()

    def exit(self):
        print(f"Counter final value: {self._count}")

    def supports_hot_reload(self) -> bool:
        return True

    def save_state(self) -> bytes | None:
        """Serialize module state for hot reload."""
        return pickle.dumps({"count": self._count})

    def restore_state(self, state: bytes):
        """Restore module state after hot reload."""
        data = pickle.loads(state)  # noqa: S301 - trusted internal data
        self._count = data.get("count", 0)
        print(f"Counter restored to: {self._count}")

    # Public API for other modules or commands
    def increment(self) -> int:
        """Increment the counter and return new value."""
        self._count += 1
        return self._count

    def decrement(self) -> int:
        """Decrement the counter and return new value."""
        self._count -= 1
        return self._count

    def get_count(self) -> int:
        """Get the current counter value."""
        return self._count

    def reset(self):
        """Reset the counter to zero."""
        self._count = 0
