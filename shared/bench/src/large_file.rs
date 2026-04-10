//! Programmatic large-file fixture generators for performance benchmarking.
//!
//! All generators write to [`std::io::Write`] so callers control where data
//! lands (tmpfile, pipe, in-memory buffer, etc.).  Every function in this
//! module is **pure `std`** — no driver or module dependencies.
//!
//! # Fixture types
//!
//! | Generator | Content | Typical size |
//! |-----------|---------|--------------|
//! | [`write_utf8_log`] | Repeating timestamped log lines | 2 GB |
//! | [`write_elf_binary`] | Valid 64-byte ELF header + zero fill | 500 MB |
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use reovim_bench_utils::large_file;
//!
//! let mut f = File::create("/tmp/big.log").unwrap();
//! large_file::write_utf8_log(&mut f, 2 * 1024 * 1024 * 1024).unwrap();
//! ```

use std::io::{self, Write};

/// Write a repeating UTF-8 log file of approximately `target_bytes` bytes.
///
/// Each line looks like:
/// ```text
/// 2026-01-15T08:30:00.000Z INFO  [worker-042] Processing request id=00000001 payload_size=4096 status=OK\n
/// ```
///
/// The generator cycles through a set of log templates so the content is not
/// perfectly uniform, which better exercises real-world scan paths.
///
/// The final size may be up to one line length short of `target_bytes` —
/// we never write a partial line.
///
/// # Errors
///
/// Returns `io::Error` if writing to `w` fails.
pub fn write_utf8_log(w: &mut dyn Write, target_bytes: u64) -> io::Result<u64> {
    const TEMPLATES: &[&str] = &[
        "2026-01-15T08:30:00.000Z INFO  [worker-042] Processing request id={id:08} payload_size=4096 status=OK\n",
        "2026-01-15T08:30:00.001Z DEBUG [worker-007] Cache hit for key=session_{id:08} ttl=3600s\n",
        "2026-01-15T08:30:00.002Z WARN  [worker-099] Slow query elapsed=523ms table=events filter=timestamp>2026-01-14 rows={id}\n",
        "2026-01-15T08:30:00.003Z ERROR [worker-001] Connection refused addr=10.0.0.{octet}:5432 retry=3/5\n",
        "2026-01-15T08:30:00.004Z INFO  [scheduler] Task {id:08} completed duration=1.{ms:03}s cpu_time=0.{ms:03}s\n",
        "2026-01-15T08:30:00.005Z TRACE [gc] Sweep pass freed={id} objects heap_size=134217728 fragmentation=0.{pct:02}\n",
    ];

    let mut written: u64 = 0;
    let mut seq: u64 = 0;
    let mut buf = String::with_capacity(256);

    while written < target_bytes {
        // Template index — seq is always small enough for usize on any
        // platform we target (64-bit), and modulo makes it bounded anyway.
        #[allow(clippy::cast_possible_truncation)]
        let idx = (seq as usize) % TEMPLATES.len();
        let template = TEMPLATES[idx];
        buf.clear();

        // Simple template expansion — avoids pulling in a formatting crate.
        let octet = (seq % 254) + 1; // 1..=254
        let ms = seq % 1000;
        let pct = seq % 100;

        for segment in template.split('{') {
            if let Some((tag, rest)) = segment.split_once('}') {
                match tag.split_once(':').map_or(tag, |(name, _)| name) {
                    "id" => {
                        // Zero-pad to 8 digits.
                        let _ = std::fmt::Write::write_fmt(&mut buf, format_args!("{seq:08}"));
                    }
                    "octet" => {
                        let _ = std::fmt::Write::write_fmt(&mut buf, format_args!("{octet}"));
                    }
                    "ms" => {
                        let _ = std::fmt::Write::write_fmt(&mut buf, format_args!("{ms:03}"));
                    }
                    "pct" => {
                        let _ = std::fmt::Write::write_fmt(&mut buf, format_args!("{pct:02}"));
                    }
                    _ => {}
                }
                buf.push_str(rest);
            } else {
                // First segment (before any `{`) or segment with no closing `}`.
                buf.push_str(segment);
            }
        }

        let line_bytes = buf.len() as u64;
        if written + line_bytes > target_bytes {
            break;
        }

        w.write_all(buf.as_bytes())?;
        written += line_bytes;
        seq += 1;
    }

    Ok(written)
}

/// Minimal valid ELF64 header (64 bytes).
///
/// This is the smallest valid ELF binary header that tools like `readelf -h`
/// will accept.  It declares:
/// - ELF magic `\x7fELF`
/// - 64-bit, little-endian, version 1
/// - `ET_EXEC` type, `EM_X86_64` machine
/// - Entry point and section/program header fields zeroed (no segments)
const ELF64_HEADER: [u8; 64] = {
    let mut h = [0u8; 64];

    // e_ident[0..4]: magic
    h[0] = 0x7f;
    h[1] = b'E';
    h[2] = b'L';
    h[3] = b'F';

    // e_ident[4]: ELFCLASS64
    h[4] = 2;
    // e_ident[5]: ELFDATA2LSB (little-endian)
    h[5] = 1;
    // e_ident[6]: EV_CURRENT
    h[6] = 1;
    // e_ident[7]: ELFOSABI_NONE
    h[7] = 0;
    // e_ident[8..16]: padding (already zero)

    // e_type: ET_EXEC (2) — little-endian u16 at offset 16
    h[16] = 2;
    h[17] = 0;

    // e_machine: EM_X86_64 (0x3E = 62) — little-endian u16 at offset 18
    h[18] = 0x3E;
    h[19] = 0;

    // e_version: EV_CURRENT (1) — little-endian u32 at offset 20
    h[20] = 1;
    h[21] = 0;
    h[22] = 0;
    h[23] = 0;

    // e_ehsize: 64 — little-endian u16 at offset 52
    h[52] = 64;
    h[53] = 0;

    h
};

/// Write a synthetic ELF binary of approximately `target_bytes` bytes.
///
/// The first 64 bytes are a valid ELF64 little-endian header.  The
/// remainder is zero-filled, simulating a stripped binary or a large
/// `.bss` section.
///
/// The file will be exactly `max(64, target_bytes)` bytes.
///
/// # Errors
///
/// Returns `io::Error` if writing to `w` fails.
pub fn write_elf_binary(w: &mut dyn Write, target_bytes: u64) -> io::Result<u64> {
    let total = target_bytes.max(64);
    w.write_all(&ELF64_HEADER)?;

    let remaining = total - 64;
    write_zeros(w, remaining)?;

    Ok(total)
}

/// Write `count` zero bytes efficiently using a fixed-size buffer.
fn write_zeros(w: &mut dyn Write, count: u64) -> io::Result<()> {
    const BUF_SIZE: usize = 64 * 1024; // 64 KiB chunks
    #[allow(clippy::large_stack_arrays)]
    let zeros = [0u8; BUF_SIZE];

    let full_chunks = count / BUF_SIZE as u64;
    // Tail is always < BUF_SIZE (64 KiB), fits in usize on all platforms.
    #[allow(clippy::cast_possible_truncation)]
    let tail = (count % BUF_SIZE as u64) as usize;

    for _ in 0..full_chunks {
        w.write_all(&zeros)?;
    }
    if tail > 0 {
        w.write_all(&zeros[..tail])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "large_file_tests.rs"]
mod tests;
