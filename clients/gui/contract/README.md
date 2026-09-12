# GUI contribution profile

The first public contribution uses a narrow Editor capability and command
registration. A selection contains its observed Text snapshot, scalar/opaque
range and copied units. Replace rejects a different current snapshot before
crossing the native Text contract, which also checks actual source currentness.
Module code imports this contract, not a concrete GUI or Kernel Runtime.

The command owner admits at most 64 names (lowercase identifiers up to 64
characters) and 32 concurrent invocations per command. Duplicate names reject.
Closing a registration fences new calls immediately. Busy preserves its owner
until admitted calls settle; close is then retryable and idempotent. An old
registration cannot remove a later registration of the same name.

The uppercase contribution demonstrates dynamic activation and explicit unload.
It transforms only selected Unicode scalars and refuses opaque units. It owns
neither Buffer nor History, and all changes use the supplied Editor capability.
The hosting app supplies view selection, grants and registration lifetime.
Unload deactivates registrations; it does not promise removal of code from
the browser's ES module cache.

This small profile does not complete the GUI extension SDK, module discovery,
dependency negotiation, keymaps or packaging. JavaScript capabilities describe
the intended interface; a shared JavaScript realm is not a memory sandbox.
Kernel native loading and Client contribution loading remain separate owners.
