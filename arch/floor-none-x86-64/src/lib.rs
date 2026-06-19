//! `reovim-arch-floor-none-x86-64` — the `x86_64` bare-metal language floor.
//!
//! This `#![no_std]` crate owns the per-binary **language-knowledge** symbols
//! for `x86_64-unknown-none` (OS-Modes §2.2, master-plan invariant #1 — the
//! LINK, not import, floor↔product crossing): the naked `_start` boot entry
//! (the Multiboot1 32→64-bit climb), the `#[panic_handler]` lang item and its
//! allocator-free render/flush mechanism, `rust_eh_personality`, the
//! freestanding `mem` intrinsics the compiler lowers to under `-nodefaultlibs`,
//! and the [`entry!`] macro / `reovim_arch_main` link seam. These were
//! relocated out of `reovim-arch` in SP03; `arch` no longer defines any of them.
//!
//! On bare metal `_start` *is* the firmware entry: QEMU's Multiboot1 loader
//! jumps here in 32-bit protected mode and this floor climbs to long mode
//! before handing control to Rust. It reaches the machine only through its
//! matching `reovim-arch-sys-none-x86-64` dep (the `MULTIBOOT_INFO_PTR`
//! boot-pointer static the asm stashes by `sym`, plus the port-I/O exit/clock
//! floor) and the `kabi/panic` registry it reads on a panic; it never names
//! `arch` (master-plan invariant #8). `unsafe` is allowed here because the
//! language floor owns naked asm and raw-pointer memory management.
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
use reovim_arch_sys_none_x86_64 as sys;

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

// ── _start (x86_64 bare-metal, Multiboot1) ─────────────────────────────────────

/// The Multiboot1 header that marks the image as a bootable kernel.
///
/// QEMU's `-kernel` loader recognizes a Multiboot1 image by a header in the
/// first 8 KiB of the file: magic `0x1BADB002`, a flags word, and a checksum
/// such that `magic + flags + checksum ≡ 0 (mod 2^32)`. `flags = 0` requests
/// nothing extra (no module alignment, no extra header tags) — the loader still
/// fills the default info structure, whose memory map the floor reads via the
/// boot pointer in `ebx` (stashed by `_start` into `MULTIBOOT_INFO_PTR`). The
/// linker script places `.multiboot` first, so the header lands at the image base.
#[cfg(feature = "runtime")]
#[repr(C, align(4))]
struct MultibootHeader {
    magic: u32,
    flags: u32,
    checksum: u32,
}

#[cfg(feature = "runtime")]
#[unsafe(link_section = ".multiboot")]
#[used]
static MULTIBOOT_HEADER: MultibootHeader = MultibootHeader {
    magic: 0x1BAD_B002,
    flags: 0,
    checksum: 0u32.wrapping_sub(0x1BAD_B002),
};

/// A 4 KiB page table (512 × 8-byte entries), filled by the `_start` asm
/// before paging is enabled (Rust never touches it).
///
/// Three of these build the identity map for the climb to long mode:
/// `PML4[0] → PDPT[0] → PD`, where `PD`'s 512 entries are 2 MiB pages covering
/// the low 1 GiB (image, stack, COM1's port space is I/O not memory). One
/// branch, 2 MiB leaves: the floor needs an address space to enter 64-bit
/// mode, not fine-grained protection.
#[cfg(feature = "runtime")]
#[repr(C, align(4096))]
struct PageTable(core::cell::UnsafeCell<[u64; 512]>);

// SAFETY: written only by the `_start` asm before any Rust code runs and
// before the CPU's page walker consumes them; afterwards only the hardware
// walker reads them. No two accessors ever race.
#[cfg(feature = "runtime")]
unsafe impl Sync for PageTable {}

#[cfg(feature = "runtime")]
static PML4: PageTable = PageTable(core::cell::UnsafeCell::new([0; 512]));
#[cfg(feature = "runtime")]
static PDPT: PageTable = PageTable(core::cell::UnsafeCell::new([0; 512]));
#[cfg(feature = "runtime")]
static PD: PageTable = PageTable(core::cell::UnsafeCell::new([0; 512]));

/// The flat long-mode GDT: null, 64-bit ring-0 code (`L=1`), ring-0 data.
///
/// Long mode ignores segment base/limit, so the descriptors only need the
/// type/present/long bits: code = `0x00AF_9A00_0000_FFFF` (present, code,
/// readable, `L`), data = `0x00CF_9200_0000_FFFF` (present, data, writable).
/// Selector `0x08` is the code segment, `0x10` the data segment.
#[cfg(feature = "runtime")]
#[repr(C, align(16))]
struct Gdt([u64; 3]);

// SAFETY: read-only after construction; `lgdt` and the CPU read it, Rust never
// mutates it.
#[cfg(feature = "runtime")]
unsafe impl Sync for Gdt {}

