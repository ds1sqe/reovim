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
//! the std runtime's own entry. Fixture bins and the Phase 5 `no_std` test
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
#[cfg(all(feature = "runtime", target_arch = "aarch64", target_os = "linux"))]
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

/// The level-1 translation table for the bare-metal identity map, filled by
/// the `_start` asm before the MMU is enabled (Rust never touches it).
///
/// Four 1 GiB block entries cover the 4 GiB physical window the image uses
/// (T0SZ = 32): blocks 0-2 are normal write-back memory (RAM: code, data,
/// arena, stack), block 3 — which contains the BCM2711 peripheral window at
/// `0xFC00_0000+` — is Device-nGnRE. One table, no level-2/3: the floor
/// needs caches on (LL/SC exclusives require cacheable memory on real
/// cores), not fine-grained protection.
#[cfg(all(feature = "runtime", target_arch = "aarch64", target_os = "none"))]
#[repr(C, align(4096))]
struct L1Table(core::cell::UnsafeCell<[u64; 512]>);

// SAFETY: written only by the `_start` asm before any Rust code runs and
// before the MMU consumes it; afterwards only the hardware walker reads it.
// No two accessors ever race.
#[cfg(all(feature = "runtime", target_arch = "aarch64", target_os = "none"))]
unsafe impl Sync for L1Table {}

#[cfg(all(feature = "runtime", target_arch = "aarch64", target_os = "none"))]
static L1_TABLE: L1Table = L1Table(core::cell::UnsafeCell::new([0; 512]));

