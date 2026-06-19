//! Floor-owned minimal LLVM coverage profiler runtime (#785 Phase 5; relocated
//! from `arch/src/profiler.rs` in SP03).
//!
//! `-C instrument-coverage` makes the compiler emit per-function counter,
//! data, name, and (for MC/DC) bitmap arrays into dedicated ELF sections,
//! plus a reference to the `__llvm_profile_runtime` marker that pulls in the
//! profiler runtime at link. The toolchain's runtime (`profiler_builtins`,
//! compiler-rt's `InstrProfiling`) needs ~38 libc symbols (the stdio family,
//! `malloc`/`free`, `getenv`, `atexit`, …) that a `-nodefaultlibs` `no_std`
//! bin cannot satisfy (the Phase 4 spike failed to link, 2026-06-11).
//!
//! This module is that runtime, hand-written over the floor's arch-sys
//! syscalls (Linux `kernel/pgo` precedent): it defines the
//! `__llvm_profile_runtime` marker so the compiler links *this* instead of
//! compiler-rt, and exports [`__llvm_profile_write_file`] (the symbol the exit
//! shim and the panic handler call under `arch_coverage`) which serializes the
//! raw profile to the path named by `LLVM_PROFILE_FILE`. The build must pass
//! `-Z no-profiler-runtime` so the toolchain does not also try to link
//! `profiler_builtins`.
//!
//! ## Profraw format version
//!
//! The on-disk header carries `INSTR_PROF_RAW_VERSION`. It is hardcoded as
//! [`RAW_VERSION`] (10, decoded field-by-field from a reference profraw the
//! current toolchain emitted) because the toolchain's
//! `__llvm_profile_get_version` accessor lives in compiler-rt — exactly the
//! runtime `-Z no-profiler-runtime` removes, so there is nothing to read it
//! from at run time. The header layout below matches raw-profile v9/v10 (the
//! bitmap fields landed in v9). A future toolchain bump is loud, not silent:
//! `llvm-profdata merge` rejects a profraw whose version/layout disagree
//! with the data, and this constant + header layout are the one place to
//! update.
//!
//! Gated on `arch_coverage` (the cfg the coverage script sets alongside
//! `-C instrument-coverage`): an ordinary build compiles none of this and
//! never references the LLVM sections, so the floor links without any
//! instrumentation runtime.

use reovim_arch_sys_linux_aarch64::{
    AT_FDCWD, O_CLOEXEC, O_CREAT, O_TRUNC, O_WRONLY, close, openat, write,
};

// The LLVM instrumentation sections. The linker synthesizes
// `__start_<section>` / `__stop_<section>` symbols bracketing each
// `link_section` so the runtime can walk them without a hand-maintained
// table. The section names are the LLVM `__llvm_prf_*` convention; declaring
// arbitrary externs with those `link_name`s binds to the bracket symbols the
// linker emits for the compiler-generated sections.
// (Plain comment: rustdoc rejects doc comments on extern blocks via
// `unused_doc_comments`; the per-item docs inside are fine.)
unsafe extern "C" {
    /// First byte of the per-function `__llvm_prf_data` array.
    #[link_name = "__start___llvm_prf_data"]
    static DATA_START: u8;
    /// One past the last byte of `__llvm_prf_data`.
    #[link_name = "__stop___llvm_prf_data"]
    static DATA_STOP: u8;
    /// First byte of the `__llvm_prf_cnts` counter array.
    #[link_name = "__start___llvm_prf_cnts"]
    static CNTS_START: u8;
    /// One past the last byte of `__llvm_prf_cnts`.
    #[link_name = "__stop___llvm_prf_cnts"]
    static CNTS_STOP: u8;
    /// First byte of the `__llvm_prf_bits` MC/DC bitmap array.
    #[link_name = "__start___llvm_prf_bits"]
    static BITS_START: u8;
    /// One past the last byte of `__llvm_prf_bits`.
    #[link_name = "__stop___llvm_prf_bits"]
    static BITS_STOP: u8;
    /// First byte of the `__llvm_prf_names` name-blob array.
    #[link_name = "__start___llvm_prf_names"]
    static NAMES_START: u8;
    /// One past the last byte of `__llvm_prf_names`.
    #[link_name = "__stop___llvm_prf_names"]
    static NAMES_STOP: u8;
}

/// `INSTR_PROF_RAW_VERSION` for the active toolchain (see the module doc:
/// verified against a reference profraw; a toolchain bump fails loudly at
/// `llvm-profdata merge`, and this is the constant to update).
const RAW_VERSION: u64 = 10;

