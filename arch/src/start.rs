//! Process entry: the arch-owned `_start` and the `#![no_main]` entry seam.
//!
//! Under DAG6 (1.2 §10) product binaries are `#![no_main]` with an
//! arch-owned `_start`; there is no C runtime, no `crt0`, no libc. This
//! module provides the naked `_start` symbol the kernel jumps to, an
//! argc/argv/envp unpacker, the exit shim that emits coverage profraw under
//! instrumentation, and the [`entry!`] macro a bin crate uses to declare its
//! Rust entry point.
//!
//! Gated on the `runtime` feature: defining `_start` (a global symbol) in
//! arch's pre-migration libtest builds (bootstrap state 1) would clash with
//! the std runtime's own entry. Fixture bins and the Phase 5 no_std test
//! runner enable `runtime`; libtest builds do not (see `arch/Cargo.toml`).

/// The process entry point the kernel transfers control to (`_start`).
///
/// At entry the stack top holds the System V `x86_64` process-startup layout:
///
/// ```text
/// [rsp]      argc           (a word)
/// [rsp+8]    argv[0]        ... argv[argc-1]
/// [rsp+...]  NULL           (argv terminator)
/// [rsp+...]  envp[0]        ... (NULL-terminated)
/// ```
///
/// `rsp` is 16-byte aligned at entry per the ABI, and there is no return
/// address (the kernel jumps here; `_start` never returns to a caller). The
/// naked body loads `argc`/`argv`/`envp` into the System V argument registers and
/// tail-calls [`rust_entry`], which never returns (it exits the process).
///
/// This is the raw process entry: it is reached only via the kernel's initial
/// control transfer with the documented stack layout, never as a callable Rust
/// function. The `#[unsafe(naked)]` body upholds the System V startup contract.
///
/// ```ignore
/// // _start requires a no_main binary context — not callable from the doctest harness.
/// // It is entered only via the kernel's initial control transfer after exec.
/// ```
#[cfg(all(feature = "runtime", target_arch = "x86_64"))]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // SAFETY: `naked_asm` is the whole function body (a naked fn has no
    // prologue/epilogue). The instructions read the SysV startup stack the
    // kernel set up: argc at [rsp], argv at rsp+8, envp after the argv NULL
    // terminator. They are loaded into rdi/rsi/rdx (the first three SysV
    // argument registers) and control tail-jumps to `rust_entry`, which never
    // returns. `and rsp, -16` re-establishes 16-byte alignment before the
    // call (the ABI requires `rsp % 16 == 0` at a `call` site's callee entry,
    // i.e. rsp ≡ 8 (mod 16) after the implicit return-address push — the
    // `call` instruction handles that push). `xor rbp, rbp` marks the
    // outermost frame for unwinders/debuggers (the ABI's outermost-frame
    // convention), even though DAG6 has no unwinder.
    core::arch::naked_asm!(
        "xor rbp, rbp",        // outermost stack frame marker (SysV)
        "mov rdi, [rsp]",      // rdi = argc
        "lea rsi, [rsp + 8]",  // rsi = &argv[0]
        // envp = argv + (argc+1)*8: skip argc words plus the NULL terminator.
        "lea rdx, [rsi + rdi*8 + 8]", // rdx = &envp[0]
        "and rsp, -16",        // 16-byte align the stack before the call
        "call {entry}",        // rust_entry(argc, argv, envp) -> ! (never returns)
        "ud2",                // unreachable: rust_entry exits the process
        entry = sym rust_entry,
    )
}

