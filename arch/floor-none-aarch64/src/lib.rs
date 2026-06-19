//! `reovim-arch-floor-none-aarch64` — the `aarch64` bare-metal language floor.
//!
//! This `#![no_std]` crate owns the per-binary **language-knowledge** symbols
//! for `aarch64-unknown-none` on the Raspberry Pi 4 (BCM2711) — OS-Modes §2.2,
//! master-plan invariant #1 (the LINK, not import, floor↔product crossing):
//! the naked `_start` boot entry (the EL2→EL1 drop + identity-map MMU climb),
//! the `#[panic_handler]` lang item and its allocator-free render/flush
//! mechanism, `rust_eh_personality`, the freestanding `mem` intrinsics the
//! compiler lowers to under `-nodefaultlibs`, and the [`entry!`] macro /
//! `reovim_arch_main` link seam. These were relocated out of `reovim-arch` in
//! SP03; `arch` no longer defines any of them.
//!
//! On bare metal `_start` *is* the firmware entry: firmware jumps here at EL2
//! with the MMU off, and this floor drops to EL1, brings up caches, and hands
//! control to Rust. It reaches the machine only through its matching
//! `reovim-arch-sys-none-aarch64` dep (the `DTB_PTR` boot-pointer static the
//! asm stashes by `sym`, plus the semihosting exit + generic-timer clock floor)
//! and the `kabi/panic` registry it reads on a panic; it never names `arch`
//! (master-plan invariant #8). `unsafe` is allowed here because the language
//! floor owns naked asm and raw-pointer memory management.
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
use reovim_arch_sys_none_aarch64 as sys;

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

// ── _start (aarch64 bare-metal, EL2->EL1) ──────────────────────────────────────

/// The level-1 translation table for the bare-metal identity map, filled by
/// the `_start` asm before the MMU is enabled (Rust never touches it).
///
/// Four 1 GiB block entries cover the 4 GiB physical window the image uses
/// (T0SZ = 32): blocks 0-2 are normal write-back memory (RAM: code, data,
/// arena, stack), block 3 — which contains the BCM2711 peripheral window at
/// `0xFC00_0000+` — is Device-nGnRE. One table, no level-2/3: the floor
/// needs caches on (LL/SC exclusives require cacheable memory on real
/// cores), not fine-grained protection.
#[cfg(feature = "runtime")]
#[repr(C, align(4096))]
struct L1Table(core::cell::UnsafeCell<[u64; 512]>);

// SAFETY: written only by the `_start` asm before any Rust code runs and
// before the MMU consumes it; afterwards only the hardware walker reads it.
// No two accessors ever race.
#[cfg(feature = "runtime")]
unsafe impl Sync for L1Table {}

#[cfg(feature = "runtime")]
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
///    with NEON enabled, so compiler-vectorized code (`memcpy` and friends)
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
#[cfg(feature = "runtime")]
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
        // -- 0: capture the firmware DTB pointer before x0 is reused ----------
        // Firmware (and QEMU `-dtb`) passes the flattened-device-tree physical
        // address in x0 at entry, but the very next instruction overwrites x0
        // with MPIDR. Move it into x19 — callee-saved in AAPCS64 and untouched
        // by the entire prologue — so it survives to the post-BSS store below.
        // Secondary cores also run this and then park at `wfe`; only core 0
        // reaches the store, so only core 0's (real) DTB pointer is recorded.
        "mov x19, x0",
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
        // Stash the firmware DTB pointer, held in x19 since entry. This is the
        // first instruction past the BSS clear's `b.hs 5f` exit, so it always
        // runs on core 0; DTB_PTR lives in .bss, so storing it only now — after
        // the clear — keeps the value from being zeroed. x0 is free scratch
        // (the identity-map setup immediately below reloads it).
        "adrp x0, {dtb_ptr}",
        "add x0, x0, :lo12:{dtb_ptr}",
        "str x19, [x0]",
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
        dtb_ptr = sym reovim_arch_sys_none_aarch64::DTB_PTR,
        entry = sym rust_entry,
    )
}

