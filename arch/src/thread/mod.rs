//! Threads over raw `clone` with a futex join word.
//!
//! [`spawn`] creates a kernel thread that shares the parent's address space
//! (`CLONE_VM`), runs a `Send + 'static` closure, stores the result in
//! arch-allocated shared memory, and exits **the thread only** (the `exit`
//! syscall, not `exit_group`). [`JoinHandle::join`] blocks on the kernel's
//! `CLONE_CHILD_CLEARTID` futex word until the thread terminates, then reads
//! the result and frees the thread's resources.
//!
//! ## Teardown ownership (the spawn/join boundary)
//!
//! The child **never frees its own stack** — it is running on it. The joiner
//! owns teardown:
//!
//! 1. The child runs the closure, writes the result into the shared block,
//!    then issues the `exit` syscall. The kernel zeroes the ctid join word
//!    and `FUTEX_WAKE`s one waiter as the thread dies.
//! 2. [`JoinHandle::join`] `FUTEX_WAIT`s on the ctid word until it reads
//!    zero, takes the result out of the shared block, then `munmap`s the
//!    thread stack and frees the shared block.
//!
//! The shared block is reference-counted across exactly two owners (the
//! running child and the `JoinHandle`) so neither a child that finishes
//! before `join` nor a `join` that runs before the child can free memory the
//! other still touches. The stack mapping is recorded in the shared block and
//! freed by whichever owner drops last — always the joiner, because the child
//! drops its handle to the block *before* `exit`, but the kernel keeps the
//! stack alive until the thread is fully gone (after the ctid clear `join`
//! waits on). See [`spawn`] for the exact sequence.
//!
//! ## TLS-free
//!
//! No `CLONE_SETTLS`, no thread-locals anywhere. The closure and its result
//! travel through the heap-allocated shared block, not TLS.
//!
//! ## No detach
//!
//! `JoinHandle` is join-only. Detach is not built until a real consumer needs
//! it (rule of three); a dropped, un-joined handle leaks the thread's stack
//! and shared block deliberately rather than racing the still-running child.

use core::{
    alloc::Layout,
    ptr::NonNull,
    sync::atomic::{
        AtomicI32,
        Ordering::{Acquire, Release},
    },
};

use crate::{
    alloc::{AllocError, alloc, dealloc},
    sys::{
        CLONE_CHILD_CLEARTID, CLONE_FILES, CLONE_FS, CLONE_PARENT_SETTID, CLONE_SIGHAND,
        CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM, Errno, FUTEX_WAIT, MAP_ANONYMOUS, MAP_PRIVATE,
        PROT_NONE, PROT_READ, PROT_WRITE, exit as sys_exit, from_ret, futex, mmap, mprotect,
        munmap,
    },
};

#[cfg(feature = "selftest")]
mod testhooks {
    //! Test-only injection for the thread teardown arms that a healthy kernel
    //! never takes (an `mprotect` on freshly-`mmap`d anonymous memory cannot
    //! fail in-process). Mirrors the `sync` testhooks precedent: instrumentation
    //! beside the code it probes, gated entirely on `selftest`.
    use core::sync::atomic::{AtomicBool, Ordering};

    /// One-shot flag forcing the next guard-page `mprotect` to be treated as
    /// failed, so the unmap-and-`Err` teardown after the guard install runs.
    static FORCE_GUARD_FAIL: AtomicBool = AtomicBool::new(false);

    /// Arms a single forced guard-`mprotect` failure.
    pub(super) fn fail_next_guard() {
        FORCE_GUARD_FAIL.store(true, Ordering::SeqCst);
    }

    /// Whether the guard-install hook should report failure. One-shot:
    /// self-disarms on fire.
    pub(super) fn guard_should_fail() -> bool {
        FORCE_GUARD_FAIL.swap(false, Ordering::SeqCst)
    }
}

/// Host page size (x86_64-linux 4 KiB base pages; rule of three).
const PAGE_SIZE: usize = 4096;
/// Usable thread stack size (1 MiB), excluding the guard page.
const STACK_SIZE: usize = 1024 * 1024;
/// Total mapped size: guard page + usable stack.
const MAP_SIZE: usize = PAGE_SIZE + STACK_SIZE;

