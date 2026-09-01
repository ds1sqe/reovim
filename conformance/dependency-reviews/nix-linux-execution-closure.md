# Nix Linux Execution Closure Review

Status: selected only for Linux Repository Tooling; final admission requires
the integrated locked closure, retained Plan 07 measurements, and complete
namespace/process fault evidence.

## Exact need and feature selection

The Product-proof checker needs user/mount/PID/network namespaces, mount/
unmount, chroot/chdir, process limits, process groups/signals/waits, and parent-
death state without first-party unsafe code or distro-specific orchestration
executables. The direct dependency is exactly:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "=0.31.3", default-features = false,
        features = ["fs", "mount", "resource", "sched", "signal"] }
```

Effective features are `fs`, `mount`, `process`, `resource`, `sched`, `signal`,
and `uio`. `user` is unnecessary because bounded `/proc` parsing and `std::fs`
own UID/GID mapping; `net` is unnecessary because `sched::CloneFlags` supplies
`CLONE_NEWNET`. Enabling either feature requires renewed review.

## Exact researched closure

Resolved with Cargo/Rust 1.92 on 2026-09-01:

| Package | Role | Checksum | License | Declared MSRV |
| --- | --- | --- | --- | --- |
| `nix 0.31.3` | direct/runtime | `cf20d2fde8ff38632c426f1165ed7436270b44f199fc55284c38276f9db47c3d` | MIT | 1.69 |
| `bitflags 2.13.1` | runtime | `b588b76d00fde79687d7646a9b5bdf3cc0f655e0bbd080335a95d7e96f3587da` | MIT OR Apache-2.0 | 1.56 |
| `cfg-if 1.0.4` | runtime | `9330f8b2ff13f34540b44e946ef35111825727b38d33286ef986142615121801` | MIT OR Apache-2.0 | 1.32 |
| `libc 0.2.189` | runtime, `std+extra_traits` | `3eaf3ede3fee6db1a4c2ee091bf8a8b4dccdc6d17f656fb07896ee72867612f2` | MIT OR Apache-2.0 | 1.65 |
| `cfg_aliases 0.2.2` | build only | `f079e83a288787bcd14a6aea84cee5c87a67c5a3e660c30f557a3d24761b3527` | MIT | undeclared |

No other feature-selected package enters the closure. Final machine admission
uses the integrated lock and metadata-resolved consumers/profiles, not this
table alone.

## Safety, build, and runtime boundary

Nix provides safe caller APIs for the selected operations. Reovim remains
`#![forbid(unsafe_code)]` and may use only the reviewed safe subset:

- `sched::unshare`;
- `mount::{mount,umount,umount2}`;
- `unistd::{chroot,chdir,getpid,getppid,setpgid,getpgid}`;
- `sys::resource::{getrlimit,setrlimit}`;
- `sys::prctl::{get_pdeathsig,set_pdeathsig}`;
- `sys::signal::{kill,killpg}` including signal-zero probes; and
- `sys::wait::{wait,waitpid}`.

Unsafe `clone`, `fork`, signal installation, unchecked signal-set conversion,
and raw descriptor duplication remain forbidden. Nix and libc contain
substantial internal unsafe FFI; exact package source, kernel ABI behavior, and
safe-wrapper correctness are the dependency boundary rather than first-party
unsafe proof.

The Nix build script uses build-only `cfg_aliases` to inspect Cargo target/
feature environment and emit cfg declarations. Libc's build script invokes the
exact configured Rust compiler or wrapper for `--version`, always attempts
`emcc -dumpversion` through `PATH`, and may invoke `freebsd-version` when
`LIBC_CI` is set. Exact builds therefore clear `LIBC_CI`,
`LIBC_BUILD_VERBOSE`, unstable libc configuration variables, and use a
controlled PATH which proves `emcc` absent; the Rust compiler and any explicit
linker remain exact execution-tool/compiler-profile inputs. Any behavior or
build-input change invalidates this review.

## Process/mount conditions

The setup mode is a dedicated initially single-threaded same-binary process.
It creates/maps the user namespace before mount/network/PID namespaces, then
spawns namespace PID 1 through safe `std::process::Command`. Every supervisor
arms/verifies parent-death `SIGKILL` and uses a pipe handshake to close the
startup race. PID 1 mounts `/proc`, reaps descendants, applies rlimits, and
chroots/chdirs before Cargo. Parent process-group termination and namespace
teardown are both required.

The old mount API cannot make unknown nested submounts recursively read-only by
assertion. Each bind source has no nested mount or every visible nested mount is
enumerated, remounted, and verified through bounded mountinfo. No util-linux
fallback is admitted.

## Compatibility, maintenance, and security

The selected API surface passed isolated locked/offline check/build on Rust
1.92.0 and stable 1.98.0, including a `#![forbid(unsafe_code)]` live namespace/
PID-1/tmpfs/chroot/rlimit probe. This is feasibility evidence, not integrated
conformance or cross-host qualification.

Nix 0.31.3 was released 2026-05-11, supports Rust 1.69+, lists
`x86_64-unknown-linux-gnu` as Tier 1, and is MIT licensed. RustSec database
revision `72f8b23d78ea6c4c9ded301a4c6ec4260e8b4c27` has no match for the selected
version/profile; historical `RUSTSEC-2021-0119` affects old `getgrouplist` code,
is patched from 0.23 onward, and the `user` feature is disabled. “No match” is
not an unsafe/correctness/supply-chain proof.

Invalidators include any package/checksum/feature/lock change, Linux/kernel/
user-namespace policy change, mount or parent-death assumption change, new
advisory, unsafe-wrapper finding, and `cfg_aliases` behavior/MSRV change.

The actual newer lock is deliberate. `bitflags 2.13.1` fixes a macro-recursion
regression and enables no selected unsafe feature; `cfg_aliases 0.2.2` changes
macro punctuation/documentation without new dependency or process behavior;
and libc 0.2.187–0.2.189 includes an extern-type soundness correction while the
selected x86_64 GNU namespace/mount/prctl/rlimit/signal/wait declarations remain
unchanged. Artificial older transitive pins provide no demonstrated benefit.

## Cost and decision

Earlier build/size probes used the superseded transitive snapshot and are
discarded. They select no budget or cost claim.

Final admission requires retained clean/cached build cost, checker binary delta,
namespace/prepared-root setup/teardown p50/p99, peak RSS, scratch bytes/inodes,
timeout-to-kill/teardown/reap/join/cleanup latency, and comparison with the
discarded command-based design.

Decision: select this exact closure conditionally for the Linux Product-proof
checker. It grants no Product dependency, runtime Host API, cross-platform
claim, or permission to widen the feature/API set.

Primary references: [Nix 0.31.3 manifest](https://docs.rs/crate/nix/0.31.3/source/Cargo.toml),
[Nix upstream](https://github.com/nix-rust/nix),
[unshare source](https://docs.rs/crate/nix/0.31.3/source/src/sched.rs), and
[RUSTSEC-2021-0119](https://rustsec.org/advisories/RUSTSEC-2021-0119.html).