// The firmware-provided DTB physical address (`DTB_PTR`) lives in
// `reovim-arch-sys-none-aarch64` (SP01 edge inversion): it is a raw hardware
// boot pointer and belongs with the raw mechanism, mirroring the x86
// `MULTIBOOT_INFO_PTR` contract. The aarch64 `_start` asm captures x0 into the
// callee-saved `x19` at the very first instruction (before `MPIDR` overwrites
// x0) and stores it via the cross-crate `sym` operand above, after the BSS
// clear. The device-tree reader (`arch::sys::collect_device_inventory`) loads
// it to walk the DTB and enumerate devices.

// ── rust_entry / ENVP / env_block / the arch_main seam / exit_process ──────────

/// The captured process environment vector (`envp`), stashed by [`rust_entry`]
/// at startup so the exit path can read it without re-threading it through
/// every call frame. `null` before entry runs.
///
/// On bare metal there is no process ABI, so `_start` hands `rust_entry` a null
/// `envp`; the capture is null and `env_block` correctly reports an empty
/// environment. The atomic keeps the single-write/single-read access
/// well-defined.
#[cfg(feature = "runtime")]
static ENVP: core::sync::atomic::AtomicPtr<*const u8> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

/// The Rust side of process entry: runs the registered main shim and exits
/// the process with its return code through [`exit_process`].
///
/// `extern "C"` so its calling convention matches the `_start` tail-call. The
/// main shim is the `arch_main` symbol the [`entry!`] macro installs. A bin
/// crate that links this floor with `runtime` but never invokes `entry!` would
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
    // The platform-vtable install is owned by the fixture composition root
    // (SP02): it drives `install_platform()` as the first statement of its
    // `entry!` closure, before any `kabi::handle` read. `rust_entry` therefore
    // just forwards the startup vectors to `arch_main`.
    // SAFETY: `arch_main` is the bin crate's `entry!`-installed shim with this
    // exact signature; `argc`/`argv`/`envp` are the startup vectors forwarded
    // verbatim from `_start` (all zero on bare metal). The shim owns them.
    let code = unsafe { arch_main(argc, argv, envp) };
    exit_process(code)
}

/// The captured process environment as a slice of NUL-terminated `KEY=VALUE`
/// byte strings (each including its terminating NUL), or empty before entry
/// runs. On the freestanding entry the captured `envp` is null, so this is
/// always empty — kept for symmetry with the hosted floors (the profiler reads
/// it the same way on every target).
#[cfg(all(feature = "runtime", arch_coverage))]
#[must_use]
pub(crate) fn env_block() -> &'static [&'static [u8]] {
    use core::sync::atomic::Ordering::Acquire;

    /// Upper bound on env entries collected (the scratch array size). A real
    /// env block is far smaller; entries past this are ignored.
    const ENV_MAX: usize = 4096;

    // A process-static scratch table the returned slice borrows. Accessed only
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
        // SAFETY: `envp` is a NULL-terminated environment array; indexing up to
        // the first NULL stays in bounds by the ABI.
        let entry = unsafe { *envp.add(n) };
        if entry.is_null() {
            break;
        }
        // Scan the C string to its NUL to recover the byte length, then build
        // a slice that includes the terminating NUL (callers want a C string).
        let mut len = 0;
        // SAFETY: `entry` is a NUL-terminated C string; reading bytes up to and
        // including the NUL stays in bounds.
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
/// instrumentation profile is never written automatically. When built under
/// coverage instrumentation this shim calls `__llvm_profile_write_file` before
/// `exit_group`, so an exec'd instrumented fixture emits the profraw the
/// coverage merge consumes. The call is gated on the `arch_coverage` cfg the
/// coverage script sets; an ordinary build never references the symbol.
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
        // cfg, so the `__llvm_prf_*` sections it walks are present. Calling it
        // once immediately before exit is the no_std analog of the std atexit hook.
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
/// reovim_arch_floor_none_aarch64::entry!(|_argc, _argv, _envp| {
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
