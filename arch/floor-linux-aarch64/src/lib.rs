//! `reovim-arch-floor-linux-aarch64` — the `aarch64`-linux language floor.
//!
//! This `#![no_std]` crate owns the per-binary **language-knowledge** symbols
//! for `aarch64-linux` (OS-Modes §2.2, master-plan invariant #1 — the LINK, not
//! import, floor↔product crossing): the naked `_start` process entry, the
//! `#[panic_handler]` lang item and its allocator-free render/flush mechanism,
//! `rust_eh_personality`, the freestanding `mem` intrinsics the compiler lowers
//! to under `-nodefaultlibs`, and the [`entry!`] macro / `reovim_arch_main`
//! link seam a bin crate declares its Rust entry through. These were relocated
//! out of `reovim-arch` in SP03; `arch` no longer defines any of them.
//!
//! It reaches the OS only through its matching `reovim-arch-sys-linux-aarch64`
//! dep (syscalls / clock) and the `kabi/panic` registry it reads on a panic;
//! it never names `arch` (that would close the floor→product airlock —
//! master-plan invariant #8). `unsafe` is allowed here because, like `arch`,
//! the language floor owns naked asm and raw-pointer memory management that
//! cannot be expressed in safe Rust.
#![no_std]
// SAFETY (lint): the language floor owns the naked `_start` asm, the
// freestanding `#[no_mangle]` mem intrinsics, and raw-pointer env/panic
// rendering — all of which require unsafe. The workspace lint stays `warn` so
// every crate above the floor still flags unsafe; the allow is scoped here.
#![allow(unsafe_code)]

// The arch-sys + uapi/panic imports are referenced only by the `runtime`-gated
// lang items (the `_start` asm, the panic handler, `exit_process`), so the
// imports themselves are `runtime`-gated too — a plain (non-runtime) rlib build
// compiles an empty floor and must not carry an unused import under
// `warnings = deny`.
#[cfg(feature = "runtime")]
use reovim_arch_sys_linux_aarch64 as sys;

/// The disposition + record types are owned up-face by `uapi/panic`; the panic
/// handler and `rust_eh_personality` name them through the floor's dep.
#[cfg(feature = "runtime")]
use reovim_uapi_panic::{Disposition, PanicRecord};

// The LLVM coverage profiler runtime (#785 Phase 5, relocated with the panic
// handler in SP03). It defines `__llvm_profile_runtime` + `__llvm_profile_write_file`
// and is same-crate to both `exit_process` and the `#[panic_handler]`, so their
// `cfg(arch_coverage)` writer calls resolve directly (Q5). Compiled only under
// coverage instrumentation (`arch_coverage`), where the `__llvm_prf_*` sections
// exist; it reads the captured env block (`env_block`), so it also needs the
// `runtime` boot path. An ordinary build compiles none of it.
#[cfg(all(feature = "runtime", arch_coverage))]
mod profiler;

// ── _start (aarch64-linux) ─────────────────────────────────────────────────────

