//! x86_64-linux backend for `arch/src/sys/`.
//!
//! Everything target-specific in `sys/` lives here: the raw syscall asm
//! primitives, the `x86_64` syscall-number table, and the fused clone asm
//! (`clone_into`). The per-target convention is that each `sys/<target>/`
//! backend is cfg-selected from `sys/mod.rs` and exposes an identical floor
//! signature; only the asm and number tables differ across backends.

pub mod raw;

// The wrapper-level floor this backend owes `sys/mod.rs` — satisfied by the
// shared Linux-family `wrap.rs` (freestanding backends define their own).
pub use super::wrap::{
    AT_FDCWD, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES, CLONE_FS,
    CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM, FUTEX_PRIVATE_FLAG,
    FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, O_CLOEXEC, O_CREAT, O_RDONLY,
    O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ, PROT_WRITE, Timespec, clock_gettime, close, exit,
    exit_group, futex, gettid, mmap, mprotect, munmap, openat, read, write,
};

use core::arch::asm;

use super::errno::{Errno, from_ret};

/// Issues `clone` and, in the child, branches straight to `entry` on the new
/// stack — all in one `asm!` block so the child never executes a Rust
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
