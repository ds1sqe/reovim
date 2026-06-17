//! Detached thread spawning over the kabi platform handle's `thread_spawn`
//! primitive.
//!
//! [`spawn`] runs a `Send + 'static` Rust closure on a fresh thread reached
//! through the boot-installed `kabi` handle — never through `arch` directly.
//! The handle's `thread_spawn` slot is a pthread_create-shaped C-ABI primitive
//! (`entry: extern "C" fn(*mut u8)`, `arg: *mut u8`); this module bridges a Rust
//! closure to it by boxing the closure into a handle-allocated block, passing
//! the block pointer as `arg`, and supplying a MONOMORPHIZED `extern "C"`
//! trampoline as `entry` that reconstructs and runs the closure (the same
//! closure-FFI bridge `std`'s thread spawn uses, sized for the floor).
//!
//! ## Detached (no join), by design
//!
//! The threads are detached: there is no `JoinHandle`. The only floor consumer
//! (the server accept loop + per-connection threads) never joins — those
//! threads run until the client disconnects or the process exits. A joinable
//! handle (with teardown ownership) is deferred until a real joiner appears
//! (rule of three). This matches the prior `arch::thread` behaviour where the
//! server dropped every `JoinHandle` un-joined.
//!
//! ## Bootstrap prerequisite
//!
//! [`spawn`] reads the `kabi` handle (for both the closure-box allocation and
//! the `thread_spawn` slot), so it must run after the boot path installs the
//! handle — the same no-DS-before-install prerequisite as every other `lib/ds`
//! op.

use core::alloc::Layout;

use {reovim_kabi_platform::handle, reovim_uapi_posix::Errno};

/// Why a [`spawn`] failed.
///
/// ```rust
/// use reovim_lib_ds::thread::SpawnError;
///
/// assert_eq!(SpawnError::OutOfMemory, SpawnError::OutOfMemory);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The closure box could not be allocated through the handle.
    OutOfMemory,
    /// The handle's `thread_spawn` primitive refused (the underlying `clone`
    /// failed, or a stack could not be mapped), carrying the platform errno.
    Spawn(Errno),
}

/// The monomorphized child entry: reconstructs the boxed `F`, runs it, and frees
/// the box. `extern "C"` so it matches the handle slot's `entry` ABI.
///
/// # Safety
///
/// `arg` must be the pointer to a `handle().alloc`-ed `F` that [`spawn`] wrote
/// and handed ownership of to the new thread; this trampoline is the unique
/// consumer of that box.
extern "C" fn trampoline<F: FnOnce()>(arg: *mut u8) {
    let boxed = arg.cast::<F>();
    // SAFETY: `arg` is the closure handle `spawn` produced — a heap box for a
    // non-ZST `F`, or a dangling-aligned pointer for a ZST. `read` is the unique
    // live access (the spawning thread does not touch it after spawn returns);
    // for a ZST it reads no real memory.
    let f = unsafe { core::ptr::read(boxed) };
    // Free the box ONLY when `F` was heap-boxed (non-ZST). A ZST closure was
    // passed via a dangling pointer with no backing allocation, so there is
    // nothing to free — calling `dealloc` on it would be unsound.
    if core::mem::size_of::<F>() != 0 {
        // SAFETY: for a non-ZST `F`, `boxed` is the live heap box `spawn`
        // allocated; the contents were just moved out, so freeing it here is the
        // sole teardown of that allocation.
        unsafe {
            handle()
                .dealloc(core::ptr::NonNull::new_unchecked(boxed.cast::<u8>()), Layout::new::<F>());
        }
    }
    f();
}

/// Returns the calling thread's kernel thread id through the kabi handle.
///
/// Reaches the handle's `thread_id` slot — never `arch` directly. The id
/// cannot fail; a valid tid is a small positive integer (widened to `i64` to
/// match the slot's scalar return).
///
/// ```no_run
/// // no_run: reading the thread id needs a booted `kabi` handle.
/// use reovim_lib_ds::thread::current_id;
///
/// let _tid = current_id();
/// ```
#[must_use]
pub fn current_id() -> i64 {
    i64::from(handle().thread_id())
}

/// Spawns a detached thread running `f` through the kabi handle.
///
/// `f` is boxed into a handle-allocated block and run on the new thread via a
/// monomorphized `extern "C"` trampoline. There is no join: the thread runs to
/// completion and is reclaimed by the OS at thread/process exit.
///
/// # Errors
///
/// Returns [`SpawnError::OutOfMemory`] when the closure box cannot be allocated,
/// or [`SpawnError::Spawn`] when the handle's `thread_spawn` primitive refuses.
///
/// ```no_run
/// // no_run: spawning needs a booted `kabi` handle (the boot path installs it).
/// use reovim_lib_ds::thread::spawn;
///
/// spawn(|| {
///     // background work on a detached thread
/// })
/// .expect("spawn succeeds on a booted handle");
/// ```
pub fn spawn<F>(f: F) -> Result<(), SpawnError>
where
    F: FnOnce() + Send + 'static,
{
    // A zero-sized closure needs no heap: pass a dangling-but-aligned pointer as
    // `arg` and let the trampoline reconstruct the ZST from it without a read of
    // real memory. The handle's allocator contract is `size > 0`, so allocating
    // a ZST would violate it; the dangling pointer is the canonical ZST handle.
    if core::mem::size_of::<F>() == 0 {
        // The closure is zero-sized; it carries no captured state to move.
        // `NonNull::<F>::dangling()` is a valid, aligned pointer for a ZST read.
        let arg = core::ptr::NonNull::<F>::dangling().as_ptr().cast::<u8>();
        // SAFETY: `trampoline::<F>` reads a ZST `F` from `arg` (no real memory is
        // dereferenced for a ZST) and runs it; for a ZST it frees nothing. `f`
        // is dropped here as it is not moved into a box, BUT a ZST `FnOnce` has
        // no state — re-creating it in the trampoline via the ZST read is sound.
        let _ = f; // a ZST has no Drop side effect to preserve across the spawn
        let result = unsafe { handle().thread_spawn(trampoline::<F>, arg) };
        return result.map(|_| ()).map_err(SpawnError::Spawn);
    }

    // Box the closure through the handle's allocator.
    let layout = Layout::new::<F>();
    let boxed = handle()
        .alloc(layout)
        .map_err(|_| SpawnError::OutOfMemory)?;
    let boxed = boxed.cast::<F>();
    // SAFETY: `boxed` is a fresh, correctly-sized, aligned allocation for `F`;
    // the write initializes it. Ownership of the box transfers to the new thread
    // (the trampoline frees it); on a spawn error below we free it ourselves.
    unsafe {
        core::ptr::write(boxed.as_ptr(), f);
    }

    // SAFETY: `trampoline::<F>` is a valid `extern "C"` entry that consumes
    // exactly the box at `arg`; the box outlives the spawn (it is freed by the
    // trampoline on the new thread). Ownership of `arg` transfers to that thread.
    let result = unsafe { handle().thread_spawn(trampoline::<F>, boxed.as_ptr().cast::<u8>()) };
    match result {
        Ok(_tid) => Ok(()),
        Err(e) => {
            // The thread never started: reclaim the box we wrote. The closure
            // `F` is dropped in place, then the allocation is freed.
            // SAFETY: `boxed` is the live box; the thread did not start, so this
            // is the unique owner — drop the closure and free the block.
            unsafe {
                core::ptr::drop_in_place(boxed.as_ptr());
                handle().dealloc(boxed.cast::<u8>(), layout);
            }
            Err(SpawnError::Spawn(e))
        }
    }
}