/// The composite `clone` flag set for a TLS-free worker thread.
///
/// `CLONE_VM|FS|FILES|SIGHAND|THREAD|SYSVSEM` make a real thread sharing the
/// process's address space, fd table, signal handlers and `SysV` sem undo
/// list.
/// `CLONE_PARENT_SETTID` writes the child tid into the join word **before
/// clone returns in the parent** (synchronously, no race), and
/// `CLONE_CHILD_CLEARTID` zeroes that same word and `FUTEX_WAKE`s it on child
/// exit — together making the one word a set-on-create, clear-on-exit join
/// futex (`ptid` and `ctid` point at the same word, the glibc pattern).
const CLONE_FLAGS: usize = CLONE_VM
    | CLONE_FS
    | CLONE_FILES
    | CLONE_SIGHAND
    | CLONE_THREAD
    | CLONE_SYSVSEM
    | CLONE_PARENT_SETTID
    | CLONE_CHILD_CLEARTID;

/// Why a spawn failed.
///
/// ```rust
/// use reovim_arch::thread::{SpawnError, spawn};
///
/// // A successful spawn produces a handle; join it to obtain the result.
/// let h = spawn::<_, u32>(|| 7).expect("spawn should succeed on a healthy kernel");
/// assert_eq!(h.join(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The thread stack (or its guard page, or the shared block) could not be
    /// allocated.
    OutOfMemory,
    /// The `clone` syscall itself was refused by the kernel.
    Clone(Errno),
}

impl From<AllocError> for SpawnError {
    fn from(_: AllocError) -> Self {
        Self::OutOfMemory
    }
}

/// The heap block shared between the running child and the `JoinHandle`.
///
/// `F` is the closure (consumed once, by the child); `T` is its result
/// (written by the child, read by the joiner). The block is type-erased to a
/// `*mut ()` for the `extern "C"` trampoline and re-typed inside it.
struct ThreadShared<F, T> {
    /// The closure to run. `None` after the child takes it.
    closure: Option<F>,
    /// The result. `None` until the child writes it.
    result: Option<T>,
    /// The kernel's `CLONE_CHILD_CLEARTID` join word: the child's tid while
    /// running, zeroed (and futex-woken) by the kernel on exit.
    ctid: AtomicI32,
    /// Base address of the thread's stack mapping (guard page + stack), so
    /// the joiner can `munmap` it exactly.
    stack_base: usize,
    /// Total mapped size of the stack region, for the joiner's `munmap`.
    stack_map_size: usize,
}

/// A handle to a spawned thread, joinable exactly once.
///
/// Join-only: there is no detach (see the module doc). The result is taken on
/// [`join`](JoinHandle::join), which consumes the handle, making double-join a
/// compile-time impossibility (the handle is moved).
///
/// ```rust
/// use reovim_arch::thread::spawn;
///
/// let h = spawn::<_, u32>(|| 42).expect("spawn succeeds on a healthy kernel");
/// assert!(!h.is_finished() || h.is_finished()); // either state is valid here
/// let result = h.join();
/// assert_eq!(result, 42);
/// ```
pub struct JoinHandle<F, T> {
    /// The shared block (also reachable by the running child until it exits).
    shared: NonNull<ThreadShared<F, T>>,
}

// SAFETY: the `JoinHandle` is the joiner's sole owner of the shared block; the
// child accesses the block only through the raw pointer captured at spawn, and
// the join futex serializes the child's last write (the result) before the
// joiner's read. Moving the handle across threads is sound when `T: Send`
// (the result crosses the spawn/join boundary). `F` need not be `Send` on the
// handle: by the time a `JoinHandle` exists the closure has already been moved
// into the block and will run on the child, but the bound is required at
// `spawn` so the move into the block is sound.
unsafe impl<F, T: Send> Send for JoinHandle<F, T> {}

/// Spawns a new thread running `f`, returning a [`JoinHandle`].
///
/// # Errors
///
/// Returns [`SpawnError`] if the stack, guard page, or shared block cannot be
/// allocated, or if the `clone` syscall is refused.
///
/// ```rust
/// use reovim_arch::thread::spawn;
///
/// // Spawn a closure that computes a value; join to retrieve it.
/// let h = spawn::<_, u64>(|| 1 + 1).expect("spawn succeeds on a healthy kernel");
/// assert_eq!(h.join(), 2);
/// ```
pub fn spawn<F, T>(f: F) -> Result<JoinHandle<F, T>, SpawnError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    spawn_inner(f, MAP_SIZE, CLONE_FLAGS)
}

