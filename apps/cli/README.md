# Resource shell

The CLI discovers Resource interfaces and invokes their declared methods through
public Caller frames. It has no backend implementations or domain schemas.
Paths acquire zero-right access; each invocation selects its declaration's
operation and required access before calling the bound interface.

Build the CLI with `--features std`. Start the
[server](../server/README.md), then use its announced address and Caller token:

```sh
product/target/debug/reovim-cli ADDRESS TOKEN JOURNAL inspect /load
```

`JOURNAL` is one persistent file for that Caller and address. Reuse it on every
CLI restart while the server retains the Caller. One coordinator owns each
credential: do not use another journal or independent client with the same
credential. The journal takes an exclusive file lock and contains the credential
and complete input/output frames; new Unix files use mode 0600. A new server
issues a new credential and needs a separate journal.

## Commands

| Command | Result |
| --- | --- |
| `inspect PATH` | API identities, method names, operations and value types |
| `ls PATH` | Direct names as hex and Resource identities as `@HEX` |
| `invoke PATH METHOD HEX` | Call the uniquely named declared method |
| `close ACCESS` | Close a returned or retained opened access |
| `receipts` | IDs whose receipt custody is still retained |
| `retry` | Repeat the exact pending frame after interrupted observation |
| `result ID` | Original request's current evidence |
| `adopt ID` | Adopt successful returned access |
| `release ID` | Release receipt custody |
| `cancel ID` | Request cancellation; acknowledgement is not rollback |
| `reconcile ID` | Refine the original uncertain effect |
| `exit` | Exit the shell; the server retains the Caller |

`PATH` is an absolute Resource path or an `@HEX` Resource identity returned by
listing. `ACCESS` is a caller-local opened access number, a different identity.
`HEX` encodes canonical value bytes; `-` means empty data. Input reference slots
and qualified selection between same-named methods are not yet exposed.
Outputs print their data as hex and their returned access numbers. Contents
remain unrestricted bytes. Method names come from declarations, including the
loader's `admit` and the package's `activate`; the CLI does not hard-code them.

Without a command, terminal input uses line editing and 100 in-memory history
entries. Ctrl-C clears input and Ctrl-D exits. Piped input prints `ready`, then
`ok` or `failed` after each command. Lines are bounded to 16 KiB. Discovery is
bounded to 256 pages and 256 interfaces; namespace rows stream as pages arrive.

## Recovery and migration status

Each outgoing frame is flushed before transmission. Each reply is flushed
before advancing. A lost exchange blocks further frames until `retry` recovers
that exact exchange, including an allocation whose reply was lost. Disk-write
failure stops exchanges until the CLI reopens and reconstructs the transcript.
A torn final record is discarded; malformed complete records reject opening.
File and parent-directory flushes execute on Unix. Other platforms do not yet
have equivalent directory durability qualification.

Normal calls adopt returned access and release their receipt. Failed or unknown
outcomes retain the request ID for explicit recovery. Errors after acquisition
report the opened access to close after recovery; no cleanup call replaces an
unresolved frame. The journal retains raw successful output even if observation
ends before it is printed. It currently grows without compaction. This is not
complete durable user-input/session recovery or a server restart journal.

Retired Contract/Message/default-write dispatch has been replaced. The retained
`invocation.rs` and `shell/completion.rs` / `shell/interactive.rs` are uncompiled
donors: CWD/PATH, semantic argument authoring and completion still need migration
through declared services. The new terminal loop preserves basic line editing;
see the [dependency review](DEPENDENCIES.md). Full Client extension, Session and
cross-language SDK journeys remain in the implementation plan.