/// The aarch64 process entry point the kernel transfers control to (`_start`).
///
/// At entry the stack top holds the aarch64 Linux ELF process-startup layout,
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
/// function. The `#[unsafe(naked)]` body upholds the aarch64 startup contract.
///
/// ```ignore
/// // _start requires a no_main binary context — not callable from the doctest harness.
/// // It is entered only via the kernel's initial control transfer after exec.
/// ```
#[cfg(all(feature = "runtime", target_arch = "aarch64"))]
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
/// (`argc` in rdi, `argv` in rsi, `envp` in rdx — the System V first three).
///
/// The main shim is the `arch_main` symbol the [`entry!`] macro installs. A
/// bin crate that links arch with `runtime` but never invokes `entry!` would
/// have no shim; that is a link error by construction (the symbol is
/// undefined), which is the intended "you must declare an entry" contract.
#[cfg(feature = "runtime")]
extern "C" fn rust_entry(argc: usize, argv: *const *const u8, envp: *const *const u8) -> ! {
    // Stash envp before running main so the coverage exit path can read
    // `LLVM_PROFILE_FILE` from it. The cast drops only the outer const (the
    // atomic stores `*mut`); the pointer is never written through.
    ENVP.store(envp.cast_mut(), core::sync::atomic::Ordering::Release);
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
/// lifetime). Returns at most [`ENV_MAX`] entries — a kernel env block is
/// bounded by `ARG_MAX`, and capping keeps the scratch array stack-sized.
///
/// ```ignore
/// // env_block requires the runtime feature and a live _start-initialized ENVP.
/// // Before _start runs (i.e. in the doctest harness) it returns an empty slice.
/// use reovim_arch::start::env_block;
/// let env = env_block();
/// // In a real runtime binary: env contains the process environment.
/// ```
#[cfg(feature = "runtime")]
#[must_use]
pub fn env_block() -> &'static [&'static [u8]] {
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
/// A `#![no_main]` no_std binary has no C-runtime `atexit` hook, so the LLVM
/// instrumentation profile is never written automatically (std binaries get
/// it via the runtime's atexit registration). When built under coverage
/// instrumentation this shim calls `__llvm_profile_write_file` — the
/// toolchain-provided profile writer (L9-clean: shipped with the compiler,
/// not a third-party crate) — before `exit_group`, so an exec'd instrumented
/// fixture emits the profraw the coverage merge consumes (Phase 5).
///
/// The call is gated on the `arch_coverage` cfg, which the coverage script
/// sets (`RUSTFLAGS=--cfg arch_coverage`) alongside `-C instrument-coverage`.
/// An ordinary uninstrumented build does not set the cfg, so the
/// `__llvm_profile_write_file` symbol is never referenced and the binary
/// links without the instrumentation runtime.
///
/// ```ignore
/// // exit_process terminates the process — not safe to run in the doctest harness.
/// reovim_arch::start::exit_process(0);
/// ```
#[cfg(feature = "runtime")]
pub fn exit_process(code: i32) -> ! {
    #[cfg(arch_coverage)]
    {
        // SAFETY: `__llvm_profile_write_file` is arch's own profiler runtime
        // (`crate::profiler`), compiled under the same `arch_coverage` cfg, so
        // the `__llvm_prf_*` sections it walks are present. It writes the
        // in-memory counters to the path named by `LLVM_PROFILE_FILE` and
        // returns an ignored status. Calling it once immediately before exit is
        // the no_std analog of the std atexit hook.
        unsafe {
            crate::profiler::__llvm_profile_write_file();
        }
    }
    crate::sys::exit_group(code)
}

// L12 layout (#785 Phase 5 coverage): tests for start.rs live in the sibling
// file `start_tests.rs`, declared here as a `#[path]` child so `super::`
// reaches the private `ENVP` and `env_block`. Gated on both `selftest` and
// `runtime` because `env_block` and `ENVP` are `#[cfg(feature = "runtime")]`.
#[cfg(all(feature = "selftest", feature = "runtime"))]
#[path = "start_tests.rs"]
mod tests;

/// Declares a bin crate's process entry point under the arch `_start`.
///
/// A `#![no_std] #![no_main]` binary has no `fn main`; this macro installs the
/// `extern "C"` shim symbol [`_start`]'s `rust_entry` tail-calls. The body
/// receives `argc: usize`, `argv: *const *const u8`, `envp: *const *const u8`
/// and returns the process exit code (`i32`).
///
/// ```ignore
/// #![no_std]
/// #![no_main]
/// reovim_arch::entry!(|_argc, _argv, _envp| {
///     // ... boot, do work ...
///     0
/// });
/// ```
///
/// The closure form keeps the entry self-contained; the macro names the
/// symbol `reovim_arch_main` (the link target of arch's `arch_main` extern
/// declaration), so exactly one `entry!` per bin is permitted (a second is a
/// duplicate-symbol link error — the intended single-entry contract).
#[macro_export]
macro_rules! entry {
    ($body:expr) => {
        /// The arch entry shim, tail-called by the platform-floor `_start` via
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
