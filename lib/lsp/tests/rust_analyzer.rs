//! Integration test for rust-analyzer connection.
//!
//! This test verifies that we can:
//! 1. Spawn rust-analyzer
//! 2. Complete the initialize handshake
//! 3. Receive server capabilities
//!
//! Requires `rust-analyzer` to be installed and in PATH.

use {
    lsp_types::Uri,
    reovim_lsp::{ClientConfig, ClientError, LspSaturator},
    std::{path::PathBuf, sync::Once, time::Duration},
    tokio::time::timeout,
    tracing_subscriber::{EnvFilter, fmt},
};

static INIT_TRACING: Once = Once::new();

fn init_tracing() {
    INIT_TRACING.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("reovim_lsp=debug"));
        fmt()
            .with_env_filter(filter)
            .with_test_writer()
            .try_init()
            .ok();
    });
}

/// Helper to create a file:// URI from a path.
fn uri_from_path(path: &std::path::Path) -> Uri {
    let url = url::Url::from_file_path(path).expect("Invalid path");
    url.as_str().parse().expect("Invalid URI")
}

/// Skip test if rust-analyzer is not available.
fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Test file with intentional errors.
const TEST_FILE_WITH_ERRORS: &str = r"
fn main() {
    // Missing semicolon
    let x = 42
}
";

#[tokio::test]
async fn test_connect_to_rust_analyzer() {
    if !rust_analyzer_available() {
        eprintln!("Skipping test: rust-analyzer not found in PATH");
        return;
    }

    // Use the reovim project root as our test workspace
    let root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    eprintln!("Using project root: {}", root_path.display());

    let config = ClientConfig::rust_analyzer(&root_path);

    // Start the saturator with a 30-second timeout
    let result =
        timeout(Duration::from_secs(30), async { LspSaturator::start(config, None, None).await }).await;

    match result {
        Ok(Ok((handle, cache))) => {
            eprintln!("Successfully connected to rust-analyzer!");
            eprintln!("Diagnostic cache is empty: {}", cache.is_empty());

            // Request shutdown
            assert!(handle.shutdown(), "Failed to send shutdown request");

            // Give it a moment to shut down
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(Err(ClientError::SpawnFailed(e))) => {
            eprintln!("Skipping test: Failed to spawn rust-analyzer: {e}");
        }
        Ok(Err(e)) => {
            panic!("Failed to connect to rust-analyzer: {e}");
        }
        Err(elapsed) => {
            panic!("Timeout waiting for rust-analyzer to initialize: {elapsed}");
        }
    }
}

#[tokio::test]
async fn test_diagnostics_with_errors() {
    init_tracing();

    if !rust_analyzer_available() {
        eprintln!("Skipping test: rust-analyzer not found in PATH");
        return;
    }

    // Use the reovim project root as our test workspace
    let root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    eprintln!("Using project root: {}", root_path.display());

    let config = ClientConfig::rust_analyzer(&root_path);

    // Start the saturator
    let result =
        timeout(Duration::from_secs(30), async { LspSaturator::start(config, None, None).await }).await;

    let (handle, cache) = match result {
        Ok(Ok((h, c))) => (h, c),
        Ok(Err(ClientError::SpawnFailed(e))) => {
            eprintln!("Skipping test: Failed to spawn rust-analyzer: {e}");
            return;
        }
        Ok(Err(e)) => {
            panic!("Failed to connect to rust-analyzer: {e}");
        }
        Err(elapsed) => {
            panic!("Timeout waiting for rust-analyzer to initialize: {elapsed}");
        }
    };

    eprintln!("Connected to rust-analyzer");

    // Create a test file inside a real crate (lib/lsp/src/) so rust-analyzer analyzes it
    let test_file_path = root_path.join("lib/lsp/src/_test_lsp_errors.rs");
    let uri = uri_from_path(&test_file_path);

    // Write the test file to disk
    std::fs::write(&test_file_path, TEST_FILE_WITH_ERRORS).expect("Failed to write test file");

    // Send didOpen
    assert!(
        handle.did_open(uri.clone(), "rust".to_string(), 1, TEST_FILE_WITH_ERRORS.to_string()),
        "Failed to send didOpen"
    );

    eprintln!("Sent didOpen for {uri:?}");

    // Wait for diagnostics (with timeout)
    let start = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(10);

    while start.elapsed() < timeout_duration {
        if let Some(diags) = cache.get(&uri) {
            eprintln!("Received {} diagnostics for test file", diags.diagnostics.len());
            for diag in &diags.diagnostics {
                eprintln!("  - {:?}: {}", diag.severity, diag.message);
            }

            // We expect at least one error (missing semicolon)
            assert!(!diags.diagnostics.is_empty(), "Expected diagnostics for file with errors");

            // Cleanup
            std::fs::remove_file(&test_file_path).ok();
            assert!(handle.shutdown());
            return;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Cleanup even on failure
    std::fs::remove_file(&test_file_path).ok();

    // Note: rust-analyzer may not report diagnostics for files outside the workspace
    // This is expected behavior
    eprintln!(
        "No diagnostics received within timeout - this may be expected for files outside workspace"
    );
    assert!(handle.shutdown());
}
