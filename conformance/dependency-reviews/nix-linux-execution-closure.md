# Nix Linux Process-Group Closure Review

Status: admitted only for the current Linux Repository Tooling process
supervisor and invalidated by any consumer, API, feature, or closure change.

## Exact consumer and selection

The current checker owns bounded child capture, one dedicated process group,
whole-group termination, leader reap, descendant/group-absence observation,
and reader joins. `tools/conformance/src/process.rs` is the named consumer. It
uses Nix only for typed Linux process-group identity and signaling without
first-party unsafe code.

```toml
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "=0.31.3", default-features = false,
        features = ["signal"] }
```

The effective Nix features are `process` and `signal`. Filesystem, mount,
resource, scheduler, socket, user, and network features are not admitted. A
Product prepared root, namespace coordinator, or another execution profile is
a new `TOOL-001` consumer/block decision and cannot widen this selection.

## Exact locked closure

Resolved with Cargo/Rust 1.92 from the repository lock:

| Package | Role | Checksum | License | Declared MSRV |
| --- | --- | --- | --- | --- |
| `nix 0.31.3` | direct/runtime | `cf20d2fde8ff38632c426f1165ed7436270b44f199fc55284c38276f9db47c3d` | MIT | 1.69 |
| `bitflags 2.13.1` | runtime | `b588b76d00fde79687d7646a9b5bdf3cc0f655e0bbd080335a95d7e96f3587da` | MIT OR Apache-2.0 | 1.56 |
| `cfg-if 1.0.4` | runtime | `9330f8b2ff13f34540b44e946ef35111825727b38d33286ef986142615121801` | MIT OR Apache-2.0 | 1.32 |
| `libc 0.2.189` | runtime | `3eaf3ede3fee6db1a4c2ee091bf8a8b4dccdc6d17f656fb07896ee72867612f2` | MIT OR Apache-2.0 | 1.65 |
| `cfg_aliases 0.2.2` | build only | `f079e83a288787bcd14a6aea84cee5c87a67c5a3e660c30f557a3d24761b3527` | MIT | undeclared |

The machine admission uses integrated lock and metadata-resolved
consumer/profile records. This table does not substitute for them.

## Reviewed API and behavior

The first-party crate remains `#![forbid(unsafe_code)]`. Its complete admitted
Nix API is:

- `unistd::{Pid, getpgid}`;
- `sys::signal::{Signal, killpg}`; and
- `errno::Errno` for exact absence/error classification.

The supervisor creates the group through safe `std::process::Command` setup,
validates the leader/group relation before accepting a result, signals the
owned group on timeout or residue, reaps the leader, waits for group absence,
and joins both bounded readers. `ESRCH` is accepted only where terminal absence
is the required relation; another error is typed failure. Kernel-assigned PID
and PGID values are observations and are omitted from deterministic report
identity.

Nix and libc contain internal unsafe FFI. Their safe-wrapper/kernel-ABI
correctness, exact package bytes, build scripts, and the selected Linux target
are the dependency boundary. Unsafe `fork`, `clone`, signal installation, raw
descriptor operations, and all unselected modules are forbidden.

Nix uses `cfg_aliases` at build time. Libc's build script may inspect compiler
and target configuration. Exact checker builds therefore retain the selected
compiler, target, lock, cleared control variables, and controlled environment
through the existing repository evidence contract.

## Compatibility, cost, and invalidation

Nix 0.31.3 supports Rust 1.69+, lists
`x86_64-unknown-linux-gnu` as Tier 1, and is MIT licensed. The selected closure
builds on Rust 1.92 and current stable under the repository's locked/offline
checks. Process deadline, group kill/absence, reap, reader, and publication
fault tests are the active consumer evidence; namespace or sandbox probes are
not evidence for this selection.

No standalone performance or release claim is made. The dependency is already
on the current checker path, so normal candidate/exact gate duration and binary
size measurements own its observed cost. A version, checksum, feature, lock,
consumer, API, target, build-script, advisory, unsafe-surface, or process-group
semantic change invalidates this review.

Decision: admit this exact process/signal closure for the current Linux
Repository Tooling supervisor. Stop at that consumer. Another execution
mechanism requires a separate Tooling Acceptance record, dependency review,
measurements, and end-to-end use.

Primary references: [Nix 0.31.3 manifest](https://docs.rs/crate/nix/0.31.3/source/Cargo.toml),
[Nix upstream](https://github.com/nix-rust/nix), and
[Nix signal API](https://docs.rs/nix/0.31.3/nix/sys/signal/index.html).