/// Zero-length anchor guaranteeing the `__llvm_prf_bits` section exists even
/// in a non-MC/DC instrumented build, where the compiler emits no bitmap
/// entries: with no input section the linker leaves the `__start_/__stop_`
/// bracket symbols undefined and the link fails. Contributes zero bytes, so
/// MC/DC builds are unaffected; `#[used]` marks it retained against
/// `--gc-sections`.
#[unsafe(link_section = "__llvm_prf_bits")]
#[used]
static BITS_ANCHOR: [u8; 0] = [];

/// The size in bytes of one `__llvm_prf_data` entry (`__llvm_profile_data`).
///
/// The raw header reports `NumData` as a *count* of these structs, not a byte
/// length; the writer divides the section span by this. The struct is
/// 64 bytes on 64-bit targets at raw-profile v10 (`InstrProfData.inc`): two
/// `u64` hashes (name, func), four pointer-sized fields (relative counter
/// pointer, relative bitmap pointer, function pointer, values pointer), then
/// `u32` NumCounters, two `u16` value-site counts, `u32` NumBitmapBytes —
/// 60 bytes padded to 64. Verified arithmetically against a
/// toolchain-emitted reference profraw (288 = 128 header + 32 binary-ids +
/// 1 x 64 data + 8 counters + 56 padded names) AND against the fixture
/// binary's section (6400 bytes / 64 = 100 functions exactly). This is the
/// second version-coupled constant (with the header layout); it is asserted
/// against the section span being an exact multiple at write time.
const DATA_ENTRY_SIZE: usize = 64;

/// One profile-counter entry is a `u64`.
const COUNTER_SIZE: usize = 8;

/// The `NumData` count for a `__llvm_prf_data` span of `data_len` bytes, or
/// `None` when the span is not an exact multiple of [`DATA_ENTRY_SIZE`] — which
/// means the version-coupled entry size is stale and the header would be
/// corrupt. Factored out so both arms are directly unit-testable (the
/// non-multiple arm is otherwise structurally unreachable under correct
/// instrumentation, where the span is always a multiple).
const fn num_data_entries(data_len: usize) -> Option<u64> {
    if data_len % DATA_ENTRY_SIZE != 0 {
        return None;
    }
    Some((data_len / DATA_ENTRY_SIZE) as u64)
}

/// The default profraw filename when `LLVM_PROFILE_FILE` is unset, matching
/// the toolchain runtime's default so an exec without the env var still emits
/// a discoverable file.
const DEFAULT_PROFILE_PATH: &[u8] = b"default.profraw\0";

/// Computes `__start_/__stop_` section span as a byte length.
///
/// # Safety
///
/// `start`/`stop` must be the linker-emitted bracket symbols for the same
/// section, with `stop >= start`; the result is their address difference.
unsafe fn span_len(start: *const u8, stop: *const u8) -> usize {
    // SAFETY: `start` and `stop` bracket one contiguous linker section, so
    // both derive from the same allocated object and `stop >= start`; the
    // pointer difference is the section's byte length.
    unsafe { stop.offset_from(start).cast_unsigned() }
}

/// Writes all of `buf` to `fd`, looping over short writes; stops on the first
/// error (best-effort at process exit, no retry budget).
fn write_all(fd: i32, buf: &[u8]) -> bool {
    let mut off = 0;
    while off < buf.len() {
        match write(fd, &buf[off..]) {
            Ok(0) | Err(_) => return false,
            Ok(n) => off += n,
        }
    }
    true
}