/// The `aarch64` process entry point the kernel transfers control to (`_start`).
///
/// At entry the stack top holds the `aarch64` Linux ELF process-startup layout,
/// the same shape as the SysV `x86_64` layout:
///
/// ```text
/// [sp]       argc           (a word)
/// [sp+8]     argv[0]        ... argv[argc-1]
/// [sp+...]   NULL           (argv terminator)
/// [sp+...]   envp[0]        ... (NULL-terminated)
/// ```
///
/// `sp` is 16-byte aligned at entry per the ABI, and there is no return
/// address (the kernel jumps here; `_start` never returns to a caller). The
/// naked body loads `argc`/`argv`/`envp` into the AAPCS64 argument registers
/// (`x0`/`x1`/`x2`) and tail-calls [`rust_entry`], which never returns (it
/// exits the process).
///
/// This is the raw process entry: it is reached only via the kernel's initial
/// control transfer with the documented stack layout, never as a callable Rust
/// function. The `#[unsafe(naked)]` body upholds the `aarch64` startup contract.
///
/// ```ignore
/// // _start requires a no_main binary context — not callable from the doctest harness.
/// // It is entered only via the kernel's initial control transfer after exec.
/// ```
#[cfg(feature = "runtime")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // SAFETY: `naked_asm` is the whole function body (a naked fn has no
    // prologue/epilogue). The instructions read the aarch64 startup stack the
    // kernel set up: argc at [sp], argv at sp+8, envp after the argv NULL
    // terminator. They are loaded into x0/x1/x2 (the first three AAPCS64
    // argument registers) and control tail-jumps to `rust_entry`, which never
    // returns. No stack re-alignment is needed: the kernel guarantees `sp`
    // 16-byte aligned at entry, and `bl` (unlike `x86_64`'s `call`) pushes
    // nothing — it writes the return address to the link register `x30` — so
    // the alignment is preserved through the call site. `mov x29, xzr` marks
    // the outermost frame for unwinders/debuggers (the ABI's outermost-frame
    // convention), even though DAG6 has no unwinder.
    core::arch::naked_asm!(
        "mov x29, xzr",        // outermost stack frame marker (frame pointer)
        "ldr x0, [sp]",        // x0 = argc
        "add x1, sp, #8",      // x1 = &argv[0]
        // envp = argv + (argc+1)*8: skip argc words plus the NULL terminator.
        "add x2, x0, #1",      // x2 = argc + 1
        "add x2, x1, x2, lsl #3", // x2 = &argv[0] + (argc+1)*8 = &envp[0]
        "bl {entry}",          // rust_entry(argc, argv, envp) -> ! (never returns)
        "brk #1",              // unreachable: rust_entry exits the process
        entry = sym rust_entry,
    )
}

// ── rust_entry / ENVP / env_block / the arch_main seam / exit_process ──────────

/// The captured process environment vector (`envp`), stashed by [`rust_entry`]
/// at startup so the exit path can read it without re-threading it through
/// every call frame. `null` before entry runs.
///
/// The pointer aliases the kernel-provided startup stack, which lives for the
/// whole process, so the capture stays valid until exit. A single write at
/// the start of `rust_entry` (before any thread spawns) and reads only at
/// exit; no concurrent access, but the atomic keeps the access well-defined.
#[cfg(feature = "runtime")]
static ENVP: core::sync::atomic::AtomicPtr<*const u8> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

/// The Rust side of process entry: runs the registered main shim and exits
/// the process with its return code through [`exit_process`].
///
/// `extern "C"` so its calling convention matches the `_start` tail-call
/// (`argc` in x0, `argv` in x1, `envp` in x2 — the AAPCS64 first three).
///
/// The main shim is the `arch_main` symbol the [`entry!`] macro installs. A
/// bin crate that links this floor with `runtime` but never invokes `entry!`
/// would have no shim; that is a link error by construction (the symbol is
/// undefined), which is the intended "you must declare an entry" contract.
#[cfg(feature = "runtime")]
// argc/argv/envp are the canonical C entry-point parameter names; renaming
// them to satisfy the lint would obscure the well-known convention.
#[allow(clippy::similar_names)]
extern "C" fn rust_entry(argc: usize, argv: *const *const u8, envp: *const *const u8) -> ! {
    // Stash envp before running main so the coverage exit path can read
    // `LLVM_PROFILE_FILE` from it. The cast drops only the outer const (the
    // atomic stores `*mut`); the pointer is never written through.
    ENVP.store(envp.cast_mut(), core::sync::atomic::Ordering::Release);
    // The platform-vtable install is owned by the `apps/*`/fixture composition
    // root (SP02): it drives `install_platform()` as the first statement of its
    // `entry!` closure, before any `kabi::handle` read. `rust_entry` therefore
    // just forwards the startup vectors to `arch_main`.
    // SAFETY: `arch_main` is the bin crate's `entry!`-installed shim with this
    // exact signature; `argc`/`argv`/`envp` are the kernel-provided startup
    // vectors forwarded verbatim from `_start`. The shim owns interpreting them.
    let code = unsafe { arch_main(argc, argv, envp) };
    exit_process(code)
}

