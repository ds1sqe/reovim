# 00 — Vocabulary

Cross-chapter terms. A term is defined here if it appears in more
than one chapter or names a stable surface. Chapter-local terms stay
in the chapter that owns them.

## Process

| Term | Definition |
|---|---|
| **Init** | The boot actor of a server runtime. Exclusively owns kernel state during boot (no locks), then moves ownership into `Arc<Kernel>`; boot-only state dies with it (2.1). |
| **Kernel** | The steady-state root structure of a server runtime: the registries and state behind the hostapi. Constructed only by `Init::boot`; shared via `Arc`; protected by per-field locks. |
| **Session** | Named editing context; `SessionId(Arc<str>)`. Shares buffers and kernel state across attached clients. |
| **Client** | One transport-attached consumer of a session; `ClientId(usize)`. Independent mode, cursor, viewport. |
| **CdylibId** | `NonZeroU32` interned at load. Identifies one runtime-loaded cdylib for the lifetime of the process. |
| **CdylibState** | One of `Loaded`, `Active`, `Draining`, `Shutting`, `Unloaded`, `FailedInit`, `Panicked`. See `02-Process/02-Lifecycle.md`. |
| **RefGuard** | A counted reference held by the kernel for the duration of a cdylib slot invocation. Blocks unload while held. |
| **Generation** | A monotonic counter on each `CdylibId` that increments on every state transition. Used to detect stale registry references. |
| **Panic disposition** | Boot-only policy `kernel.host.[panic].disposition` selecting what the `arch/` panic handler does after the final ring flush (9.5 §9.1) and the persisted-state record: `recover` (supervised restart + persistence restore + quarantine; default) or `halt` (stop for analysis; dev/test posture). See 6.2 §5 (AB12). |
| **Final flush** | The panic-time act of the `arch/` panic handler: the panic renders into the one kernel log ring as a normal LOG2 line (with owning-cdylib attribution), and the ring tail is synchronously flushed to the LOG7 file via raw syscalls. The pstore analog — same file, same format, no second log. See 9.5 §9.1. |
| **Quarantine** | The recorded-at-restart consequence of a panic under `recover`, read from the persisted state (2.4): the attributed cdylib is not loaded again until explicitly cleared (7.1 `pkg`). First panic quarantines; there is no strike count. The Linux taint/blacklist analog. |

## Domain & coordination

| Term | Definition |
|---|---|
| **Domain** | An identifier (`DomainId(NonZeroU32)`) plus a routing entry. Behaviour lives in handlers/projectors/codecs registered under that ID, not in a Rust trait. |
| **DomainAttachment** | A live attachment of a Domain to a Buffer at a Scope. Carries `struct_refs` (structural lifetime) and `focus_refs` (focus-chain lifetime). |
| **PendingAttachment** | A focus-chain entry whose Domain is registered but whose cdylib is not yet `Active`. Carries enough context to resolve later. |
| **Scope** | Position-stable identity of a sub-region within a Buffer that a Domain has attached to. |
| **PositionCarrier** | Header (8 bytes: domain_id, inner_id, flags) + opaque content bytes. Domain-neutral position. (Distinct sense from the protocol **carrier** — see Surfaces.) |
| **CursorCarrier** | Same shape as `PositionCarrier`, used for cursors and selections. |
| **Codec** | Registered behaviour for `(domain_id, inner_id)`: validate, equal, display, persist. |

## ABI

| Term | Definition |
|---|---|
| **Vtable** | `#[repr(C)]` struct of function pointers exported by a cdylib. Begins with `VtableHeader`. |
| **VtableHeader** | `{ abi, api, size_of_self, kind, flags }`. Read before any vtable function is called. |
| **AbiVersion** | `(major, minor, patch)`. Binary epoch. Loader rejects on `major` mismatch or `minor > kernel.minor`. |
| **ApiVersion** | Kind-specific semantic contract version inside a fixed ABI. |
| **ManifestKind** | The vtable kind a cdylib declares: `module-server`, `driver-server-*`, `module-client`, `driver-client-*`, `capability-client-*`. |
| **ConfigSlice** | Kernel-owned, init-lifetime view of a participant's merged config; layout in `06-ABI/04-Config-Slice.md`. |

