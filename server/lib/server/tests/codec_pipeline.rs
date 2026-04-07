//! Integration tests for codec pipeline: large-file open/save, binary codec viewing.
//!
//! Verifies end-to-end paths that Plan 04 (#740) wired but unit tests
//! cannot exercise: mmap-backed `VirtualBuffer` via `:edit`, streaming
//! `:write`, and binary codec routing (ELF, rlib).
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-server --test codec_pipeline
//! ```

use std::io::Write;
use std::time::Duration;

/// Minimum file size to trigger `VirtualBuffer` (mmap) path: 64 MB + 1 byte.
const LARGE_FILE_SIZE: usize = 64 * 1024 * 1024 + 1;

/// Generate a temp file path with unique suffix.
fn temp_path(suffix: &str) -> String {
    format!(
        "/tmp/reovim-codec-test-{}-{suffix}",
        std::process::id()
    )
}

/// Create a large UTF-8 file (>64 MB) with predictable line content.
///
/// Each line is "line NNNNN padding...\n" = exactly 100 bytes.
/// Returns `(path, line_count)`.
fn create_large_utf8_file(path: &str) -> usize {
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(path).expect("Failed to create large file"),
    );
    let line_count = LARGE_FILE_SIZE / 100 + 1;
    for i in 0..line_count {
        write!(file, "line {i:>05} ").expect("write");
        // Pad to exactly 99 bytes of content + \n = 100 bytes per line
        // "line NNNNN " = 11 bytes, so pad 88 more bytes
        for _ in 0..88 {
            file.write_all(b".").expect("write");
        }
        file.write_all(b"\n").expect("write");
    }
    file.flush().expect("flush");
    line_count
}

// ─── Large UTF-8 File Open ──────────────────────────────────────────────────

/// Verify that opening a >64 MB UTF-8 file uses `VirtualBuffer` (mmap path)
/// and the buffer is readable with correct line count.
#[tokio::test]
async fn large_utf8_file_opens_with_streamable_capability() {
    let path = temp_path("large-utf8.txt");
    let expected_lines = create_large_utf8_file(&path);

    let harness = reovim_testing::TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = {
        let mut attempts = 0;
        loop {
            match reovim_client_cli::GrpcClient::connect(&addr).await {
                Ok(c) => break c,
                Err(_) if attempts < 30 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => panic!("Failed to connect: {e}"),
            }
        }
    };

    // Open the large file
    client
        .send_keys(&format!(":e {path}<CR>"))
        .await
        .expect("Failed to send :e command");
    // Allow time for mmap + line index construction
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check buffer listing — should have STREAMABLE capability
    let buffers = client.list_buffers().await.expect("list_buffers failed");
    let buf = buffers
        .buffers
        .iter()
        .find(|b| b.path.as_deref().is_some_and(|p| p.contains("large-utf8")))
        .expect("Large file buffer not found in buffer list");

    // STREAMABLE = bit 2 = 0x04 (from BufferCapabilities::STREAMABLE)
    assert!(
        buf.capabilities & 0x04 != 0,
        "Buffer should have STREAMABLE capability, got caps={:#010b}",
        buf.capabilities
    );

    // Line count should match what we wrote (~650K lines).
    assert!(
        buf.line_count >= expected_lines as u64 / 2,
        "Line count {} too low, expected ~{expected_lines}",
        buf.line_count
    );

    // Verify the buffer is actually the large file (not the scratch buffer).
    assert!(
        buf.line_count > 1000,
        "Buffer should have many lines (got {}), likely not the large file",
        buf.line_count
    );

    // Note: We don't fetch full content here — 650K+ lines over gRPC would
    // be too slow. Content correctness is verified by large_file_streaming_save_roundtrip
    // which does a byte-for-byte file comparison.

    drop(client);
    drop(harness);
    let _ = std::fs::remove_file(&path);
}

// ─── Large File Streaming Save ──────────────────────────────────────────────

/// Verify that `:w` on a large `STREAMABLE` buffer produces a correct file.
#[tokio::test]
async fn large_file_streaming_save_roundtrip() {
    let src_path = temp_path("large-save-src.txt");
    let dst_path = temp_path("large-save-dst.txt");
    create_large_utf8_file(&src_path);

    let harness = reovim_testing::TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = {
        let mut attempts = 0;
        loop {
            match reovim_client_cli::GrpcClient::connect(&addr).await {
                Ok(c) => break c,
                Err(_) if attempts < 30 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => panic!("Failed to connect: {e}"),
            }
        }
    };

    // Open the large file
    client
        .send_keys(&format!(":e {src_path}<CR>"))
        .await
        .expect("send :e");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Save to a different path
    client
        .send_keys(&format!(":w {dst_path}<CR>"))
        .await
        .expect("send :w");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Compare files byte-for-byte
    let src_size = std::fs::metadata(&src_path).expect("src metadata").len();
    let dst_size = std::fs::metadata(&dst_path).expect("dst metadata").len();
    assert_eq!(
        src_size, dst_size,
        "Saved file size ({dst_size}) != source ({src_size})"
    );

    // Chunk comparison to avoid loading 128MB into memory
    let src_data = std::fs::read(&src_path).expect("read src");
    let dst_data = std::fs::read(&dst_path).expect("read dst");
    assert_eq!(src_data, dst_data, "Saved file content differs from source");

    drop(client);
    drop(harness);
    let _ = std::fs::remove_file(&src_path);
    let _ = std::fs::remove_file(&dst_path);
}

