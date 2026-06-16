# Multi-OS Ports — Beyond-0.16

**Status:** Non-normative. Hosted ports past the 0.16 goal.

0.16's OS-mode goal is two concrete targets: **Over-OS on Linux** and
**RTOS-itself on bare-metal aarch64 (Raspberry Pi 4)** — see
`01-Architecture/06-OS-Modes.md`. Hosted ports to **Windows / macOS /
Solaris/illumos** are a separate, later track. They are recorded here
because they would force one spec amendment (DAG6) that bare metal does
not.

## The porting model

Linux is the only OS where raw syscalls are stable ABI. Each target's
port = reimplement `arch/src/sys/` against that target's lowest **stable**
boundary; the public `arch/` surface does not change, and product crates
do not change.

| Target | Stable boundary | `sys/` binds to |
|---|---|---|
| Linux | kernel syscall ABI | inline-asm syscalls (current) |
| Windows | kernel32 / documented ntdll | `VirtualAlloc`, `CreateThread`, `WaitOnAddress`/`WakeByAddress` (direct futex analog), `QueryPerformanceCounter`, `ReadFile`/`WriteFile`; PE entry via `mainCRTStartup` arm in `entry!`; `LoadLibraryW` |
| macOS | libSystem (syscall numbers explicitly unstable; Go retreated to libSystem) | libc `mmap`/`read`/`write`/`clock_gettime`, `pthread_create`, `os_sync_wait_on_address` (public ≥14.4) or pthread condvars; `dlopen` from libSystem |
| Solaris/illumos | libc (syscall table private) | libc + pthread primitives (no futex equivalent → Mutex/Condvar over pthread) |

Mechanics:

- **Structure:** `arch/src/sys/{linux_x86_64, linux_aarch64, windows,
  macos, ...}`, cfg-selected modules with **identical** function
  signatures (the Rust-std `sys/` / Linux `arch/` pattern). No traits, no
  vtable cost.
- The mmap-slab allocator ports everywhere: each target only supplies a
  "give me N pages" primitive.
- `entry!` grows per-target arms (`_start` ELF / `mainCRTStartup` PE / crt
  `main` on macOS where libSystem must be linked anyway).
- **Sequencing (rule of three):** do NOT build the matrix speculatively.
  The cheapest second target that forces the right structure is
  **aarch64-linux** — same kernel ABI, new syscall numbers + `svc #0` asm —
  which is also the Pi 4 under Linux, i.e. step 0 of the bare-metal track.

## The doctrine tension (DAG6)

DAG6 says "raw syscall FFI (no libc)" workspace-wide. On macOS/Solaris the
vendor-stable floor **is** libSystem/libc; raw syscalls there are unstable
ABI. This must be resolved before a macOS/Solaris port — but **not** before
bare metal, which has no libc to want.

Options:

- **(a) Amend DAG6** to "each target binds to its lowest stable boundary"
  (`extern "C"` to system libraries remains no_std and
  dependency-sovereign). **Recommended.**
- **(b)** Accept breakage on OS updates. Rejected.

Recorded as a spec delta to apply whenever an OS port is scheduled. Until
then DAG6 stands as written for the 0.16 targets.

## Open question

Optional, when an OS port lands: enumerate supported targets / target
classes (kernel-ABI, system-library, freestanding) somewhere in
`01-Architecture/`.