/// Writes the 128-byte raw-profile header (raw-profile v10 layout, verified
/// field-by-field against a toolchain-emitted profraw on the current nightly).
///
/// The sixteen `u64` fields are emitted little-endian (every supported target
/// is little-endian). Delta semantics mirror compiler-rt's writer: `counters_delta`
/// and `bitmap_delta` are the counter/bitmap section starts MINUS the data
/// section start (the per-record pointers the compiler emits are relative,
/// and the reader walks them against these section-relative deltas);
/// `names_delta` is the absolute names start. The writer emits no binary-id
/// note, no vtable profiling, and no value-profiling names, so those sizes
/// are zero; `ValueKindLast` is the format's fixed enum tail (2).
#[allow(clippy::too_many_arguments)]
fn write_header(
    fd: i32,
    version: u64,
    num_data: u64,
    num_counters: u64,
    num_bitmap_bytes: u64,
    names_size: u64,
    counters_delta: u64,
    bitmap_delta: u64,
    names_delta: u64,
) -> bool {
    // The raw-profile magic: "lprofr" with byte-order sentinels 0xFF/0x81
    // (`__llvm_profile_get_magic`), little-endian on disk as 81 72 66 6f 72 70
    // 6c ff — verified against a toolchain-emitted v10 profraw.
    const MAGIC: u64 = 0xff6c_7072_6f66_7281;
    // Counters follow the header with no extra alignment padding here (the
    // section data is already 8-aligned); bitmap follows counters likewise.
    let header: [u64; 16] = [
        MAGIC,
        version,
        0, // BinaryIdsSize: no build-id note emitted.
        num_data,
        0, // PaddingBytesBeforeCounters.
        num_counters,
        0, // PaddingBytesAfterCounters.
        num_bitmap_bytes,
        0, // PaddingBytesAfterBitmapBytes.
        names_size,
        counters_delta,
        bitmap_delta,
        names_delta,
        0, // NumVTables: no vtable profiling emitted.
        0, // VNamesSize: no value-profiling names emitted.
        2, // ValueKindLast (IPVK_Last): fixed by the v10 format.
    ];
    let mut bytes = [0u8; 16 * 8];
    let mut i = 0;
    while i < header.len() {
        let le = header[i].to_le_bytes();
        let base = i * 8;
        bytes[base..base + 8].copy_from_slice(&le);
        i += 1;
    }
    write_all(fd, &bytes)
}

/// Writes `len` zero bytes to `fd` (section-boundary padding to an 8-byte
/// multiple), in chunks so no allocation is needed.
fn write_padding(fd: i32, len: usize) -> bool {
    let zeros = [0u8; 8];
    let mut remaining = len;
    while remaining > 0 {
        let n = remaining.min(zeros.len());
        if !write_all(fd, &zeros[..n]) {
            return false;
        }
        remaining -= n;
    }
    true
}

