# 00 — Vocabulary

Cross-chapter terms. A term is defined here if it appears in more
than one chapter or names a stable surface. Chapter-local terms stay
in the chapter that owns them.

## Process

| Term | Definition |
|---|---|
| **Editor init** | `EditorInit`, the boot actor of the editor core. It owns boot-only state until the handoff and is consumed by `EditorInit::boot`. |
| **Editor core** | `editor/lib/core` / `reovim-editor-core`, the product edit-state mechanism: sessions, domains, registries, HostApi state, event/log policy, and services. |
| **Editor core root** | `EditorCore`, the steady-state root structure of the editor core: the registries and state behind the HostApi. |
| **Server runtime** | The process/transport host around the editor core: listener, framed protocol carrier, connection dispatch, and notify push. The editor usually runs inside the server runtime, but the editor core is not the server runtime. |
| **Session** | Named editing context; `SessionId(Arc<str>)`. Shares buffers and editor-core state across attached clients. |
| **Client** | One transport-attached consumer of a session; `ClientId(usize)`. Independent mode, cursor, viewport. |
| **CdylibId** | `NonZeroU32` interned at load. Identifies one runtime-loaded cdylib for the lifetime of the process. |
| **CdylibState** | One of `Loaded`, `Active`, `Draining`, `Shutting`, `Unloaded`, `FailedInit`, `Panicked`. See `02-Process/02-Lifecycle.md`. |
| **RefGuard** | A counted reference held by the editor core for the duration of a cdylib slot invocation. Blocks unload while held. |
| **Generation** | A monotonic counter on each `CdylibId` that increments on every state transition. Used to detect stale registry references. |
| **Panic disposition** | Boot-only policy keyed as `editor.host.[panic].disposition` selecting what the `arch/` panic handler does after the final ring flush (9.5 §9.1) and the persisted-state record: `recover` (supervised restart + persistence restore + quarantine; default) or `halt` (stop for analysis; dev/test posture). See 6.2 §5 (AB12). |
| **Final flush** | The panic-time act of the `arch/` panic handler: the panic renders into the one editor-core log ring as a normal LOG2 line (with owning-cdylib attribution), and the ring tail is synchronously flushed to the LOG7 file via raw syscalls. The pstore analog — same file, same format, no second log. See 9.5 §9.1. |
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
| **ConfigSlice** | Editor-core-owned, init-lifetime view of a participant's merged config; layout in `06-ABI/04-Config-Slice.md`. |

## Config

| Term | Definition |
|---|---|
| **Participant** | A unit that owns a config namespace. Current kinds include `editor.host`, `editor.shell`, `module.<name>`, `driver.<kind>.<name>`, and `pkgs`. |
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
| **DS12** | The editor core's structured-event channel. Required event families listed in `09-Conformance/04-Observability.md`. |
| **Correlation ID** | Per-request identifier propagated through DS12 events for one external operation. |
| **Carrier (protocol)** | The replaceable byte-mover the framed message protocol rides on: the default framed UDS/TCP stream, or HTTP, WebSocket, gRPC, pipes, file IO. Satisfies the 7.3 §1a carrier contract; never inspects message bodies (SP16). Distinct from the coordination `PositionCarrier`/`CursorCarrier` (4.5). |
| **Sans-IO** | The purity discipline of `uapi/protocol` (SP15): the codec and protocol state machine are pure computation over byte slices — no syscalls, no IO traits, no timers, no allocation. |
| **Log ring** | Bounded in-memory ring of rendered log lines, retained from boot stage 0; the `dmesg` analog for the editor core. The ONE product log buffer — every editor-core log path, including the panic-time final flush (9.5 §9.1), goes through it. See `09-Conformance/05-Logging.md`. |
| **Emitter address** | `kind/CdylibId.vtable` token identifying where in the loaded topology a log line originated; kernel subsystems use a bare name. See `09-Conformance/05-Logging.md` §3. |
| **Up-face** | The product-facing foundation contract family. Editor/client/product code names the public `reovim-uapi` facade (`uapi::net`, `uapi::sched`, etc.) for system-visible semantics; physical SSOT leaf crates live under `uapi/inner/*` for dependency enforcement. There is no product POSIX face: product semantics live in domain uapi leaves. |
| **Down-face** | The machine/provider-facing foundation contract family, `kabi/*`. Hardware/chip/device/provider code names this face and does not name `uapi/*` directly unless a documented inescapable exception exists; POSIX-shaped scalar ABI belongs here in `kabi/platform`. |
| **System kernel** | `system/lib/kernel`. The World-layer kernel and bridge that may name both `uapi/*` and `kabi/*`; owns common system semantics/policy and must not name `arch-*`, `arch-sys-*`, `arch-floor-*`, or `platform-*`. Unqualified **kernel** in new architecture prose refers to this layer only. |

## Architectural categories

| Term | Definition |
|---|---|
| **Foundation** | `uapi`, `uapi/inner/*`, `system/lib/kernel`, `kabi/*`, `lib/*`, `platform-*`, and `arch-*` families. No upward deps; up-face code names the `uapi` facade, down-face code names `kabi`, and the system kernel is the normal bridge between them. |
| **Platform floor** | `arch/` / `arch-*` in its `DAG6` role: target-owned raw mechanism and link items at each target's lowest stable boundary, per the 1.2 §10 target-class table. |
| **Bootstrap state** | A tracked, transitional zero-std exemption recorded in the 1.2 §10 table (libtest in test builds; depgraph/scripts ground tooling). Sequencing necessity, never convenience; ratchets to zero. |
| **Server contracts** | `editor/lib/subsys/*`. Closed; zero ext deps. |
| **Editor core category** | `editor/lib/core/*`. Editor Math mechanism; no ext, client, `kabi`, provider, or arch deps. Depgraph labels this category `EditorCore` / `editor-core`. |
| **Server runtime category** | `editor/lib/server/*`. Framed-protocol + dispatch glue. |
| **Client contracts** | `client/lib/subsys/*`. Closed; zero ext deps. |
| **Client core** | The client-side Math mechanism for raw-input normalization, projection/codec, frame/cell render, and capability slots. No hardened crate exists yet; new docs should use this term for that product mechanism. |
| **Server extensions** | `editor/{modules,drivers,providers,domains}`. Runtime-loaded. |
| **Client extensions** | `client/{platforms,drivers,modules,capabilities}`. Flat-category tree. |
| **Composition roots** | `apps/*`. Wire layers into bins. |
| **Tools** | `tools/*`. Dev / test / perf only. Non-shipping. |

## Reserved identifiers

- `__meta` is reserved as a TOML table name across all participant
  namespaces. Used for schema-version metadata in user files. No
  participant may declare a field named `__meta`.
- `pkgs` is the only participant namespace not under
  `module.`, `driver.`, or the editor-owned `editor.*` namespaces.
- The `flags` bit `0x8000` in `PositionHeader` and `CursorHeader` is
  reserved for editor-core annotation; participant codecs MUST NOT set it.