/// The captured process environment as a slice of NUL-terminated `KEY=VALUE`
/// byte strings (each including its terminating NUL), or empty before entry
/// runs.
///
/// Walks the captured `envp` (a NULL-terminated array of C strings) and, for
/// each entry, scans to its NUL to recover the byte length. Used by the
/// coverage profiler runtime to read `LLVM_PROFILE_FILE`; kept minimal (no
/// parsing) per the floor's mechanism-not-policy posture.
///
/// The returned slices borrow the live startup stack (valid for the process
/// lifetime). Returns at most `ENV_MAX` entries — a kernel env block is
/// bounded by `ARG_MAX`, and capping keeps the scratch array stack-sized.
#[cfg(all(feature = "runtime", arch_coverage))]
#[must_use]
pub(crate) fn env_block() -> &'static [&'static [u8]] {
    use core::sync::atomic::Ordering::Acquire;

    /// Upper bound on env entries collected (the scratch array size). A real
    /// env block is far smaller; entries past this are ignored.
    const ENV_MAX: usize = 4096;

    // A process-static scratch table the returned slice borrows. Filled once
    // per call from the immutable env block; reads at exit are single-threaded
    // (the coverage write happens just before `exit_group`). Accessed only
    // through raw pointers (`&raw mut`) to avoid the `static_mut_refs` lint.
    static mut TABLE: [&[u8]; ENV_MAX] = [b""; ENV_MAX];

    let envp: *const *const u8 = ENVP.load(Acquire).cast_const();
    if envp.is_null() {
        return &[];
    }

    let table: *mut [&[u8]] = &raw mut TABLE;
    let table_base: *mut &[u8] = table.cast::<&[u8]>();

    let mut n = 0;
    while n < ENV_MAX {
        // SAFETY: `envp` is the kernel-provided NULL-terminated environment
        // array; indexing up to the first NULL stays in bounds by the ABI.
        let entry = unsafe { *envp.add(n) };
        if entry.is_null() {
            break;
        }
        // Scan the C string to its NUL to recover the byte length, then build
        // a slice that includes the terminating NUL (callers want a C string).
        let mut len = 0;
        // SAFETY: `entry` is a NUL-terminated C string from the kernel env
        // block; reading bytes up to and including the NUL stays in bounds.
        while unsafe { *entry.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `[entry, entry + len]` are the string's bytes plus its NUL,
        // all live for the process lifetime; the slice is read-only.
        let bytes = unsafe { core::slice::from_raw_parts(entry, len + 1) };
        // SAFETY: `table_base.add(n)` with `n < ENV_MAX` is in bounds of
        // `TABLE`; this function is the only writer and the coverage caller is
        // single-threaded, so the raw store has no data race.
        unsafe {
            *table_base.add(n) = bytes;
        }
        n += 1;
    }
    // SAFETY: `TABLE[..n]` was just initialized with live env slices; the
    // returned shared slice borrows the process-static table, valid `'static`.
    unsafe { core::slice::from_raw_parts(table_base.cast_const(), n) }
}

// The bin-crate entry shim installed by `entry!`. Declared here as an
// `extern "C"` symbol `rust_entry` resolves at link time; the macro defines
// it in the bin crate. (Plain comment: rustdoc generates nothing for extern
// blocks, so a doc comment here is rejected by `unused_doc_comments`.)
#[cfg(feature = "runtime")]
unsafe extern "C" {
    /// `fn(argc, argv, envp) -> i32` — the bin's main, returning its exit
    /// code. Defined by [`entry!`]; undefined (link error) if a `runtime` bin
    /// declares no entry.
    #[link_name = "reovim_arch_main"]
    fn arch_main(argc: usize, argv: *const *const u8, envp: *const *const u8) -> i32;
}

/// Exits the process with `code`, emitting coverage profraw first under
/// instrumentation.
///
/// A `#![no_main]` `no_std` binary has no C-runtime `atexit` hook, so the LLVM
/// instrumentation profile is never written automatically (std binaries get
/// it via the runtime's atexit registration). When built under coverage
/// instrumentation this shim calls `__llvm_profile_write_file` — the
/// toolchain-provided profile writer (L9-clean: shipped with the compiler,
/// not a third-party crate) — before `exit_group`, so an exec'd instrumented
/// fixture emits the profraw the coverage merge consumes.
///
/// The call is gated on the `arch_coverage` cfg, which the coverage script
/// sets (`RUSTFLAGS=--cfg arch_coverage`) alongside `-C instrument-coverage`.
/// An ordinary uninstrumented build does not set the cfg, so the
/// `__llvm_profile_write_file` symbol is never referenced and the binary
/// links without the instrumentation runtime.
///
/// ```ignore
/// // exit_process terminates the process — not safe to run in the doctest harness.
/// ```
#[cfg(feature = "runtime")]
pub fn exit_process(code: i32) -> ! {
    #[cfg(arch_coverage)]
    {
        // SAFETY: `__llvm_profile_write_file` is this floor's own profiler
        // runtime (`crate::profiler`), compiled under the same `arch_coverage`
        // cfg, so the `__llvm_prf_*` sections it walks are present. It writes the
        // in-memory counters to the path named by `LLVM_PROFILE_FILE` and
        // returns an ignored status. Calling it once immediately before exit is
        // the no_std analog of the std atexit hook.
        unsafe {
            profiler::__llvm_profile_write_file();
        }
    }
    sys::exit_group(code)
}

/// Declares a bin crate's process entry point under the floor `_start`.
///
/// A `#![no_std] #![no_main]` binary has no `fn main`; this macro installs the
/// `extern "C"` shim symbol [`_start`]'s `rust_entry` tail-calls. The body
/// receives `argc: usize`, `argv: *const *const u8`, `envp: *const *const u8`
/// and returns the process exit code (`i32`).
///
/// ```ignore
/// #![no_std]
/// #![no_main]
/// reovim_arch_floor_linux_aarch64::entry!(|_argc, _argv, _envp| {
///     // ... boot, do work ...
///     0
/// });
/// ```
///
/// The closure form keeps the entry self-contained; the macro names the
/// symbol `reovim_arch_main` (the link target of the floor's `arch_main` extern
/// declaration), so exactly one `entry!` per bin is permitted (a second is a
/// duplicate-symbol link error — the intended single-entry contract).
#[macro_export]
macro_rules! entry {
    ($body:expr) => {
        /// The arch entry shim, tail-called by the language-floor `_start` via
        /// the `reovim_arch_main` link name with the process startup vectors.
        #[unsafe(no_mangle)]
        pub extern "C" fn reovim_arch_main(
            argc: usize,
            argv: *const *const u8,
            envp: *const *const u8,
        ) -> i32 {
            // Bind the body as a typed closure so its signature is checked.
            let f: fn(usize, *const *const u8, *const *const u8) -> i32 = $body;
            f(argc, argv, envp)
        }
    };
}

// ── mem intrinsics ────────────────────────────────────────────────────────────
//
// A `#![no_std] #![no_main]` binary links no libc, but the compiler still
// lowers core operations (slice fills, `core::fmt` buffer writes, struct
// copies) to calls on these five C symbols. Someone must define them; under
// DAG6 the language floor does. Gated on `runtime` so a libtest build (where
// libc already provides them) does not see a duplicate strong definition.
//
// ## Why volatile byte loops
//
// LLVM's loop-idiom pass recognizes a plain byte-fill/byte-copy loop and
// rewrites it into a call to `memset`/`memcpy` — inside the very function
// defining that symbol, producing infinite recursion. Volatile accesses are
// exempt from idiom recognition, so each function below is immune by
// construction. Byte-at-a-time volatile is the slow-but-right floor; a
// `rep stosb`/`rep movsb` fast path is added when a real consumer profiles a
// need (rule of three), not speculatively.

/// Fills `n` bytes at `dest` with the byte value `c`. Returns `dest`.
///
/// # Safety
///
/// `dest` must be valid for `n` writes. The caller is the compiler's
/// lowering machinery (or equivalent), which guarantees exclusive access to
/// the range for the duration of the call.
#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    // The C contract passes the fill byte as an `int`; only the low 8 bits
    // are the value.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = c as u8;
    let mut i = 0;
    while i < n {
        // SAFETY: the caller guarantees `dest..dest+n` is valid for writes;
        // `i < n` keeps the access in bounds. Volatile defeats loop-idiom
        // recognition (module doc).
        unsafe { dest.add(i).write_volatile(byte) };
        i += 1;
    }
    dest
}

