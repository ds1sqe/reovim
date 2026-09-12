# Hosted terminal editing dependency

The interactive CLI uses [Rustyline 18.0.1][source] with default features off.
It supplies terminal-mode restoration, Unicode cursor editing, keyboard input,
completion display and in-memory history. Reovim supplies Resource/PATH lookup
and invocation policy. This dependency is gated by the CLI's std feature;
portable Kernel/module crates do not depend on it.

The [manifest][manifest] and [MIT license][license] were reviewed. Direct Unix
dependencies include nix/libc for terminal/OS operations, unicode-width and
unicode-segmentation for display, utf8parse, memchr, cfg-if, bitflags and log.
cfg_aliases is a build dependency. Windows adds windows-sys/windows-link and
clipboard-win/error-code. The latter two use BSL-1.0; the remaining packages
use MIT and/or Apache-2.0 (memchr also offers Unlicense). Cargo.lock owns the
exact resolution. Reovim's own bitflag and vtable macros remain the owners of
its public contracts; the third-party bitflags package is internal to this
hosted terminal dependency chain.

Disabled defaults exclude file-history support, directory helpers and custom
key-binding tries. SQLite, fuzzy matching and derive macros are not selected.
No history file is loaded or saved. We use custom completion rather than
filesystem completion, so Tab queries only the client's granted namespace.

Unsafe OS/terminal operations remain inside the dependency's platform adapter;
the CLI itself introduces no unsafe code. Real PTY tests exercise Tab without
invocation, history Up/Down, cursor edits, Ctrl-C, Ctrl-D and invalid-CWD repair.
Windows compilation is checked separately from the Linux terminal journey.

Measured GNU size text+data+bss for the development CLI changed from 608,387
bytes at the prior audit to 1,439,836 bytes with PATH and terminal editing.
This measures the combined CLI change, not isolated Rustyline cost, heap use,
release size or startup latency. The added cost buys terminal behavior rather
than introducing an in-repository raw-terminal/Unicode editing subsystem.

[source]: https://github.com/kkawakam/rustyline/tree/v18.0.1
[manifest]: https://github.com/kkawakam/rustyline/blob/v18.0.1/Cargo.toml
[license]: https://github.com/kkawakam/rustyline/blob/v18.0.1/LICENSE
