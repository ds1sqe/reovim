//! Large-file performance gate benchmarks (#739).
//!
//! Run: `cargo bench -p reovim-bench --bench large_file_gates`
//!
//! Gates G1–G6 are measured here. G7 (small-file regression) is
//! verified via `cargo test -p reovim-provider-text`.
//!
//! Expected total runtime: ~35 minutes (10 samples × 30 s measurement per gate).
//!
//! # Gates
//!
//! | Gate | Operation                        | Target              |
//! |------|----------------------------------|---------------------|
//! | G1   | Open 2 GB UTF-8 log              | < 5 s, RSS < 200 MB |
//! | G2   | Scroll (per frame) on 2 GB       | < 16 ms             |
//! | G3   | Insert/delete at any pos on 2 GB | < 50 ms             |
//! | G4   | Write 2 GB with edits            | < 30 s              |
//! | G5   | Pattern search on 2 GB           | < 1 s               |
//! | G6   | Open 500 MB ELF binary           | < 2 s, RSS < 50 MB  |
//! | G7   | Small-file unchanged             | `cargo test`        |

#![allow(clippy::cast_possible_truncation)]
// G3/G4 use large BatchSize::LargeInput to avoid re-opening the 2 GB file per
// iteration — the input is the already-opened buffer, not the fixture file.

use std::{io::Write, path::Path, sync::Arc, time::SystemTime};

use {
    criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main},
    memmap2::Mmap,
    regex::Regex,
    reovim_bench_utils::{large_file, rss},
    reovim_driver_vfs::{FileMapping, MappedFile},
    reovim_provider_text::{HeapMapping, VirtualBuffer},
    reovim_types_text::Position,
};

// ─── Fixture sizes ────────────────────────────────────────────────────────────

const SIZE_2GB: u64 = 2 * 1024 * 1024 * 1024;
const SIZE_500MB: u64 = 500 * 1024 * 1024;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Generate a UTF-8 log tmpfile of approximately `size` bytes.
///
/// The file is kept alive as long as the returned [`tempfile::NamedTempFile`]
/// is in scope.  The fixture is written in full before the function returns
/// so the benchmark sees a stable, flushed file.
///
/// # Errors
///
/// Panics if the tmpfile cannot be created or written.
fn generate_utf8_log_file(size: u64) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().expect("tmpfile create failed");
    {
        let mut writer = std::io::BufWriter::new(f.as_file_mut());
        large_file::write_utf8_log(&mut writer, size).expect("write_utf8_log failed");
        writer.flush().expect("flush failed");
    }
    f
}

/// Generate an ELF binary tmpfile of approximately `size` bytes.
///
/// # Errors
///
/// Panics if the tmpfile cannot be created or written.
fn generate_elf_file(size: u64) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().expect("tmpfile create failed");
    {
        let mut writer = std::io::BufWriter::new(f.as_file_mut());
        large_file::write_elf_binary(&mut writer, size).expect("write_elf_binary failed");
        writer.flush().expect("flush failed");
    }
    f
}

/// Open a file via `memmap2::Mmap` → `MappedFile` → `Arc<dyn FileMapping>`.
///
/// The mmap is created in read-only mode.  The caller must not truncate the
/// file while the returned mapping is alive (SIGBUS risk).
///
/// # Safety
///
/// `memmap2::Mmap::map` is unsafe because an external truncation of the
/// underlying file while the mmap is live would cause undefined behaviour
/// (SIGBUS on Linux).  In benchmark fixtures the file is stable.
///
/// # Errors
///
/// Panics if the file cannot be opened or mapped.
fn mmap_open(path: &Path) -> Arc<dyn FileMapping> {
    let file = std::fs::File::open(path).expect("file open failed");
    let meta = file.metadata().expect("metadata failed");
    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let size = meta.len();

    // SAFETY: The fixture file is not modified or truncated during the
    // benchmark.  A SIGBUS would only occur if the file were externally
    // shrunk while the mapping is live, which does not happen here.
    let mmap: Mmap = unsafe { Mmap::map(&file) }.expect("mmap failed");

    Arc::new(MappedFile::new(mmap, path, mtime, size))
}

// ─── G1: Open 2 GB UTF-8 log (< 5 s, RSS < 200 MB) ──────────────────────────

