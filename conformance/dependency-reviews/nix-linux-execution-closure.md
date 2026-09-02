# Nix Linux Execution Closure Review

Status: admitted only for the current Linux Repository Tooling process
supervisor and Plan 05's active Foundation Product-proof prepared root. Any
consumer, API, feature, or closure change invalidates this review.

## Exact consumer and selection

The checker has two exact consumers:

1. `tools/conformance/src/process.rs` owns bounded child capture, one dedicated
   process group, whole-group termination, leader reap, group-absence
   observation, and reader joins.
2. Plan 05's active Product-proof execution module owns the same-binary Linux
   prepared root: user/mount/network/PID namespaces, private propagation,
   quota tmpfs, exact read-only mounts, chroot, per-process limits, and bounded
   teardown. It exists only because the real Foundation and public consumer
   reached `PRODUCT_EXECUTION_PROOF_UNAVAILABLE`.

Both use safe Nix APIs without first-party unsafe code.

```toml
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "=0.31.3", default-features = false,
        features = ["fs", "mount", "resource", "sched", "signal", "socket"] }
```

The effective Nix features are `fs`, `memoffset`, `mount`, `process`,
`resource`, `sched`, `signal`, `socket`, and `uio`. Net, user,
asynchronous-I/O, poll, term, time, feature, ioctl, event, kmod, mman,
personality, pthread, reboot, zerocopy, and fanotify features are not admitted.
No util-linux or shell fallback exists.

## Exact locked closure

Resolved with Cargo/Rust 1.92 from the repository lock:

| Package | Role | Checksum | License | Declared MSRV |
| --- | --- | --- | --- | --- |
| `nix 0.31.3` | direct/runtime | `cf20d2fde8ff38632c426f1165ed7436270b44f199fc55284c38276f9db47c3d` | MIT | 1.69 |
| `bitflags 2.13.1` | runtime | `b588b76d00fde79687d7646a9b5bdf3cc0f655e0bbd080335a95d7e96f3587da` | MIT OR Apache-2.0 | 1.56 |
| `cfg-if 1.0.4` | runtime | `9330f8b2ff13f34540b44e946ef35111825727b38d33286ef986142615121801` | MIT OR Apache-2.0 | 1.32 |
| `libc 0.2.189` | runtime | `3eaf3ede3fee6db1a4c2ee091bf8a8b4dccdc6d17f656fb07896ee72867612f2` | MIT OR Apache-2.0 | 1.65 |
| `memoffset 0.9.1` | runtime | `488016bfae457b036d996092f6cb448677611ce4449e970ceaf42695203f218a` | MIT | undeclared |
| `cfg_aliases 0.2.2` | build only | `f079e83a288787bcd14a6aea84cee5c87a67c5a3e660c30f557a3d24761b3527` | MIT | undeclared |
| `autocfg 1.5.1` | build only | `f2032f911046de80f0a198e0901378627c33f59ea0ac00e363d481118bd70a53` | Apache-2.0 OR MIT | 1.0 |

The machine admission uses integrated lock and metadata-resolved
consumer/profile records. This table does not substitute for them.

## Reviewed API and behavior

The first-party crate remains `#![forbid(unsafe_code)]`. The process supervisor
uses:

- `unistd::{Pid, getpgid}`;
- `sys::signal::{Signal, killpg}`; and
- `errno::Errno` for exact absence/error classification.

The prepared-root consumer may additionally use only:

- `sched::{CloneFlags, unshare}`;
- `mount::{MntFlags, MsFlags, mount, umount2}`;
- `unistd::{Pid, chdir, chroot, getpid}`;
- `sys::signal::{Signal, kill}` and
  `sys::wait::{WaitPidFlag, WaitStatus, waitpid}` for bounded PID-namespace
  descendant termination and reap;
- `sys::prctl::{get_pdeathsig, set_pdeathsig}` for the namespace-PID-1
  parent-death relation;
- `fcntl::{fcntl, FcntlArg, OFlag}` only to preserve existing descriptor flags
  while adding `O_NONBLOCK` to the bounded Netfilter socket and the two owned
  namespace-PID-1 pipes;
- `fcntl::{open, OFlag}` and `sys::stat::Mode` for no-follow mountpoint and
  mount-target ownership;
- `sys::resource::{Resource, getrlimit, setrlimit}`;
- `sys::statvfs::{FsFlags, statvfs}` for mounted filesystem facts;
- `sys::stat::{major, minor}` for the exact `/dev/null` device identity;
- `sys::socket` safe descriptor, bind, local-address, send, and receive
  operations for one bounded Netfilter netlink table dump; and
- `errno::Errno` for typed retry, absence, and terminal error relations.