/// The bare-metal boot entry (`_start`) for the Pi 4 machine (BCM2711,
/// QEMU `-M raspi4b`).
///
/// Firmware loads `kernel8.img` at `0x80000` and jumps to its first byte at
/// EL2 (the image's linker script places this function's `.text.boot`
/// section first), with the MMU and caches off and no startup stack. There
/// is no kernel and no process ABI: no argc/argv/envp exist, so
/// [`rust_entry`] receives zeros — `env_block` correctly reports an empty
/// environment.
///
/// The naked body owns the whole machine bring-up, in order:
///
/// 1. **Park secondary cores.** Firmware normally parks them already
///    (spin-table); the `MPIDR_EL1` check makes the image self-sufficient
///    if one ever arrives here.
/// 2. **Drop EL2 → EL1.** `HCR_EL2.RW` selects `AArch64` EL1;
///    `CNTHCTL_EL2.{EL1PCTEN,EL1PCEN}` lets EL1 read the generic timer
///    untrapped (the floor's clock); `SCTLR_EL1` starts as its RES1
///    pattern with MMU/caches off; `eret` lands on the EL1 continuation
///    with DAIF masked. Entered at EL1 directly, the drop is skipped (the
///    handoff contract is EL2-or-EL1; EL3 entry is out of contract).
/// 3. **FP/SIMD enable + stack + BSS.** `CPACR_EL1.FPEN = 0b11` makes EL1
///    FP/SIMD accesses untrapped — the `aarch64-unknown-none` ABI compiles
///    with NEON enabled, so compiler-vectorized code (memcpy and friends)
///    traps fatally without it (the reset value traps and there is no
///    vector table; the MMU-enable `isb` below synchronizes the write
///    before any Rust code runs). The boot stack and BSS bounds come from
///    the linker script (`__stack_top`, `__bss_start`/`__bss_end`); BSS
///    clear is what zero-initializes the page arena, upholding `mmap`'s
///    zero-fill contract.
/// 4. **Identity-map MMU + caches.** Cortex-A72 LL/SC exclusives
///    (`ldxr`/`stxr` — every atomic in the image) are only architecturally
///    guaranteed on cacheable memory, so the MMU must be on before any
///    Rust code runs. [`L1_TABLE`] gets four 1 GiB blocks (RAM normal,
///    peripheral window device); `MAIR_EL1` defines Attr0 = normal
///    write-back write-allocate (`0xFF`), Attr1 = Device-nGnRE (`0x04`);
///    `TCR_EL1` selects 4 KiB granule, 32-bit VA (`T0SZ = 32`, level-1
///    start), inner-shareable write-back walks, 36-bit IPA, TTBR1 walks
///    disabled. QEMU ignores cache state; on real silicon the firmware
///    hands over with caches off/invalid, and set/way invalidation is the
///    hardware-bring-up hardening, deliberately not modeled here.
/// 5. **Call [`rust_entry`] with `(0, null, null)`.** Never returns; exit
///    is the semihosting channel in `sys`.
///
/// No vector table is installed: the payload's failure channel is the Rust
/// panic path (UART + exit code), which raises no exceptions. An unexpected
/// synchronous exception is fatal by hang — the honest behavior for a floor
/// with no handler policy.
///
/// ```ignore
/// // _start is entered only by the firmware's initial control transfer at
/// // 0x80000 — not callable from any Rust context.
/// ```
#[cfg(all(feature = "runtime", target_arch = "aarch64", target_os = "none"))]
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn _start() -> ! {
    // SAFETY: `naked_asm` is the whole function body, reached only by the
    // firmware jump described above. Register use is free (no caller, no
    // ABI); every system-register write is documented at its step. The
    // `eret` continuation label and the table/stack/BSS symbols are all
    // link-time-resolved addresses inside this image.
    core::arch::naked_asm!(
        // -- 1: park anything that is not core 0 ------------------------------
        "mrs x0, mpidr_el1",
        "and x0, x0, #0xFF",   // Aff0 = core id within the cluster
        "cbz x0, 2f",
        "1:",
        "wfe",
        "b 1b",
        // -- 2: EL2 -> EL1 ----------------------------------------------------
        "2:",
        "mrs x0, currentel",
        "lsr x0, x0, #2",
        "cmp x0, #2",
        "b.ne 3f",             // already EL1: skip the drop
        "mov x0, #0x80000000", // HCR_EL2.RW: EL1 executes AArch64
        "msr hcr_el2, x0",
        "mov x0, #3",          // CNTHCTL_EL2.EL1PCTEN|EL1PCEN: untrapped timer
        "msr cnthctl_el2, x0",
        "msr cntvoff_el2, xzr", // virtual counter == physical counter
        "movz x0, #0x0800",    // SCTLR_EL1 RES1 pattern (ARMv8.0), MMU/caches off
        "movk x0, #0x30D0, lsl #16",
        "msr sctlr_el1, x0",
        "movz x0, #0x3C5",     // SPSR: DAIF masked, EL1h (SP_EL1)
        "msr spsr_el2, x0",
        "adr x0, 3f",
        "msr elr_el2, x0",
        "eret",
        // -- 3: EL1 from here: FP/SIMD enable, stack, then BSS ----------------
        "3:",
        "mov x0, #0x300000",   // CPACR_EL1.FPEN = 0b11: EL1 FP/SIMD untrapped
        "msr cpacr_el1, x0",
        "adrp x0, __stack_top",
        "add x0, x0, :lo12:__stack_top",
        "mov sp, x0",
        "adrp x0, __bss_start",
        "add x0, x0, :lo12:__bss_start",
        "adrp x1, __bss_end",
        "add x1, x1, :lo12:__bss_end",
        "4:",
        "cmp x0, x1",
        "b.hs 5f",
        "str xzr, [x0], #8",   // bounds are 8-aligned by the linker script
        "b 4b",
        // -- 4: identity map + caches -----------------------------------------
        // Block descriptors: valid (bit 0), block (bit 1 clear), AttrIndx
        // (bits 4:2), SH (bits 9:8), AF (bit 10); the device block adds
        // PXN|UXN (bits 53:54) — nothing executes from peripherals.
        "5:",
        "adrp x0, {l1table}",
        "add x0, x0, :lo12:{l1table}",
        "movz x1, #0x0701",    // 0x0000_0000: normal (Attr0, inner-sh, AF)
        "str x1, [x0]",
        "movz x1, #0x0701",
        "movk x1, #0x4000, lsl #16", // 0x4000_0000: normal
        "str x1, [x0, #8]",
        "movz x1, #0x0701",
        "movk x1, #0x8000, lsl #16", // 0x8000_0000: normal
        "str x1, [x0, #16]",
        "movz x1, #0x0405",    // 0xC000_0000: device (Attr1, AF)
        "movk x1, #0xC000, lsl #16",
        "movk x1, #0x0060, lsl #48", // PXN|UXN
        "str x1, [x0, #24]",
        "msr ttbr0_el1, x0",
        "movz x1, #0x04FF",    // MAIR: Attr0 normal WBWA 0xFF, Attr1 device 0x04
        "msr mair_el1, x1",
        "movz x1, #0x3520",    // TCR: T0SZ=32, IRGN0/ORGN0=WB, SH0=inner
        "movk x1, #0x0080, lsl #16", // EPD1: no TTBR1 walks
        "movk x1, #0x1, lsl #32",    // IPS: 36-bit
        "msr tcr_el1, x1",
        "dsb ish",             // table stores complete before the walker looks
        "tlbi vmalle1",        // no stale translations from earlier stages
        "dsb ish",
        "ic iallu",            // no stale instruction fetches across enable
        "dsb ish",
        "isb",
        "mrs x1, sctlr_el1",
        "orr x1, x1, #1",      // M: MMU on
        "orr x1, x1, #4",      // C: data cache on
        "orr x1, x1, #0x1000", // I: instruction cache on
        "msr sctlr_el1, x1",
        "isb",
        // -- 5: into Rust -------------------------------------------------------
        "mov x29, xzr",        // outermost frame marker
        "mov x0, xzr",         // argc = 0: no process ABI exists here
        "mov x1, xzr",         // argv = null
        "mov x2, xzr",         // envp = null
        "bl {entry}",
        "brk #1",              // unreachable: rust_entry exits via semihosting
        l1table = sym L1_TABLE,
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
// argc/argv/envp are the canonical C entry-point parameter names; renaming
// them to satisfy the lint would obscure the well-known convention.
#[allow(clippy::similar_names)]
extern "C" fn rust_entry(argc: usize, argv: *const *const u8, envp: *const *const u8) -> ! {
    // Stash envp before running main so the coverage exit path can read
    // `LLVM_PROFILE_FILE` from it. The cast drops only the outer const (the
    // atomic stores `*mut`); the pointer is never written through.
    ENVP.store(envp.cast_mut(), core::sync::atomic::Ordering::Release);
    // Install the platform handle (SP02, AB12 write-once). This is the boot
    // sequence point: arch's allocator is heap-free and already up, and the
    // platform vtable is a `static` of const fn pointers (built at compile
    // time), so the install is one atomic store with zero heap. After this
    // line any consumer may read the handle (`kabi::handle`); reading it
    // before this point is a boot-ordering bug, surfaced by `handle`'s panic
    // (the no-read-before-install prerequisite, mirroring the panic-handler
    // seams). The result is ignored by construction: `rust_entry` is the sole
    // process entry, so a second install cannot occur here.
    let _ = crate::platform::install_platform();
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
/// A `#![no_main]` `no_std` binary has no C-runtime `atexit` hook, so the LLVM
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