fn bench_g1_open_2gb_utf8(c: &mut Criterion) {
    eprintln!("[G1] Generating 2 GB UTF-8 log fixture — this will take a while...");
    let tmpfile = generate_utf8_log_file(SIZE_2GB);
    let path = tmpfile.path().to_path_buf();
    eprintln!("[G1] Fixture ready: {}", path.display());

    let mut group = c.benchmark_group("large_file/g1_open_2gb_utf8");

    group.bench_function("open_and_index", |b| {
        b.iter_batched(
            || path.clone(),
            |p| {
                let mapping = mmap_open(&p);
                let buf = VirtualBuffer::from_mapping(mapping)
                    .expect("G1: UTF-8 validation failed — fixture must be valid UTF-8");
                // Return line_count so the optimizer can't elide the work.
                std::hint::black_box(buf.line_count())
            },
            BatchSize::LargeInput,
        );
    });

    // Measure RSS delta for one open (outside Criterion timing loop).
    let (line_count, rss_before, rss_after) = rss::measure_rss(|| {
        let mapping = mmap_open(&path);
        let buf = VirtualBuffer::from_mapping(mapping).expect("G1 RSS: UTF-8 validation failed");
        buf.line_count()
    });
    let rss_delta_mb = (rss_after.saturating_sub(rss_before)) as f64 / (1024.0 * 1024.0);
    let rss_total_mb = rss_after as f64 / (1024.0 * 1024.0);
    eprintln!(
        "[G1] RSS delta: {rss_delta_mb:.1} MB  |  RSS total: {rss_total_mb:.1} MB  \
         |  Lines indexed: {line_count}  \
         |  TARGET: delta < 200 MB"
    );

    group.finish();
    // Keep tmpfile alive until after the group finishes.
    drop(tmpfile);
}

// ─── G2: Scroll per frame on 2 GB (< 16 ms) ──────────────────────────────────

fn bench_g2_scroll_2gb(c: &mut Criterion) {
    eprintln!("[G2] Generating 2 GB UTF-8 log fixture...");
    let tmpfile = generate_utf8_log_file(SIZE_2GB);
    let path = tmpfile.path().to_path_buf();

    let mapping = mmap_open(&path);
    let buf = VirtualBuffer::from_mapping(mapping).expect("G2: UTF-8 validation failed");
    let lc = buf.line_count();
    eprintln!("[G2] Buffer open: {lc} lines");

    let mut group = c.benchmark_group("large_file/g2_scroll_2gb");

    // Five representative positions: start, 25%, 50%, 75%, end.
    let positions: &[(&str, usize)] = &[
        ("line_0", 0),
        ("line_25pct", lc / 4),
        ("line_50pct", lc / 2),
        ("line_75pct", lc * 3 / 4),
        ("line_end", lc.saturating_sub(1)),
    ];

    for &(label, idx) in positions {
        group.bench_with_input(BenchmarkId::new("line_access", label), &idx, |b, &idx| {
            b.iter(|| {
                std::hint::black_box(buf.line(idx));
            });
        });
    }

    group.finish();
    drop(tmpfile);
}

// ─── G3: Insert/delete at any position on 2 GB (< 50 ms) ────────────────────