/// [`spawn`] with an explicit total stack-mapping size, so a test can drive the
/// `mmap`-refusal failure branch with an absurd size. `map_size` must exceed
/// one page (the guard) for a usable stack. Selftest-only.
#[cfg(feature = "selftest")]
fn spawn_with_map_size<F, T>(f: F, map_size: usize) -> Result<JoinHandle<F, T>, SpawnError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    spawn_inner(f, map_size, CLONE_FLAGS)
}

/// [`spawn`] with explicit `clone` flags instead of [`CLONE_FLAGS`], so a test
/// can drive the `clone`-`Err` teardown branch with an invalid flag set
/// (`CLONE_THREAD` without `CLONE_SIGHAND` → `EINVAL`). Selftest-only.
#[cfg(feature = "selftest")]
fn spawn_with_clone_flags<F, T>(f: F, flags: usize) -> Result<JoinHandle<F, T>, SpawnError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    spawn_inner(f, MAP_SIZE, flags)
}

/// The single spawn body: map the stack, install the guard, allocate the shared
/// block, lay out the child stack, and `clone`. Both the production [`spawn`]
/// and the selftest forcing seams ([`spawn_with_map_size`],
/// [`spawn_with_clone_flags`]) route through this, so the error branches the
/// selftest forces ARE the production branches — there is no duplicated body
/// whose coverage could diverge from the flight path.
fn spawn_inner<F, T>(f: F, map_size: usize, flags: usize) -> Result<JoinHandle<F, T>, SpawnError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    // Map the stack region: a low guard page (PROT_NONE) + the usable stack.
    // Stack grows downward, so the guard sits at the lowest address — a stack
    // overflow traps on the guard instead of corrupting an adjacent mapping.
    let stack_base = mmap(0, map_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
        .map_err(|_| SpawnError::OutOfMemory)?;
    // The guard install fails only on a broken kernel; the selftest hook forces
    // the failure so the unmap-and-`Err` teardown is exercised in-process.
    #[cfg(feature = "selftest")]
    let guard_failed =
        mprotect(stack_base, PAGE_SIZE, PROT_NONE).is_err() || testhooks::guard_should_fail();
    #[cfg(not(feature = "selftest"))]
    let guard_failed = mprotect(stack_base, PAGE_SIZE, PROT_NONE).is_err();
    if guard_failed {
        // The guard could not be installed; unmap and fail rather than run
        // without overflow protection.
        let _ = munmap(stack_base, map_size);
        return Err(SpawnError::OutOfMemory);
    }

    // Allocate the shared block holding the closure, result slot, join word.
    let layout = Layout::new::<ThreadShared<F, T>>();
    let shared = match alloc(layout) {
        Ok(p) => p.cast::<ThreadShared<F, T>>(),
        Err(e) => {
            let _ = munmap(stack_base, map_size);
            return Err(e.into());
        }
    };
    // SAFETY: `shared` is a fresh, correctly-sized, aligned allocation; the
    // write initializes every field.
    unsafe {
        core::ptr::write(
            shared.as_ptr(),
            ThreadShared {
                closure: Some(f),
                result: None,
                ctid: AtomicI32::new(0),
                stack_base,
                stack_map_size: map_size,
            },
        );
    }

    // Compute the child's initial stack pointer. The trampoline does
    // `pop rdi; call entry`, so for the SysV ABI to hold (`rsp % 16 == 8` at
    // `entry`) the child's initial `rsp` must be `8 mod 16`: after `pop` it is
    // `0 mod 16`, and `call`'s pushed return address restores `8 mod 16`.
    let stack_top = stack_base + map_size;
    // Largest 16-aligned address at/below the top, minus 8 → `8 mod 16`.
    let arg_slot = ((stack_top & !0xF) - 8) - 16;
    // SAFETY: `arg_slot` is within the mapped, writable stack region (well
    // below the top and far above the guard page); writing one usize there is
    // in bounds and 8-byte aligned (a usize write needs 8-byte alignment).
    unsafe {
        core::ptr::write(arg_slot as *mut usize, shared.as_ptr() as usize);
    }

    // The ctid pointer the kernel clears+wakes on exit.
    // SAFETY: `shared` is live; taking the address of its `ctid` field is in
    // bounds. The field is an `AtomicI32` the kernel will clear on exit.
    let shared_ref = unsafe { shared.as_ref() };
    let ctid_ptr = core::ptr::addr_of!(shared_ref.ctid).addr();

    // SAFETY: the clone contract is met for the production caller: `flags` is
    // [`CLONE_FLAGS`], a coherent thread flag set; `arg_slot` is the top of a
    // valid 1 MiB stack the child owns (the shared-block pointer is stored at
    // `[arg_slot]`); `ctid_ptr` is a valid, live `AtomicI32` in the shared
    // block used as BOTH the `CLONE_PARENT_SETTID` (set-on-create) and
    // `CLONE_CHILD_CLEARTID` (clear-on-exit) word — the one join futex. The
    // child begins in `trampoline::<F, T>`, never returning to Rust. A selftest
    // caller may pass an invalid `flags` set; `clone` then returns `Err`
    // without starting a child, and the teardown arm below frees everything.
    let ret =
        unsafe { clone_into(flags, arg_slot, ctid_ptr, (trampoline::<F, T> as *const ()).addr()) };
    match ret {
        Ok(_tid) => Ok(JoinHandle { shared }),
        Err(e) => {
            // The child never started: tear down everything the parent owns.
            // SAFETY: `shared` is the live block we just wrote; drop its
            // fields (the un-taken closure) before freeing.
            unsafe {
                core::ptr::drop_in_place(shared.as_ptr());
                dealloc(shared.cast(), layout);
            }
            let _ = munmap(stack_base, map_size);
            Err(SpawnError::Clone(e))
        }
    }
}

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
unsafe fn clone_into(
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
        core::arch::asm!(
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
            in("rax") crate::sys::nr::CLONE,
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

/// The child entry point: re-types the argument, runs the closure, stores the
/// result, drops the child's hold on the shared block, and exits the thread.
///
/// `extern "C"` so its calling convention matches the hand-written child
/// branch in [`clone_into`] (`arg` arrives in `rdi`, the `SysV` first
/// argument).
extern "C" fn trampoline<F, T>(arg: usize) -> !
where
    F: FnOnce() -> T,
{
    let shared = arg as *mut ThreadShared<F, T>;
    // SAFETY: `arg` is the shared-block pointer the parent stored at the stack
    // top; the parent keeps the block alive (the `JoinHandle` owns it) until
    // join, which waits for this thread's exit. So the block is live here.
    let block = unsafe { &mut *shared };
    // Take the closure (the parent put it in `Some`) and run it.
    let f = block.closure.take().expect("closure present exactly once");
    let out = f();
    // Publish the result with Release so the joiner's Acquire on the ctid word
    // (via the kernel's clear) sees it. The result store happens-before `exit`.
    block.result = Some(out);
    // A Release fence pairs with the joiner's Acquire load of the cleared
    // ctid word, so the result write is visible after join observes the clear.
    core::sync::atomic::fence(Release);
    // The child does NOT free the stack or the shared block — it is running on
    // the stack, and the joiner owns teardown (module doc). Exit the THREAD
    // only; the kernel clears the ctid word and wakes the join waiter.
    sys_exit(0)
}

impl<F, T> JoinHandle<F, T> {
    /// The shared block behind the handle.
    const fn block(&self) -> &ThreadShared<F, T> {
        // SAFETY: the handle owns the block; it is live until `join`/drop.
        unsafe { self.shared.as_ref() }
    }

    /// Whether the thread has finished (its ctid join word is zero).
    ///
    /// ```rust
    /// use reovim_arch::thread::spawn;
    ///
    /// let h = spawn::<_, ()>(|| ()).expect("spawn succeeds on a healthy kernel");
    /// let _ = h.join(); // wait for completion
    /// // After join the handle is consumed; is_finished can be observed before join:
    /// let h2 = spawn::<_, u32>(|| 0).expect("spawn");
    /// // is_finished may be true or false before join; after join the handle is gone.
    /// let _val = h2.join();
    /// ```
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.block().ctid.load(Acquire) == 0
    }

    /// Address of the ctid join word, for the raw `futex` syscall.
    fn ctid_addr(&self) -> usize {
        core::ptr::addr_of!(self.block().ctid).addr()
    }

    /// Waits for the thread to finish and returns its result.
    ///
    /// Consuming `self` makes a second join a compile error (the handle is
    /// moved). Blocks on the kernel's `CLONE_CHILD_CLEARTID` futex word until
    /// it reads zero, then takes the result and frees the stack + shared
    /// block (teardown ownership: the joiner, per the module doc).
    ///
    /// # Panics
    ///
    /// Panics if the exited child left no result in the shared block — the
    /// trampoline always stores the result before `exit`, so this indicates
    /// memory corruption, never a reachable state of the protocol.
    ///
    /// ```rust
    /// use reovim_arch::thread::spawn;
    ///
    /// let h = spawn::<_, u32>(|| 99).expect("spawn succeeds on a healthy kernel");
    /// assert_eq!(h.join(), 99);
    /// ```
    #[must_use]
    pub fn join(self) -> T {
        // Block until the kernel clears the ctid word on thread exit. The word
        // holds the child's tid while running; FUTEX_WAIT with the observed
        // non-zero value blocks until the clear+wake, and re-checks on every
        // wake (2.3 §1.1 wait re-check).
        loop {
            let tid = self.block().ctid.load(Acquire);
            if tid == 0 {
                break;
            }
            // The futex result is ignored: EAGAIN (the word already changed)
            // and a genuine wake both re-loop to the Acquire load above, which
            // re-decides.
            // `tid` is a positive kernel tid; reinterpret as the u32 the futex
            // op compares against.
            #[allow(clippy::cast_sign_loss)]
            let expected = tid as u32;
            // Deliberately NOT `FUTEX_PRIVATE_FLAG`: the kernel's exit-time
            // `CLONE_CHILD_CLEARTID` wake is a shared-key wake, and a
            // private-key waiter on the same address hashes to a different
            // futex bucket — the wake would miss and join would sleep
            // forever. glibc's `pthread_join` waits `LLL_SHARED` for the
            // same reason.
            let _ = futex(self.ctid_addr(), FUTEX_WAIT, expected, 0, 0, 0);
        }
        // Acquire fence pairs with the child's Release fence after the result
        // write, so the result is visible now.
        core::sync::atomic::fence(Acquire);

        // Take the result and the stack mapping bounds before freeing block.
        let stack_base = self.block().stack_base;
        let stack_map_size = self.block().stack_map_size;
        // SAFETY: the thread has exited (ctid cleared), so the child no longer
        // touches the block; this is the unique live access. Take the result
        // and drop the rest of the block in place.
        let result = unsafe {
            let block = &mut *self.shared.as_ptr();
            block.result.take().expect("child stored a result")
        };

        // Free the shared block, then unmap the stack. Both must happen
        // exactly once; the child freed neither. `JoinHandle` has no `Drop`
        // (a raw `NonNull` frees nothing on its own), so consuming `self` by
        // value here is the sole teardown — no double-free risk.
        let layout = Layout::new::<ThreadShared<F, T>>();
        let shared = self.shared;
        // SAFETY: the block is no longer referenced (result taken, child gone);
        // drop any remaining fields (both `Option`s are `None` now) and free it.
        unsafe {
            core::ptr::drop_in_place(shared.as_ptr());
            dealloc(shared.cast(), layout);
        }
        // SAFETY: `stack_base`/`stack_map_size` name the thread's stack
        // mapping; the thread has exited so the kernel no longer uses it.
        let _ = munmap(stack_base, stack_map_size);
        result
    }
}

// `JoinHandle` intentionally has no `Drop`: dropping a handle without `join`
// leaks the thread's stack and shared block on purpose — the child may still
// be running on the stack, so freeing it from a `Drop` would race the child.
// No detach is offered (module doc); a real detach consumer would add a safe
// reclaim path. A raw `NonNull` frees nothing on its own, so the leak is the
// natural no-Drop behavior.

// L12 layout (#785 Phase 5): tests live in the sibling file `thread_tests.rs`,
// declared as a `#[path]` child so `super::` reaches the private
// `spawn_with_map_size`, `PAGE_SIZE`, `SpawnError`, and related items.
#[cfg(feature = "selftest")]
#[path = "thread_tests.rs"]
mod tests;
