//! x86_64-linux raw per-target mechanism (`reovim-arch-sys-linux-x86-64`).
//!
//! The raw machine layer for `x86_64-linux`: the raw syscall asm primitives,
//! the `x86_64` syscall-number table, the typed-wrapper floor, the socket and
//! termios kernel-ABI surface, and the fused clone asm (`clone_into`). Returns
//! NATIVE values (errno space, raw fds); canonicalization to POSIX is a
//! provider-tier concern, not this floor's. `reovim-arch` re-exports the whole
//! surface through its `sys` facade so consumers compile unchanged.

#![no_std]
#![allow(unsafe_code)]

// This crate carries `x86_64`-linux inline asm (raw syscall primitives + the
// fused clone trampoline). A workspace `cargo build`/`cargo test` compiles every
// member for the CURRENT host target, so the asm-bearing body is gated to its
// own triple via `TARGET` below — on any other target the body is excluded and
// the crate is an empty `#![no_std]` rlib (the `#![no_std]` attribute itself
// stays unconditional so cross-target none builds never fall back to std). The
// `arch::sys` facade only `use`s this crate on the matching target. The same
// gate convention applies to all four `arch-sys-{target}` crates.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod errno;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod wrap;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod net;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod raw;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod term;

// The sibling `*_tests.rs` files (`errno_tests`, `net_tests`, `term_tests`,
// `wrap_tests`) are declared next to their source modules (the `#[path]` child
// pattern), reaching the test runtime through the `reovim-testrt` leaf rather
// than an upward arch-sys -> arch edge (SP07).

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use errno::{
    EADDRINUSE, EAGAIN, EBADF, ECONNREFUSED, EFAULT, EINVAL, ENOENT, ENOMEM, ENOTTY, EOPNOTSUPP,
    EPIPE, EWOULDBLOCK, Errno, from_ret,
};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use raw::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall6};
// The wrapper-level floor: the typed syscall wrappers plus their constant
// vocabulary. The Linux-family `wrap.rs` realizes it over the syscall ABI.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use wrap::{
    AT_FDCWD, AT_REMOVEDIR, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES,
    CLONE_FS, CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM,
    FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE,
    MSG_NOSIGNAL, O_CLOEXEC, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ,
    PROT_WRITE, Timespec, accept, bind, clock_gettime, close, connect, exit, exit_group, futex,
    gettid, ioctl, listen, mmap, mprotect, munmap, openat, read, send_nosignal, socket,
    unix_stream_socket, unlinkat, write,
};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use core::arch::asm;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
/// Issues `clone` and branches the child straight to `entry` on its new stack.
///
/// The whole sequence is one `asm!` block so the child never executes a Rust
/// function epilogue on its fresh stack (the classic clone-from-Rust hazard).
///
/// On `x86_64` a `clone`-created child resumes after the `syscall` instruction
/// with `rax == 0` and `rsp == stack`. The asm checks `rax`: in the parent it
/// returns the child tid as a normal value; in the child it pops the argument
/// from the stack top into `rdi` (`SysV` first argument) and `call`s `entry`,
/// which never returns (it issues the `exit` syscall). Because the child's
/// control transfer happens entirely inside this asm block, no Rust caller
/// frame is ever unwound on the wrong stack.
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
pub unsafe fn clone_into(
    flags: usize,
    stack: usize,
    join_word: usize,
    entry: usize,
) -> Result<usize, Errno> {
    let ret: isize;
    // SAFETY: the `clone` syscall (nr 56) takes flags in rdi, stack in rsi,
    // ptid in rdx, ctid in r10, tls in r8 (0, TLS-free). `ptid` and `ctid`
    // both point at `join_word`: the kernel writes the child tid there before
    // returning in the parent (PARENT_SETTID), and clears+wakes it on child
    // exit (CHILD_CLEARTID). After `syscall`, the parent path falls through
    // with the tid in rax; the child path (rax == 0) pops its stack-top
    // argument into rdi and calls `entry`, which never returns. `rcx`/`r11`
    // are clobbered by `syscall` per the ABI — `entry` is therefore PINNED to
    // r9 (the sixth syscall-arg register, unused by 5-arg clone): a generic
    // `in(reg)` may legally be allocated to a `lateout` register such as rcx,
    // which `syscall` overwrites with the return RIP, turning the child's
    // `call` into a self-loop.
    unsafe {
        asm!(
            "syscall",
            "test rax, rax",      // rax == 0 ? child : parent
            "jnz 2f",             // parent: skip the child branch
            // --- child: rsp == new stack, arg at [rsp] ---
            "pop rdi",            // arg -> rdi (SysV first argument)
            "call r9",            // run trampoline (never returns)
            "ud2",               // unreachable: trampoline exits the thread
            "2:",                 // --- parent continues here ---
            // `rdi` carries the flags arg in but is overwritten by the child
            // branch (`pop rdi`), so it is `inout ... => _`.
            inout("rdi") flags => _,
            in("rax") raw::nr::CLONE,
            in("rsi") stack,
            in("rdx") join_word,
            in("r10") join_word,
            in("r8") 0_usize,
            in("r9") entry,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
    from_ret(ret)
}
