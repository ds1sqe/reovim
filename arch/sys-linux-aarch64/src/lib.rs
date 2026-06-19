//! aarch64-linux raw per-target mechanism (`reovim-arch-sys-linux-aarch64`).
//!
//! The raw machine layer for `aarch64-linux`: the raw syscall asm primitives,
//! the `aarch64` syscall-number table, the typed-wrapper floor, the socket and
//! termios kernel-ABI surface, and the fused clone asm (`clone_into`). Returns
//! NATIVE values; canonicalization to POSIX is a provider-tier concern.
//! `reovim-arch` re-exports the whole surface through its `sys` facade so
//! consumers compile unchanged.

#![no_std]
#![allow(unsafe_code)]

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod errno;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod wrap;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod net;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod raw;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod term;

// The sibling `*_tests.rs` files (`errno_tests`, `net_tests`, `term_tests`,
// `wrap_tests`) are declared next to their source modules (the `#[path]` child
// pattern), reaching the test runtime through the `reovim-testrt` leaf rather
// than an upward arch-sys -> arch edge (SP07).

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub use errno::{
    EADDRINUSE, EAGAIN, EBADF, ECONNREFUSED, EFAULT, EINVAL, ENOENT, ENOMEM, ENOTTY, EOPNOTSUPP,
    EPIPE, EWOULDBLOCK, Errno, from_ret,
};
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub use raw::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall6};
// The wrapper-level floor: the typed syscall wrappers plus their constant
// vocabulary. The Linux-family `wrap.rs` realizes it over the syscall ABI.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub use wrap::{
    AT_FDCWD, AT_REMOVEDIR, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES,
    CLONE_FS, CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM,
    FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE,
    MSG_NOSIGNAL, O_CLOEXEC, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ,
    PROT_WRITE, Timespec, accept, bind, clock_gettime, close, connect, exit, exit_group, futex,
    gettid, ioctl, listen, mmap, mprotect, munmap, openat, read, send_nosignal, socket,
    unix_stream_socket, unlinkat, write,
};

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use core::arch::asm;

/// Issues `clone` and branches the child straight to `entry` on its new stack.
///
/// The whole sequence is one `asm!` block so the child never executes a Rust
/// function epilogue on its fresh stack (the classic clone-from-Rust hazard).
///
/// On `aarch64` a `clone`-created child resumes after the `svc #0` instruction
/// with `x0 == 0` and `sp == stack`. The asm checks `x0`: in the parent it
/// returns the child tid as a normal value; in the child it loads the argument
/// from the stack top into `x0` (the AAPCS64 first argument) and `blr`s
/// `entry`, which never returns (it issues the `exit` syscall). Because the
/// child's control transfer happens entirely inside this asm block, no Rust
/// caller frame is ever unwound on the wrong stack.
///
/// Note the `aarch64` clone-argument order differs from `x86_64`: the kernel's
/// `asm-generic` ABI takes `clone(flags, stack, ptid, tls, ctid)` — `tls` and
/// `ctid` are swapped relative to the `x86_64` `CLONE_BACKWARDS` ABI
/// (`flags, stack, ptid, ctid, tls`). The argument registers below reflect the
/// `asm-generic` order.
///
/// # Errors
///
/// Returns the `clone` [`Errno`] if the syscall fails in the parent; the child
/// path never returns to report an error.
///
/// # Safety
///
/// `flags` must form a coherent thread set; `stack` must be the top of a
/// valid stack region the child owns, with the trampoline argument stored at
/// `[stack]`; `join_word` must be a valid live word used for both
/// `CLONE_PARENT_SETTID` and `CLONE_CHILD_CLEARTID`; `entry` must be the
/// trampoline matching the argument's type.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub unsafe fn clone_into(
    flags: usize,
    stack: usize,
    join_word: usize,
    entry: usize,
) -> Result<usize, Errno> {
    let ret: isize;
    // SAFETY: the `clone` syscall (nr 220) takes flags in x0, stack in x1,
    // ptid in x2, tls in x3 (0, TLS-free), ctid in x4 — the `asm-generic`
    // argument order (NOT the `x86_64` order, where tls/ctid are swapped).
    // `ptid` and `ctid` both hold `join_word`: the kernel writes the child tid
    // there before returning in the parent (PARENT_SETTID), and clears+wakes
    // it on child exit (CHILD_CLEARTID). After `svc`, the parent path falls
    // through with the tid in x0; the child path (x0 == 0) loads its stack-top
    // argument into x0 and `blr`s `entry`, which never returns. The child's
    // initial `sp` is `8 mod 16` (the parent stored the one arg word at an
    // address `8 mod 16`, mirroring the `x86_64` `pop` layout); the
    // post-increment `ldr x0, [sp], #8` consumes that word and leaves `sp`
    // `0 mod 16`, the AAPCS64 alignment `blr` requires (unlike `x86_64`'s
    // `call`, `blr` pushes nothing — the return address goes to the link
    // register — so this alignment is final at the call). `entry` is PINNED to
    // x9 (a caller-saved temporary, neither a syscall-argument register x0..x5
    // nor the nr register x8): the kernel preserves it across `svc`, so the
    // child's `blr` always reaches the trampoline. A generic `in(reg)` could be
    // allocated to x0..x5, which the syscall path overwrites.
    unsafe {
        asm!(
            "svc #0",
            "cbnz x0, 2f",        // x0 != 0 ? parent : child
            // --- child: sp == new stack, arg at [sp] ---
            "ldr x0, [sp], #8",   // arg -> x0 (AAPCS64 first arg); sp -> 16-aligned
            "blr x9",             // run trampoline (never returns)
            "brk #1",             // unreachable: trampoline exits the thread
            "2:",                 // --- parent continues here ---
            in("x8") raw::nr::CLONE,
            // `x0` carries the flags arg in but is the syscall return register
            // and is overwritten by the child branch (`ldr x0`), so `=> ret`.
            inlateout("x0") flags => ret,
            in("x1") stack,
            in("x2") join_word,   // ptid
            in("x3") 0_usize,     // tls (TLS-free)
            in("x4") join_word,   // ctid
            in("x9") entry,
        );
    }
    from_ret(ret)
}

/// Hardware-capability probe for `compiler_builtins`' outline-atomics
/// helpers — the floor's own answer, since there is no libc to provide one.
///
/// rustc's `aarch64-linux` codegen routes every atomic RMW through
/// `__aarch64_*` helpers that pick LSE or LL/SC at runtime by probing
/// `getauxval(AT_HWCAP)`, an extern "C" call normally satisfied by libc.
/// A `DAG6` binary links no libc, so the floor defines the symbol itself.
/// Returning 0 reports no capabilities: the helpers permanently select the
/// LL/SC path, which is correct on every `ARMv8.0` core and the only valid
/// path on the primary hardware target (Pi 4, Cortex-A72, no LSE).
///
/// If a future consumer wants real LSE dispatch on server-class hosts, the
/// honest upgrade is parsing the auxiliary vector that follows `envp` on the
/// initial stack — not linking libc.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
extern "C" fn getauxval(_kind: usize) -> usize {
    0
}