## Config

| Term | Definition |
|---|---|
| **Participant** | A unit that owns a config namespace. Kinds: `kernel.host`, `kernel.shell`, `module.<name>`, `driver.<kind>.<name>`, `pkgs`. |
| **ParticipantId** | The participant's stable namespace string. |
| **Trust class** | Per-field schema attribute: `host`, `shell`, or `module-private`. Determines which layers may set the field. |
| **Layer** | One of seven precedence levels (defaults, system, user, project, env, CLI, force). Last wins. |
| **Force override** | Top-precedence developer escape hatch. Logged at WARN; redacted for `secret` fields. |
| **ProjectRootResolver** | Abstraction that returns the project root used by layer 4. v1 returns launcher cwd. |

## Surfaces

| Term | Definition |
|---|---|
| **Library root** | `$ROOT/{module,driver,capability}/{server,client}/<name>.<ext>` install layout. See `01-Architecture/03-Apps-and-Invocation.md`. |
| **Lockfile** | `lockfile.toml` at the library root; SHA-256 over each shipped artifact. Required for `pkg sync`. |
| **Manifest** | Per-artefact TOML at `manifest/<kind>/<name>.toml`. Names the symbol(s), kind, platforms, vtables. |
| **DS12** | The kernel's structured-event channel. Required event families listed in `09-Conformance/04-Observability.md`. |
| **Correlation ID** | Per-request identifier propagated through DS12 events for one external operation. |
| **Carrier (protocol)** | The replaceable byte-mover the framed message protocol rides on: the default framed UDS/TCP stream, or HTTP, WebSocket, gRPC, pipes, file IO. Satisfies the 7.3 §1a carrier contract; never inspects message bodies (SP16). Distinct from the coordination `PositionCarrier`/`CursorCarrier` (4.5). |
| **Sans-IO** | The purity discipline of `uapi/protocol` (SP15): the codec and protocol state machine are pure computation over byte slices — no syscalls, no IO traits, no timers, no allocation. |
| **Log ring** | Bounded in-memory ring of rendered log lines, retained from boot stage 0; the `dmesg` analog. The ONE log buffer at kernel level — every log path, including the panic-time final flush (9.5 §9.1), goes through it. See `09-Conformance/05-Logging.md`. |
| **Emitter address** | `kind/CdylibId.vtable` token identifying where in the loaded topology a log line originated; kernel subsystems use a bare name. See `09-Conformance/05-Logging.md` §3. |

## Architectural categories

| Term | Definition |
|---|---|
| **Foundation** | `arch/`, `lib/*`, `uapi/*`. No upward deps. |
| **Platform floor** | `arch/` in its `DAG6` role: the single crate owning raw syscall FFI, process entry/exit, the panic handler, the allocator, sync primitives, and all heap data structures (1.2 §10). |
| **Bootstrap state** | A tracked, transitional zero-std exemption recorded in the 1.2 §10 table (libtest in test builds; depgraph/scripts ground tooling). Sequencing necessity, never convenience; ratchets to zero. |
| **Server contracts** | `server/lib/subsys/*`. Closed; zero ext deps. |
| **Server kernel** | `server/lib/kernel/*`. Mechanism; no ext or client deps. |
| **Server runtime** | `server/lib/server/*`. Framed-protocol + dispatch glue. |
| **Client contracts** | `clients/lib/subsys/*`. Closed; zero ext deps. |
| **Server extensions** | `ext/server/{modules,drivers,providers,domain}`. Runtime-loaded. |
| **Client extensions** | `ext/client/{platforms,driver,module,capabilities}`. Flat-category tree. |
| **Composition roots** | `apps/*`. Wire layers into bins. |
| **Tools** | `tools/*`. Dev / test / perf only. Non-shipping. |

## Reserved identifiers

- `__meta` is reserved as a TOML table name across all participant
  namespaces. Used for schema-version metadata in user files. No
  participant may declare a field named `__meta`.
- `pkgs` is the only participant namespace not under
  `module.` / `driver.` / `kernel.`.
- The `flags` bit `0x8000` in `PositionHeader` and `CursorHeader` is
  reserved for kernel annotation; participant codecs MUST NOT set it.