The supervisor creates the group through safe `std::process::Command` setup,
validates the leader/group relation before accepting a result, signals the
owned group on timeout or residue, reaps the leader, waits for group absence,
and joins both bounded readers. `ESRCH` is accepted only where terminal absence
is the required relation; another error is typed failure. Kernel-assigned PID
and PGID values are observations and are omitted from deterministic report
identity.

After `CLONE_NEWPID`, the prepared-root setup process creates no reader thread:
Linux rejects that clone pattern for this process state. It makes both owned
PID-1 pipes nonblocking through the reviewed safe `fcntl` wrapper, alternates
bounded reads in one supervision loop, validates the incremental READY prefix,
polls child status, enforces handshake/execution deadlines, and terminates and
reaps before returning any pipe or protocol failure. `EAGAIN`/`EWOULDBLOCK` and
`EINTR` mean only that the same bounded loop retries; another read or flag error
fails closed. No `poll` feature, helper process, unsafe descriptor operation,
or blocking fallback is admitted.

Nix and libc contain internal unsafe FFI. Their safe-wrapper/kernel-ABI
correctness, exact package bytes, build scripts, and the selected Linux target
are the dependency boundary. Unsafe `fork`/`clone`, signal installation, raw
descriptor operations outside the reviewed socket surface, and all unselected
modules are forbidden. The Netfilter query accepts only the fixed table-dump
reply sequence from a kernel socket address to the exact locally assigned
userspace port ID, bounds bytes/messages/deadline, and fails closed on policy
entries, kernel errors, truncation, or malformed native-endian framing.
PID-namespace child creation remains safe `std::process::Command`; Nix does
not grant a first-party unsafe block.

Nix uses `cfg_aliases` at build time. The socket feature adds `memoffset`,
whose build script uses `autocfg` to probe compiler capabilities and whose
implementation contains its own reviewed unsafe pointer arithmetic. Libc's
build script may inspect compiler and target configuration. Exact checker
builds therefore retain the selected compiler, target, lock, cleared control
variables, and controlled environment through the existing repository
evidence contract.

## Compatibility, cost, and invalidation

Nix 0.31.3 supports Rust 1.69+, lists
`x86_64-unknown-linux-gnu` as Tier 1, and is MIT licensed. The selected closure
builds on Rust 1.92 and current stable under the repository's locked/offline
checks. Process deadline, group kill/absence, reap, and reader faults remain
required. The Product checkpoint additionally requires namespace/mount/limit/
source/tool-identity/cleanup faults and a measured real Foundation journey.

No release or sandbox-against-host-root claim is made. The dependency was
already on the checker path; the prepared-root feature delta adds safe wrapper
modules plus the exact `memoffset`/`autocfg` lock closure.

On 2026-09-02, fresh locked/offline `x86_64-unknown-linux-gnu` release
builds compared `2c20cfc4` with the active Product-proof tree. Separate target
directories and `CARGO_INCREMENTAL=0` were used with Cargo/Rust nightly
1.100.0. The checker grew from 6,405,112 to 10,090,848 bytes (+57.54%);
wall time grew from 21.81 to 46.31 seconds (+112.33%), and maximum RSS from
932,800 to 1,377,156 KiB (+47.64%). Nix's selected features grew from
`process, signal` to `fs, memoffset, mount, process, resource, sched, signal,
socket, uio`. Its locked closure grew from five to seven packages, adding only
`memoffset 0.9.1` and build-only `autocfg 1.5.1`; the complete checker closure
grew from 15 to 17 packages. These measured binary, time, and RSS deltas are
accepted as conservative end-to-end checker deltas, not attributed solely to
Nix, because the current tree also contains the prepared-root proof.

Prepared-root setup/teardown and the real Foundation journey remain separate
required measurements before checkpoint.
A version, checksum, feature, lock, consumer, API, target, build script,
advisory, unsafe surface, namespace, mount, socket, Netfilter framing, limit,
or cleanup semantic change invalidates this review.

Decision: admit this exact closure for the two named Repository Tooling
consumers and no other mechanism. The Product prepared root must be invoked
end to end by the real Foundation journey and pass its faults/measurements
before checkpoint. Stop when that journey passes.

Primary references: [Nix 0.31.3 manifest](https://docs.rs/crate/nix/0.31.3/source/Cargo.toml),
[Nix upstream](https://github.com/nix-rust/nix), and
[Nix signal API](https://docs.rs/nix/0.31.3/nix/sys/signal/index.html),
[Nix scheduler API](https://docs.rs/nix/0.31.3/nix/sched/index.html),
[Nix mount API](https://docs.rs/nix/0.31.3/nix/mount/index.html),
[Nix resource API](https://docs.rs/nix/0.31.3/nix/sys/resource/index.html), and
[Nix socket API](https://docs.rs/nix/0.31.3/nix/sys/socket/index.html).