/// Copies `n` bytes from `src` to `dest`; the ranges must not overlap.
/// Returns `dest`.
///
/// # Safety
///
/// `src` must be valid for `n` reads, `dest` for `n` writes, and the ranges
/// must be disjoint (the C `memcpy` contract).
#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        // SAFETY: the caller guarantees both ranges are valid for `n`
        // accesses and disjoint; `i < n` keeps both in bounds. Volatile
        // defeats loop-idiom recognition (module doc).
        unsafe { dest.add(i).write_volatile(src.add(i).read_volatile()) };
        i += 1;
    }
    dest
}

/// Copies `n` bytes from `src` to `dest`, handling overlap. Returns `dest`.
///
/// # Safety
///
/// `src` must be valid for `n` reads and `dest` for `n` writes (the C
/// `memmove` contract; overlap is permitted).
#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        // Forward copy: dest is below src, so reading ahead of the write
        // cursor never reads an already-overwritten byte.
        let mut i = 0;
        while i < n {
            // SAFETY: caller guarantees validity for `n` accesses; `i < n`
            // bounds both. Volatile defeats loop-idiom recognition.
            unsafe { dest.add(i).write_volatile(src.add(i).read_volatile()) };
            i += 1;
        }
    } else {
        // Backward copy: dest is at/above src, so copying from the end
        // never reads an already-overwritten byte.
        let mut i = n;
        while i > 0 {
            i -= 1;
            // SAFETY: caller guarantees validity for `n` accesses; `i < n`
            // bounds both. Volatile defeats loop-idiom recognition.
            unsafe { dest.add(i).write_volatile(src.add(i).read_volatile()) };
        }
    }
    dest
}