#[cfg(feature = "runtime")]
static GDT: Gdt = Gdt([0, 0x00AF_9A00_0000_FFFF, 0x00CF_9200_0000_FFFF]);

/// The long-mode proof-of-life banner, written to COM1 by `_start` before any
/// Rust runtime exists. NUL-terminated so the asm loop knows where to stop.
#[cfg(feature = "runtime")]
#[used]
static HELLO: [u8; 39] = *b"reovim: hello from long mode (x86_64)\n\0";

// The Multiboot1 info-structure pointer (`MULTIBOOT_INFO_PTR`) the bootloader
// leaves in `EBX` at entry lives in `reovim-arch-sys-none-x86-64` (SP01 edge
// inversion): it is a raw hardware boot pointer and belongs with the raw
// mechanism. The `_start` asm below names it by cross-crate `sym` path. This is
// the boot pointer Invariant 5 calls for gathering *separately* from the
// `(argc, argv, envp)` shape — it never threads through `rust_entry`.

/// The bare-metal boot entry (`_start`) for the QEMU `q35` machine, entered via
/// Multiboot1 (`qemu-system-x86_64 -kernel <elf>`).
///
/// QEMU's Multiboot1 loader loads the ELF and jumps to its entry point in
/// **32-bit protected mode**, paging off, interrupts off, with the boot info
/// pointer in `ebx` and magic `0x2BADB002` in `eax`. There is no kernel and no
/// process ABI: no argc/argv/envp exist, so [`rust_entry`] receives zeros. The
/// boot pointer in `ebx` is stashed into `MULTIBOOT_INFO_PTR` for
/// `sys::collect_boot_info` (where the `aarch64` arm instead discovers RAM live
/// via the `VideoCore` mailbox).
///
/// The naked body owns the whole climb from 32-bit protected mode to 64-bit
/// long mode, in order:
///
/// 1. **Stack + BSS.** A temporary stack at `__stack_top` (also the long-mode
///    stack), then a `rep stosd` BSS clear over `[__bss_start, __bss_end)` —
///    which zero-initializes the page-table statics below and the page arena,
///    upholding `mmap`'s zero-fill contract. BSS is cleared *before* the page
///    tables are filled because they live in `.bss`. Immediately after the
///    clear (and before the banner loop reuses `bl`), `ebx` — the Multiboot
///    info pointer — is stashed into `MULTIBOOT_INFO_PTR`.
/// 2. **Identity page tables.** `PD` gets 512 × 2 MiB present/writable pages
///    (low 1 GiB); `PML4[0]`/`PDPT[0]` point down the branch. `cr3 ← PML4`.
/// 3. **Enable long mode.** `CR4.PAE`, then `EFER.LME` (MSR `0xC0000080`), then
///    `CR0.PG|PE` — activating IA-32e mode in 32-bit compatibility (`CS.L=0`).
/// 4. **Far-jump to 64-bit.** `lgdt` loads [`GDT`]; a far return to selector
///    `0x08` reloads `CS` with the `L=1` descriptor, entering 64-bit mode.
/// 5. **64-bit continuation.** Reload the data segment registers, set `rsp`,
///    initialize COM1 (16550 at port `0x3F8`, 115200 8N1), write [`HELLO`], then
///    call [`rust_entry`] with `(0, null, null)`. Never returns; exit is the
///    `isa-debug-exit` channel in `sys`.
///
/// The `.code32`/`.code64` directives bracket the whole body in one function so
/// the assembler-mode switch cannot leak into sibling functions; the trailing
/// `.code64` restores the default after the (runtime-unreachable) far jump.
///
/// No IDT is installed: the payload's failure channel is the Rust panic path
/// (COM1 + exit code), which raises no exceptions. An unexpected fault is fatal
/// by triple-fault reset — the honest behavior for a floor with no handler
/// policy.
///
/// ```ignore
/// // _start is entered only by QEMU's Multiboot1 control transfer — not
/// // callable from any Rust context.
/// ```
#[cfg(feature = "runtime")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn _start() -> ! {
    // SAFETY: `naked_asm` is the whole function body, reached only by QEMU's
    // Multiboot1 jump described above. Register use is free (no caller, no
    // ABI); the page-table/GDT/string statics and `rust_entry` are
    // link-time-resolved addresses inside this image, and the linker symbols
    // `__stack_top`/`__bss_start`/`__bss_end` are absolute (non-PIE static
    // link). Every control-register and MSR write is documented at its step.
    core::arch::naked_asm!(
        ".code32",
        // -- 1: temporary stack + BSS clear (before the tables are filled) ----
        "lea esp, [__stack_top]",
        "cld",                       // rep stosd ascends; DF must be clear
        "lea edi, [__bss_start]",
        "lea ecx, [__bss_end]",
        "sub ecx, edi",
        "shr ecx, 2",                // dword count (bounds are 8-aligned)
        "xor eax, eax",
        "rep stosd",
        // -- 1b: stash the Multiboot info pointer (ebx) before it is reused ----
        // ebx still holds the loader's info pointer (untouched since entry; the
        // BSS clear used edi/ecx/eax). Store it now — after the clear so it is
        // not zeroed, before the 64-bit banner loop clobbers bl. A plain
        // 4-byte store to the AtomicU32 cell (still single-threaded here); the
        // provider reads it with an atomic load.
        "mov [{mb_ptr}], ebx",
        // -- 2: identity page tables (PD: 512 * 2 MiB present|writable) -------
        "lea edi, [{pd}]",
        "xor ecx, ecx",
        "mov eax, 0x83",             // present|writable|PS(2 MiB), frame 0
        "2:",
        "mov [edi + ecx*8], eax",    // entry low dword: frame|flags
        "mov dword ptr [edi + ecx*8 + 4], 0", // entry high dword
        "add eax, 0x200000",         // next 2 MiB frame
        "inc ecx",
        "cmp ecx, 512",
        "jb 2b",
        "lea eax, [{pdpt}]",
        "or eax, 3",                 // present|writable
        "lea edi, [{pml4}]",
        "mov [edi], eax",
        "mov dword ptr [edi + 4], 0",
        "lea eax, [{pd}]",
        "or eax, 3",
        "lea edi, [{pdpt}]",
        "mov [edi], eax",
        "mov dword ptr [edi + 4], 0",
        "lea eax, [{pml4}]",
        "mov cr3, eax",
        // -- 3: enable long mode (PAE -> LME -> PG) ---------------------------
        "mov eax, cr4",
        "or eax, 0x20",              // CR4.PAE (bit 5)
        "mov cr4, eax",
        "mov ecx, 0xC0000080",       // IA32_EFER
        "rdmsr",
        "or eax, 0x100",             // EFER.LME (bit 8)
        "wrmsr",
        "mov eax, cr0",
        "or eax, 0x80000001",        // CR0.PG (bit 31) | PE (bit 0)
        "mov cr0, eax",
        // -- 4: load GDT, far-jump into the 64-bit code segment ---------------
        "sub esp, 8",                // scratch GDTR: limit(2) + base(4)
        "mov word ptr [esp], 23",    // limit = 3*8 - 1
        "lea eax, [{gdt}]",
        "mov [esp + 2], eax",        // 32-bit base
        "lgdt [esp]",
        "add esp, 8",
        "push 0x08",                 // CS selector (deeper on the stack)
        "lea eax, [3f]",
        "push eax",                  // 64-bit entry offset (popped as EIP)
        "retf",                      // far return: loads CS:EIP -> 64-bit mode
        // -- 5: 64-bit continuation -------------------------------------------
        ".code64",
        "3:",
        "mov ax, 0x10",              // data selector
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "mov fs, ax",
        "mov gs, ax",
        "lea rsp, [__stack_top]",
        // COM1 16550 init: 115200 8N1, FIFO on (mirrors sys::uart::init).
        "mov dx, 0x3F9", "mov al, 0x00", "out dx, al", // IER: no interrupts
        "mov dx, 0x3FB", "mov al, 0x80", "out dx, al", // LCR: DLAB on
        "mov dx, 0x3F8", "mov al, 0x01", "out dx, al", // DLL: divisor 1
        "mov dx, 0x3F9", "mov al, 0x00", "out dx, al", // DLM: divisor high
        "mov dx, 0x3FB", "mov al, 0x03", "out dx, al", // LCR: 8N1, DLAB off
        "mov dx, 0x3FA", "mov al, 0xC7", "out dx, al", // FCR: enable+clear FIFO
        "mov dx, 0x3FC", "mov al, 0x0B", "out dx, al", // MCR: DTR|RTS|OUT2
        // Write HELLO byte-by-byte, polling LSR (0x3FD bit 5 = THR empty).
        "lea rsi, [{hello}]",
        "4:",
        "mov bl, [rsi]",
        "test bl, bl",
        "jz 5f",
        "6:",
        "mov dx, 0x3FD",
        "in al, dx",
        "test al, 0x20",
        "jz 6b",
        "mov dx, 0x3F8",
        "mov al, bl",
        "out dx, al",
        "inc rsi",
        "jmp 4b",
        "5:",
        // -- into Rust: no process ABI here, so (argc, argv, envp) = (0, 0, 0) -
        "xor edi, edi",
        "xor esi, esi",
        "xor edx, edx",
        "call {entry}",
        "ud2",                       // unreachable: rust_entry exits via sys
        ".code64",                   // restore default mode after the block
        mb_ptr = sym reovim_arch_sys_none_x86_64::MULTIBOOT_INFO_PTR,
        pml4 = sym PML4,
        pdpt = sym PDPT,
        pd = sym PD,
        gdt = sym GDT,
        hello = sym HELLO,
        entry = sym rust_entry,
    )
}

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
/// reovim_arch_floor_none_x86_64::entry!(|_argc, _argv, _envp| {
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