fn bench_g3_insert_delete_2gb(c: &mut Criterion) {
    eprintln!("[G3] Generating 2 GB UTF-8 log fixture...");
    let tmpfile = generate_utf8_log_file(SIZE_2GB);
    let path = tmpfile.path().to_path_buf();

    let mapping = mmap_open(&path);
    let base_buf = VirtualBuffer::from_mapping(mapping).expect("G3: UTF-8 validation failed");
    let lc = base_buf.line_count();
    eprintln!("[G3] Buffer open: {lc} lines");
    eprintln!("[G3] NOTE: rebuild_line_index() is O(n) — this is the production path.");

    let mut group = c.benchmark_group("large_file/g3_insert_delete_2gb");

    // Insert at beginning (line 0, col 0).
    group.bench_function("insert_at_beginning", |b| {
        b.iter_batched_ref(
            || base_buf.clone(),
            |buf| {
                buf.insert_at(Position::new(0, 0), "benchmark text");
            },
            BatchSize::LargeInput,
        );
    });

    // Insert at middle (line lc/2, col 0).
    let mid = lc / 2;
    group.bench_with_input(
        BenchmarkId::new("insert_at_middle", format!("line_{mid}")),
        &mid,
        |b, &mid| {
            b.iter_batched_ref(
                || base_buf.clone(),
                |buf| {
                    buf.insert_at(Position::new(mid, 0), "benchmark text");
                },
                BatchSize::LargeInput,
            );
        },
    );

    // Delete one line at beginning.
    group.bench_function("delete_line_at_beginning", |b| {
        b.iter_batched_ref(
            || base_buf.clone(),
            |buf| {
                let _ = buf.delete_range(Position::new(0, 0), Position::new(1, 0));
            },
            BatchSize::LargeInput,
        );
    });

    // Delete one line at middle.
    group.bench_with_input(
        BenchmarkId::new("delete_line_at_middle", format!("line_{mid}")),
        &mid,
        |b, &mid| {
            b.iter_batched_ref(
                || base_buf.clone(),
                |buf| {
                    let _ = buf.delete_range(Position::new(mid, 0), Position::new(mid + 1, 0));
                },
                BatchSize::LargeInput,
            );
        },
    );

    group.finish();
    drop(tmpfile);
}

// ─── G4: Write 2 GB with edits (< 30 s) ──────────────────────────────────────

