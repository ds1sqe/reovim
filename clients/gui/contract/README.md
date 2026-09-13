# GUI contribution profile

The first public contribution uses a narrow Editor capability and command
registration. A selection contains its observed Text snapshot, scalar/opaque
range and copied units. Replace rejects a different current snapshot before
crossing the native Text contract, which also checks actual source currentness.
Module code imports this contract, not a concrete GUI or Kernel Runtime.
The browser derives selection endpoints from its highlighted textarea range,
checking the displayed projection before translating UTF-16 offsets to Text.
An empty uppercase selection performs no edit or History operation.

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

## Key bindings

[Keymaps](keymaps.ts) register logical key sequences against an existing
command and return an owned registration. The supplied capability determines
which command registry is available. A binding retains that registration's
identity; unloading/re-registering the same name cannot retarget old keys.
Close removes the binding and clears pending chords. Module activation rolls
back its command if binding registration fails; unload removes keys before
retiring the command and retains a busy command owner for retry.

Client Core resolves at most four keys per chord and 64 registered bindings.
The wait after each accepted prefix is 750 milliseconds on a monotonic clock.
Active named scopes precede the empty family scope, then higher priority wins.
Identical sequence/scope/priority conflicts at registration; equally ranked
active matches are ambiguous and invoke neither command. Registration order
does not rank commands. A complete match that is also a prefix waits; a
prefix-only timeout clears. Mismatch retries only the new key once.

The browser adapter currently activates the `text` scope. Blur, composition,
view changes, binding removal and input disposal fence pending chords. IME,
dead keys and AltGraph bypass command resolution. Unhandled keys remain native
input; commands never also synthesize committed text. The uppercase example
registers Ctrl+K then Ctrl+U; History uses the same resolver with the existing
input admission and recovery guards. Client modes can supply other scopes
when their actual consumer is implemented.

This small profile does not complete the GUI extension SDK, module discovery,
dependency negotiation or packaging. JavaScript capabilities describe
the intended interface; a shared JavaScript realm is not a memory sandbox.
Kernel native loading and Client contribution loading remain separate owners.