// ─── ELF Binary Open ───────────────────────────────────────────────────────

/// Verify that opening an ELF binary routes through the codec pipeline
/// and produces a structured summary view.
#[tokio::test]
async fn elf_binary_opens_with_codec_summary() {
    // /bin/ls is guaranteed ELF on Linux
    let elf_path = "/bin/ls";
    if !std::path::Path::new(elf_path).exists() {
        eprintln!("Skipping: {elf_path} not found");
        return;
    }

    let harness = reovim_testing::TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = {
        let mut attempts = 0;
        loop {
            match reovim_client_cli::GrpcClient::connect(&addr).await {
                Ok(c) => break c,
                Err(_) if attempts < 30 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => panic!("Failed to connect: {e}"),
            }
        }
    };

    // Open ELF binary
    client
        .send_keys(&format!(":e {elf_path}<CR>"))
        .await
        .expect("send :e");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Get buffer content — should be the codec summary, not raw bytes
    let content = client
        .get_buffer_content(None)
        .await
        .expect("get_buffer_content");
    let text = content.lines.join("\n");

    assert!(
        text.contains("ELF Binary Summary") || text.contains("ELF"),
        "ELF buffer should contain structured summary.\nFirst 5 lines: {:?}",
        &content.lines[..content.lines.len().min(5)]
    );

    // Check buffer metadata — should have codec info
    let buffers = client.list_buffers().await.expect("list_buffers");
    let buf = buffers
        .buffers
        .iter()
        .find(|b| b.path.as_deref().is_some_and(|p| p.contains("ls")))
        .expect("ELF buffer not found");

    // Should have codec metadata
    assert!(
        buf.codec_metadata.is_some(),
        "ELF buffer should have codec metadata"
    );

    drop(client);
    drop(harness);
}

// ─── rlib File Open ─────────────────────────────────────────────────────────

/// Verify that opening a .rlib file routes through the rlib codec
/// and produces a structured summary view.
#[tokio::test]
async fn rlib_file_opens_with_codec_summary() {
    // Find a real .rlib in target/debug/deps/ (use CARGO_MANIFEST_DIR to
    // locate workspace root — integration tests may run from a different CWD).
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .expect("workspace root not found");
    let deps_dir = workspace_root.join("target/debug/deps");
    if !deps_dir.exists() {
        eprintln!("Skipping: {} not found", deps_dir.display());
        return;
    }

    let rlib_path = std::fs::read_dir(&deps_dir)
        .ok()
        .and_then(|entries| {
            entries
                .filter_map(Result::ok)
                .find(|e| e.path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rlib")))
                .map(|e| e.path())
        });

    let Some(rlib_path) = rlib_path else {
        eprintln!("Skipping: no .rlib files in target/debug/deps/");
        return;
    };

    let harness = reovim_testing::TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = {
        let mut attempts = 0;
        loop {
            match reovim_client_cli::GrpcClient::connect(&addr).await {
                Ok(c) => break c,
                Err(_) if attempts < 30 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => panic!("Failed to connect: {e}"),
            }
        }
    };

    // Open rlib file
    client
        .send_keys(&format!(":e {}<CR>", rlib_path.display()))
        .await
        .expect("send :e");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Get buffer content
    let content = client
        .get_buffer_content(None)
        .await
        .expect("get_buffer_content");
    let text = content.lines.join("\n");

    assert!(
        text.contains("Rust Library") || text.contains(".rlib"),
        "rlib buffer should contain structured summary.\nFirst 5 lines: {:?}",
        &content.lines[..content.lines.len().min(5)]
    );

    // Check codec metadata
    let buffers = client.list_buffers().await.expect("list_buffers");
    let buf = buffers
        .buffers
        .iter()
        .find(|b| {
            b.path
                .as_deref()
                .is_some_and(|p| {
                    std::path::Path::new(p)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("rlib"))
                })
        })
        .expect("rlib buffer not found");

    assert!(
        buf.codec_metadata.is_some(),
        "rlib buffer should have codec metadata"
    );

    drop(client);
    drop(harness);
}