/// Lexicographically compares `n` bytes: `<0`, `0`, `>0` as `a` is below,
/// equal to, or above `b` at the first differing byte.
///
/// # Safety
///
/// `a` and `b` must each be valid for `n` reads.
#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
#[allow(clippy::many_single_char_names)] // a/b/n: the canonical libc memcmp names
pub(crate) unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        // SAFETY: caller guarantees both ranges are valid for `n` reads;
        // `i < n` bounds both. Volatile defeats idiom recognition.
        let (x, y) = unsafe { (a.add(i).read_volatile(), b.add(i).read_volatile()) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
        i += 1;
    }
    0
}

/// Equality-only comparison: `0` iff the `n`-byte ranges are equal (the
/// LLVM `bcmp` lowering of pure equality checks; the sign of a non-zero
/// result is unspecified).
///
/// # Safety
///
/// `a` and `b` must each be valid for `n` reads.
#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn bcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    // SAFETY: forwarded verbatim; `memcmp`'s contract is identical.
    unsafe { memcmp(a, b, n) }
}

// ── the #[panic_handler] lang item + its render/flush mechanism ────────────────
//
// This is the AB12 panic-path *mechanism* (6.2 §5, 9.5 §9.1): the
// `#[panic_handler]` lang item, the allocator-free LOG2 line renderer
// (9.5 §2 grammar, kernel-emitter form), the final flush to the registered
// fd, and process termination per the registered disposition. The write-once
// hook *registry* lives down-face in `kabi/panic` — the always-present
// fault-floor seam (Platform-Contract §3.4) — and is read here through its
// `get_*` accessors. The product-facing `set_*`/`enter_cleanup_context`
// registration shims stay in `arch::panic`; only the lang items + the private
// mechanism they need live here.

