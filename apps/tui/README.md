# TUI Source client

Experimental hosted consumer of Source/Text and Buffer History. Build with
`cargo build --manifest-path product/Cargo.toml -p reovim-tui --features std`.

```text
reovim-tui ADDRESS CLIENT @BUFFER @TEXT_FACTORY [--checkpoint FILE]
reovim-tui ADDRESS CLIENT @BUFFER @TEXT_FACTORY --journal NEW_DIRECTORY
reovim-tui --inspect-checkpoint FILE
```

The current invocation borrows an existing service Client and same-scope Resource
identities. It does not create a Client or disconnect that borrowed Client.
Input and presentation use the portable clients/tui implementation: scalar Text
positions, explicit opaque units and grapheme-aware movement/deletion.

F5 refreshes the current window or retries settled startup failure. F6 recovers
the original pending operation. Ctrl+Z/Ctrl+Y perform Undo/Redo; Ctrl+Q leaves.
Queued input and submitted actions prevent normal exit until settled or
explicitly discarded with Ctrl+D. An unresolved submitted action requires F6;
Ctrl+D cannot discard it. Refresh preserves failed actions and keeps editing
fenced until explicit resolution. A failed close exits nonzero and reports
retained service custody.

## Explicit checkpoint export

`--checkpoint FILE` exports Session and retained input when terminal processing
or worker cleanup fails. It creates a new file with private Unix permissions,
writes bounded
versioned data and syncs the file. Existing files are never overwritten.
Unsupported private-file permission paths return Unsupported.

The checkpoint contains the Client credential, exact pending frame/result,
operation, accepted Buffer state and Resource owners. It also records queued
logical input, submitted action with its original basis, cursor basis/intent
and pending/halted state. Keep it private.
`--inspect-checkpoint` validates its structural encoding, request transitions
and decoded Text qualifiers, then displays a summary without network I/O.
It also checks the saved operation against its request target, contract, input
and references, and checks adopted results against retained Resource owners.

This is not a crash journal: no checkpoint is written before request I/O,
automatic process resume is not implemented. Submitted actions remain retained
even when request construction fails. Server restart does not preserve these
Client identities.
Full journal ownership, restore validation and Client startup remain in the
durable implementation plan; compilation does not qualify those behaviors.

## Service-boundary journal

`--journal NEW_DIRECTORY` requires a new private directory. Before service I/O,
the worker requests a retained-input snapshot from the terminal loop and writes
it together with Session state. It syncs a private temporary file, replaces
`state` atomically, and syncs the directory. The terminal loop performs no disk
I/O. Shutdown continues serving snapshot requests after terminal restoration.

Persistence failure prevents the dependent call or receipt release. A prepared
call that was not transmitted remains a known-unsent reservation. The previous
checkpoint remains available when replacement fails before rename.

The current profile saves at service boundaries. Cursor changes while idle and
input arriving during blocked service I/O await the next boundary; continuous
input durability and automatic process resume are unfinished. Existing journal
directories cannot yet be reopened by the TUI. Inspect `DIRECTORY/state` with
`--inspect-checkpoint`; it contains credentials and must remain private.