fn bench_g4_write_2gb(c: &mut Criterion) {
    eprintln!("[G4] Generating 2 GB UTF-8 log fixture...");
    let tmpfile = generate_utf8_log_file(SIZE_2GB);
    let path = tmpfile.path().to_path_buf();

    let mapping = mmap_open(&path);
    let base_buf = VirtualBuffer::from_mapping(mapping).expect("G4: UTF-8 validation failed");
    let lc = base_buf.line_count();
    eprintln!("[G4] Buffer open: {lc} lines — applying small edit before write");

    // Apply one small edit so this measures the edited-file write path,
    // not just the pristine-mmap streaming case.
    let mut edited = base_buf.clone();
    edited.insert_at(Position::new(lc / 2, 0), "# benchmark edit\n");

    let mut group = c.benchmark_group("large_file/g4_write_2gb");

    group.bench_function("write_to_sink", |b| {
        b.iter_batched_ref(
            || edited.clone(),
            |buf| {
                let mut sink = std::io::sink();
                buf.write_to(&mut sink).expect("G4: write_to failed");
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
    drop(tmpfile);
}

// ─── G5: Pattern search on 2 GB (< 1 s) ──────────────────────────────────────

fn bench_g5_search_2gb(c: &mut Criterion) {
    eprintln!("[G5] Generating 2 GB UTF-8 log fixture...");
    let tmpfile = generate_utf8_log_file(SIZE_2GB);
    let path = tmpfile.path().to_path_buf();

    let mapping = mmap_open(&path);
    let buf = VirtualBuffer::from_mapping(mapping).expect("G5: UTF-8 validation failed");
    let lc = buf.line_count();
    eprintln!("[G5] Buffer open: {lc} lines");
    eprintln!("[G5] NOTE: inline regex scan — not the full SearchProvider module stack.");

    let re = Regex::new("ERROR").expect("regex compile failed");
    let mut group = c.benchmark_group("large_file/g5_search_2gb");

    // Search from line 0 (worst case: first ERROR is near the top).
    group.bench_function("search_from_beginning", |b| {
        b.iter(|| {
            let mut found = false;
            for line_idx in 0..buf.line_count() {
                if let Some(line) = buf.line(line_idx)
                    && re.is_match(&line)
                {
                    found = true;
                    break;
                }
            }
            std::hint::black_box(found)
        });
    });

    // Search from the middle of the file.
    let mid = lc / 2;
    group.bench_with_input(
        BenchmarkId::new("search_from_middle", format!("line_{mid}")),
        &mid,
        |b, &mid| {
            b.iter(|| {
                let mut found = false;
                for line_idx in mid..buf.line_count() {
                    if let Some(line) = buf.line(line_idx)
                        && re.is_match(&line)
                    {
                        found = true;
                        break;
                    }
                }
                std::hint::black_box(found)
            });
        },
    );

    group.finish();
    drop(tmpfile);
}

// ─── G6: Open 500 MB ELF binary (< 2 s, RSS < 50 MB) ────────────────────────

fn bench_g6_open_500mb_elf(c: &mut Criterion) {
    eprintln!("[G6] Generating 500 MB ELF binary fixture...");
    let tmpfile = generate_elf_file(SIZE_500MB);
    let path = tmpfile.path().to_path_buf();
    eprintln!("[G6] Fixture ready: {}", path.display());
    eprintln!(
        "[G6] NOTE: ELF is NOT valid UTF-8 (null bytes). \
         VirtualBuffer::from_mapping() is expected to reject it with InvalidUtf8. \
         The gate measures mmap + rejection speed. \
         Real binary codec path goes through a separate codec, not VirtualBuffer."
    );

    let mut group = c.benchmark_group("large_file/g6_open_500mb_elf");

    // Measure mmap + InvalidUtf8 rejection time.
    group.bench_function("mmap_and_reject", |b| {
        b.iter_batched(
            || path.clone(),
            |p| {
                let mapping = mmap_open(&p);
                // Expected to fail fast with InvalidUtf8.
                let result = VirtualBuffer::from_mapping(mapping);
                std::hint::black_box(result.is_err())
            },
            BatchSize::LargeInput,
        );
    });

    // RSS measurement (outside Criterion timing loop).
    let (rejected, rss_before, rss_after) = rss::measure_rss(|| {
        let mapping = mmap_open(&path);
        VirtualBuffer::from_mapping(mapping).is_err()
    });
    let rss_delta_mb = (rss_after.saturating_sub(rss_before)) as f64 / (1024.0 * 1024.0);
    let rss_total_mb = rss_after as f64 / (1024.0 * 1024.0);
    eprintln!(
        "[G6] Rejected (InvalidUtf8): {rejected}  \
         |  RSS delta: {rss_delta_mb:.1} MB  |  RSS total: {rss_total_mb:.1} MB  \
         |  TARGET: delta < 50 MB"
    );

    // Also benchmark a heap-backed 500 MB mapping to verify HeapMapping works
    // at this scale (no mmap in play — pure RSS from Vec allocation).
    // This is not a gated benchmark but gives a lower-bound reference.
    //
    // We do NOT run this in Criterion iterations because allocating 500 MB
    // per iteration would exhaust address space quickly.  One manual timing
    // call is enough.
    let (heap_rejected, heap_rss_before, heap_rss_after) = rss::measure_rss(|| {
        // Build a 64-byte ELF header + zeros — same content as the file.
        let mut data = Vec::with_capacity(SIZE_500MB as usize);
        // ELF magic
        data.extend_from_slice(&[0x7f, b'E', b'L', b'F']);
        data.extend_from_slice(&[2, 1, 1, 0]); // class, data, version, osabi
        data.extend_from_slice(&[0u8; 8]); // padding
        data.extend_from_slice(&[2, 0]); // e_type: ET_EXEC
        data.extend_from_slice(&[0x3E, 0]); // e_machine: EM_X86_64
        data.extend_from_slice(&[1, 0, 0, 0]); // e_version
        // Zero fill remaining bytes to reach 500 MB.
        data.resize(SIZE_500MB as usize, 0u8);
        let mapping: Arc<dyn FileMapping> = Arc::new(HeapMapping(data));
        VirtualBuffer::from_mapping(mapping).is_err()
    });
    let heap_delta_mb = (heap_rss_after.saturating_sub(heap_rss_before)) as f64 / (1024.0 * 1024.0);
    eprintln!(
        "[G6] HeapMapping reference — Rejected: {heap_rejected}  \
         |  RSS delta: {heap_delta_mb:.1} MB  (expected ~500 MB from Vec alloc)"
    );

    group.finish();
    drop(tmpfile);
}

// ─── Criterion groups ─────────────────────────────────────────────────────────

criterion_group! {
    name = large_file_gates;
    config = reovim_bench_utils::slow_bench_config();
    targets =
        bench_g1_open_2gb_utf8,
        bench_g2_scroll_2gb,
        bench_g3_insert_delete_2gb,
        bench_g4_write_2gb,
        bench_g5_search_2gb,
        bench_g6_open_500mb_elf
}

criterion_main!(large_file_gates);