/// Capacity of the panic line's fixed render buffer (bytes).
///
/// The panic renderer is **allocator-free** (it must work before the platform
/// handle installs — a panic can fire mid-boot), so it formats into a fixed
/// stack buffer instead of a heap `lib/ds::Bytes`. The line is `[ts] kernel
/// panic: <message> at <file>:<line>:<col>` plus an optional ` rollback=failed`
/// suffix; 512 bytes covers a realistic panic message + source location, and a
/// longer one is truncated best-effort.
#[cfg(feature = "runtime")]
const PANIC_LINE_CAP: usize = 512;

/// An allocator-free, fallible [`core::fmt::Write`] sink over a fixed stack
/// buffer — the panic line's render target.
///
/// This constructs NO handle-routed data structure, so a panic that fires
/// before the platform handle installs still renders (the no-DS-before-install
/// invariant holds on the panic path by construction). A write that would
/// overflow the buffer is truncated and reported as [`core::fmt::Error`], so
/// the panic path renders best-effort rather than recursing into a second
/// panic. The partially rendered bytes still flush.
#[cfg(feature = "runtime")]
struct StackWriter {
    /// The fixed render buffer; only the first `len` bytes are written.
    buf: [u8; PANIC_LINE_CAP],
    /// Bytes written so far (the live prefix of `buf`).
    len: usize,
}

#[cfg(feature = "runtime")]
impl StackWriter {
    /// Creates an empty writer over a zeroed fixed buffer.
    const fn new() -> Self {
        Self {
            buf: [0; PANIC_LINE_CAP],
            len: 0,
        }
    }