/// Reads `LLVM_PROFILE_FILE` from the captured environment, returning the
/// value bytes (without the `NAME=` prefix) including the terminating NUL, or
/// `None` when unset. The returned slice borrows the live environment block,
/// so the path is NUL-terminated in place (no copy, no allocation).
fn profile_path<'a>(env: &'a [&'a [u8]]) -> Option<&'a [u8]> {
    const KEY: &[u8] = b"LLVM_PROFILE_FILE=";
    for entry in env {
        if entry.len() > KEY.len() && &entry[..KEY.len()] == KEY {
            // The entry slice is NUL-terminated (it ends at the env string's
            // own NUL, captured by `env::vars`), so the value tail is a valid
            // C string for `openat`.
            return Some(&entry[KEY.len()..]);
        }
    }
    None
}

/// Serializes the in-memory coverage profile to `LLVM_PROFILE_FILE` (or the
/// default path) as a raw-profile file the coverage merge consumes.
///
/// This is the public entry the exit shim + the panic handler call under
/// `arch_coverage`. It is `extern "C"` and `no_mangle` so it overrides the
/// toolchain runtime's symbol of the same name; with `-Z no-profiler-runtime`
/// no other definition exists. Best-effort: any syscall failure aborts the
/// write silently (there is no recovery at process exit).
///
/// # Safety
///
/// Reads the linker-bracketed `__llvm_prf_*` sections, which exist only under
/// `-C instrument-coverage`; the `arch_coverage` gate guarantees that. Must be
/// called at most once per process (the toolchain's write-file contract).
///
/// ```ignore
/// // __llvm_profile_write_file requires arch_coverage + runtime features and
/// // live __llvm_prf_* linker sections — only callable from an instrumented binary.
/// unsafe { __llvm_profile_write_file() };
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __llvm_profile_write_file() -> i32 {
    let env = crate::env_block();
    let path = profile_path(env).unwrap_or(DEFAULT_PROFILE_PATH);

    // SAFETY: the bracket symbols are the linker's section delimiters for the
    // compiler-emitted `__llvm_prf_*` sections; taking their addresses and
    // differencing is the documented section-walk. Reading the section bytes
    // is sound: they are live, initialized, statically allocated data for the
    // life of the process.
    let written = unsafe { write_profile(path) };
    i32::from(!written)
}

/// The section walk + serialization, factored out so the unsafe surface of
/// the exported entry is a single call. Returns `true` on a complete write.
///
/// # Safety
///
/// As [`__llvm_profile_write_file`]: the `__llvm_prf_*` sections must be live
/// (guaranteed by `arch_coverage`).
unsafe fn write_profile(path: &[u8]) -> bool {
    // SAFETY: each pair brackets one contiguous compiler-emitted section.
    let data_len = unsafe { span_len(&raw const DATA_START, &raw const DATA_STOP) };
    let cnts_len = unsafe { span_len(&raw const CNTS_START, &raw const CNTS_STOP) };
    let bits_len = unsafe { span_len(&raw const BITS_START, &raw const BITS_STOP) };
    let names_len = unsafe { span_len(&raw const NAMES_START, &raw const NAMES_STOP) };

    // The data section is an array of fixed-size structs; a non-multiple span
    // means the version-coupled `DATA_ENTRY_SIZE` is stale — refuse rather
    // than emit a corrupt header.
    let Some(num_data) = num_data_entries(data_len) else {
        return false;
    };
    let num_counters = (cnts_len / COUNTER_SIZE) as u64;

    let version = RAW_VERSION;

    // Delta semantics per compiler-rt's writer: counters/bitmap deltas are
    // section starts relative to the DATA section start (the reader rebases
    // each record's relative pointer against these, advancing by one data
    // entry per record); the names delta is absolute.
    let data_addr = (&raw const DATA_START).addr() as u64;
    let counters_delta = ((&raw const CNTS_START).addr() as u64).wrapping_sub(data_addr);
    let bitmap_delta = ((&raw const BITS_START).addr() as u64).wrapping_sub(data_addr);
    let names_delta = (&raw const NAMES_START).addr() as u64;

    let fd = match openat(AT_FDCWD, path, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0o644) {
        Ok(raw) => match i32::try_from(raw) {
            Ok(fd) => fd,
            Err(_) => return false,
        },
        Err(_) => return false,
    };

    // SAFETY: each bracket symbol pair delimits live, initialized section
    // bytes for the process lifetime; the slices cover exactly each span and
    // are only read.
    let data = unsafe { core::slice::from_raw_parts(&raw const DATA_START, data_len) };
    let cnts = unsafe { core::slice::from_raw_parts(&raw const CNTS_START, cnts_len) };
    let bits = unsafe { core::slice::from_raw_parts(&raw const BITS_START, bits_len) };
    let names = unsafe { core::slice::from_raw_parts(&raw const NAMES_START, names_len) };

    // Sections are written in the raw-profile order (header, data, counters,
    // bitmap, names), then the names region is padded to an 8-byte boundary so
    // the file ends aligned. The write stops at the first failure; `fd` is
    // closed regardless and the overall success is reported to the caller.
    let ok = write_header(
        fd,
        version,
        num_data,
        num_counters,
        bits_len as u64,
        names_len as u64,
        counters_delta,
        bitmap_delta,
        names_delta,
    ) && write_all(fd, data)
        && write_all(fd, cnts)
        && write_all(fd, bits)
        && write_all(fd, names)
        && write_padding(fd, names_len.wrapping_neg() & 7);

    let _ = close(fd);
    ok
}

// The `__llvm_profile_runtime` marker. `-C instrument-coverage` emits a
// reference to this symbol from every instrumented object; providing it here
// (and passing `-Z no-profiler-runtime`) makes the compiler link *this*
// module as the runtime instead of compiler-rt's `profiler_builtins`. The
// symbol's only role is to be present at link; its value is never read.
// (Plain comment: rustdoc rejects doc comments on a `static` with
// `no_mangle` in some positions; the role is documented here in prose.)
// The lowercase `__llvm_profile_*` name is the ABI symbol the compiler emits
// a reference to, so the casing lint is allowed for this one item.
#[unsafe(no_mangle)]
#[used]
#[allow(non_upper_case_globals)]
pub(crate) static __llvm_profile_runtime: i32 = 0;

// SP07 park: the profiler unit tests (`profiler_tests.rs`) are NOT re-homed
// here. Re-homing them would force this minimal floor crate to carry a
// `selftest`-gated `reovim-testrt` dep purely for unit tests whose behaviour
// the SP03 Phase-4 bare-metal fixtures already prove (the `arch_coverage`
// boot path is exercised by the fixture coverage-merge AC at landing). They
// stay parked in `arch/src/profiler_tests.rs` until a floor-side selftest
// harness is justified (00-master-plan deferred list).