    /// The rendered bytes (the live prefix of the buffer).
    fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

#[cfg(feature = "runtime")]
impl core::fmt::Write for StackWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = PANIC_LINE_CAP - self.len;
        if bytes.len() > remaining {
            // Buffer full: copy what fits, then report truncation so the panic
            // path stays alive rather than re-panicking. Best-effort by design.
            self.buf[self.len..].copy_from_slice(&bytes[..remaining]);
            self.len = PANIC_LINE_CAP;
            return Err(core::fmt::Error);
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

/// Reads the current monotonic time as a raw [`sys::Timespec`] — the LOG2
/// timestamp source for the panic line.
///
/// Relocated as a small private helper (SP03): the panic handler cannot reach
/// `arch::time::monotonic` (the floor does not dep `arch`), so it reads
/// `CLOCK_MONOTONIC` directly through the floor's arch-sys dep. On the
/// (practically impossible) clock-read error it returns the zero timespec —
/// the floor has no panic budget for a clock read.
#[cfg(feature = "runtime")]
fn monotonic() -> sys::Timespec {
    let mut ts = sys::Timespec::default();
    let _ = sys::clock_gettime(sys::CLOCK_MONOTONIC, &mut ts);
    ts
}

/// Resolves the effective disposition: the registered value, or `halt` when
/// nothing is registered (6.2 §5.2 default).
#[cfg(feature = "runtime")]
fn current_disposition() -> Disposition {
    // The disposition atom is owned by kabi/panic. An unset registry resolves
    // to `halt` here — an unbooted process halts (the safe posture, 6.2 §5.2).
    reovim_kabi_panic::get_disposition().unwrap_or(Disposition::Halt)
}

/// Builds the [`PanicRecord`] for the current fault from the registry.
#[cfg(feature = "runtime")]
fn current_record() -> PanicRecord {
    PanicRecord {
        disposition: current_disposition(),
        // The AB13 cleanup-context marker is the 6th kabi/panic fault-floor atom
        // (SP03): the product-facing `arch::panic::enter_cleanup_context` setter
        // and this reader reach the SAME static through their shared kabi/panic
        // dep, so a product-raised flag is observed here. The accessor's Acquire
        // pairs with the setter's Release.
        rollback_failed: reovim_kabi_panic::get_cleanup_context(),
    }
}

/// Renders a LOG2 kernel-emitter line whose message is produced by `render_msg`.
///
/// Form: `[ts] kernel panic: <message>` with the AB13 ` rollback=failed`
/// suffix appended when the cleanup marker is set, terminated by `\n`. The
/// timestamp is the monotonic clock (the LOG2 `ts` source). Attribution is
/// omitted — the floor renders kernel-emitter form unless a hook supplies
/// attribution (deferred to a later phase).
///
/// Rendering is best-effort and **allocator-free**: it formats into the
/// caller-owned [`StackWriter`] (a fixed stack buffer), so a panic that fires
/// before the platform handle installs still renders. A buffer-overflow write
/// surfaces as `fmt::Error` and is swallowed, so the panic path never recurses
/// into a second panic; the partially rendered bytes are still flushed.
#[cfg(feature = "runtime")]
fn render_line<F>(w: &mut StackWriter, record: PanicRecord, render_msg: F)
where
    F: FnOnce(&mut StackWriter) -> core::fmt::Result,
{
    use core::fmt::Write;

    let ts = monotonic();

    // LOG2 ts: seconds right-aligned min width 5, micros zero-padded width 6.
    // Then kernel-emitter form: emitter `kernel`, bare-subsystem address
    // `panic` (no `/`, so unambiguously a kernel address — 9.5 §3).
    let secs = ts.tv_sec;
    // `tv_nsec` is in `0..1_000_000_000`, so micros fits and the cast is exact.
    #[allow(clippy::cast_sign_loss)]
    let micros = (ts.tv_nsec / 1_000) as u64;
    // A failed write leaves `w` holding whatever rendered first; the panic
    // path stays alive rather than re-panicking on buffer overflow.
    let _ = write!(w, "[{secs:>5}.{micros:06}] kernel panic: ");
    let _ = render_msg(w);
    if record.rollback_failed {
        // AB13: the cleanup-context panic carries the rollback marker.
        let _ = w.write_str(" rollback=failed");
    }
    let _ = w.write_str("\n");
}

/// Writes the panic message + location into `w` per LOG2 message rendering.
///
/// The `#[panic_handler]` feeds the live `PanicInfo`. `PanicInfo::location()`
/// always returns `Some` for Rust panics today, but the API returns `Option`
/// and the docs say "currently" — that is not a contract, so the `None` arm is
/// handled safely in [`render_location`].
#[cfg(feature = "runtime")]
fn render_panic_message(w: &mut StackWriter, info: &core::panic::PanicInfo) -> core::fmt::Result {
    use core::fmt::Write;

    // `PanicInfo::message()` is the structured payload; `Display` renders it.
    write!(w, "{}", info.message())?;
    render_location(w, info.location())
}

/// Renders the ` at file:line:col` suffix, or ` at <unknown>` when the
/// panic machinery supplied no location (not contractually impossible —
/// see [`render_panic_message`]).
#[cfg(feature = "runtime")]
fn render_location(
    w: &mut StackWriter,
    location: Option<&core::panic::Location<'_>>,
) -> core::fmt::Result {
    use core::fmt::Write;

    match location {
        Some(loc) => write!(w, " at {}:{}:{}", loc.file(), loc.line(), loc.column()),
        None => w.write_str(" at <unknown>"),
    }
}

/// Performs the final flush (9.5 §9.1): write the registered ring tail (if
/// any) then the panic line to the registered fd. With no fd registered the
/// line goes to fd 2 (stderr) and the ring tail is skipped (the provider's
/// tail is the ring's, meaningless without the sink).
#[cfg(feature = "runtime")]
fn final_flush(line: &[u8]) {
    // The flush-fd atom is owned by kabi/panic; `None` means no sink registered.
    if let Some(fd) = reovim_kabi_panic::get_flush_fd() {
        // A registered sink: write the pre-rendered ring tail verbatim ahead
        // of the panic line (9.5 §9.1 ordering). A short or failed write is
        // swallowed — at panic time there is no recovery, only best effort.
        if let Some(provider) = reovim_kabi_panic::get_ring_tail_provider() {
            write_all(fd, provider());
        }
        write_all(fd, line);
    } else {
        // Default posture (6.2 §5.2): no sink registered, render to stderr.
        write_all(2, line);
    }
}

/// Writes the whole of `buf` to `fd`, looping over short writes; gives up on
/// the first error (panic-time best effort, no retry budget).
#[cfg(feature = "runtime")]
fn write_all(fd: i32, buf: &[u8]) {
    let mut off = 0;
    while off < buf.len() {
        match sys::write(fd, &buf[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}

/// The AB12 panic sequence (6.2 §5), parameterized over the message renderer:
/// read the registry, invoke the pre-exit hook (so terminal state is restored
/// before output), render the line, fire the state hook, perform the final
/// flush, and return the disposition's exit code. The caller terminates with
/// that code.
#[cfg(feature = "runtime")]
fn handle<F>(render_msg: F) -> i32
where
    F: FnOnce(&mut StackWriter) -> core::fmt::Result,
{
    let record = current_record();
    // Invoke the pre-exit hook FIRST (gap-7, 8.2 §2): the TUI runtime registers
    // a terminal-restore function here; calling it before rendering ensures the
    // panic line is written in cooked mode rather than raw mode.
    if let Some(hook) = reovim_kabi_panic::get_pre_exit_hook() {
        hook();
    }
    // Render into a fixed stack buffer — allocator-free, so this works even
    // before the platform handle installs (a panic mid-boot still renders).
    let mut line = StackWriter::new();
    render_line(&mut line, record, render_msg);
    // Fire the state-record hook (6.2 §5 step 3) before flushing, so a hook
    // that wants to influence the tail has run; the flush is the last step.
    if let Some(hook) = reovim_kabi_panic::get_state_record_hook() {
        hook(record);
    }
    final_flush(line.as_slice());
    record.disposition.exit_code()
}

/// The AB12 panic handler (gated on `runtime`).
///
/// Defining `#[panic_handler]` unconditionally would clash with the std
/// libtest binary that hosts pre-migration tests: std already provides the
/// lang item. The `runtime` feature is the gate — the composition root opts in.
#[cfg(feature = "runtime")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let code = handle(|w| render_panic_message(w, info));
    // Under coverage instrumentation, flush the profile before the panic
    // exit too — the panic path bypasses `exit_process`, and the panic
    // fixtures' coverage would otherwise be lost with the process.
    #[cfg(arch_coverage)]
    {
        // SAFETY: the panic sequence is single-threaded and terminal; this
        // is the sole profile write on this path (the write-file contract).
        unsafe {
            profiler::__llvm_profile_write_file();
        }
    }
    sys::exit_group(code)
}

/// Satisfies the prebuilt sysroot `libcore`'s `DW.ref.rust_eh_personality`
/// reference at link time. Under DAG6 there is no unwinder (`panic =
/// "abort"` everywhere), so unwinding machinery can never invoke this; if
/// control ever arrives here the binary is corrupt — halt immediately with
/// the AB12 halt code.
#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() -> ! {
    sys::exit_group(Disposition::Halt.exit_code())
}
