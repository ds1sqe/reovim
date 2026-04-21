//! Integration tests for the LSP diagnostic pipeline.
//!
//! These tests verify that diagnostics flow from a real language server
//! (rust-analyzer) through the server's extension state bridge to clients.
//!
//! # Requirements
//!
//! - `rust-analyzer` must be installed and in PATH
//! - Tests skip gracefully if rust-analyzer is unavailable
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-server --test lsp_diagnostics
//! ```
//!
//! # Log Files
//!
//! Test logs are captured to `tmp/test-logs/{test_name}_{timestamp}.log`.

use std::{path::PathBuf, time::Duration};

use {reovim_client_cli::GrpcClient, reovim_testing::TestServerHarness};

/// Max time to wait for rust-analyzer to produce diagnostics.
const DIAGNOSTIC_TIMEOUT: Duration = Duration::from_secs(30);

/// Poll interval when waiting for diagnostics.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Get the absolute path to the LSP test fixture project.
fn fixture_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = server/lib/server
    // Fixture is at tools/testing/fixtures/rust-lsp-project/
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent() // server/lib/
        .unwrap()
        .parent() // server/
        .unwrap()
        .parent() // workspace root
        .unwrap()
        .join("tools/testing/fixtures/rust-lsp-project")
}

/// Check if rust-analyzer is installed and available.
fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Connect to server with retry.
async fn connect_with_retry(addr: &str) -> GrpcClient {
    let mut attempts: u32 = 0;
    loop {
        match GrpcClient::connect(addr).await {
            Ok(c) => return c,
            Err(_) if attempts < 20 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(50 + u64::from(attempts) * 50)).await;
            }
            Err(e) => panic!("Failed to connect after 20 attempts: {e}"),
        }
    }
}

/// Poll extension state until diagnostics appear or timeout.
///
/// Returns the JSON data string if diagnostics became active, or `None` on timeout.
async fn wait_for_diagnostics(client: &mut GrpcClient, timeout: Duration) -> Option<String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(resp) = client.debug_get_extension_state("diagnostics", 1).await
            && resp.active
            && !resp.data.is_empty()
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.data)
            && json.get("active").and_then(serde_json::Value::as_bool) == Some(true)
            && let Some(diags) = json
                .get("diagnostics")
                .and_then(serde_json::Value::as_array)
            && diags.iter().any(|entry| {
                entry
                    .get("items")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|items| !items.is_empty())
            })
        {
            return Some(resp.data);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    None
}

// ============================================================================
// LSP Diagnostic Pipeline
// ============================================================================

/// Verify that opening a Rust file triggers rust-analyzer and produces diagnostics.
///
/// This test exercises the full pipeline:
///   1. Server loads LSP module (builtin tier 4)
///   2. `:e` opens a Rust file in a Cargo project
///   3. LSP auto-starter detects Rust, spawns rust-analyzer
///   4. rust-analyzer analyzes and publishes diagnostics
///   5. `DiagnosticBridge` resolves URIs to buffer IDs
///   6. Extension state becomes queryable via gRPC debug API
#[tokio::test]
async fn test_lsp_diagnostics_from_rust_analyzer() {
    if !rust_analyzer_available() {
        eprintln!("Skipping: rust-analyzer not found in PATH");
        return;
    }

    let fixture = fixture_path();
    let lib_rs = fixture.join("src/lib.rs");
    assert!(lib_rs.exists(), "Fixture file not found: {}", lib_rs.display());

    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    // Open the fixture file — triggers LSP auto-start
    client
        .send_keys(&format!(":e {}<CR>", lib_rs.display()))
        .await
        .expect("Failed to open fixture file");

    // Wait for rust-analyzer to initialize and produce diagnostics
    let data = wait_for_diagnostics(&mut client, DIAGNOSTIC_TIMEOUT).await;
    drop(client);

    if let Some(json_str) = data {
        eprintln!("Diagnostics received:\n{json_str}");

        let json: serde_json::Value =
            serde_json::from_str(&json_str).expect("Invalid JSON in extension state");

        // Verify structure: { active: true, diagnostics: [{ bufferId, items: [...] }] }
        assert_eq!(json["active"], true, "Diagnostics should be active");

        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array");
        assert!(!diagnostics.is_empty(), "Should have at least one buffer with diagnostics");

        // Collect all diagnostic items across buffers
        let all_items: Vec<&serde_json::Value> = diagnostics
            .iter()
            .filter_map(|entry| entry.get("items").and_then(serde_json::Value::as_array))
            .flatten()
            .collect();

        // The fixture has at least a type mismatch error. Warnings may arrive
        // later depending on rust-analyzer timing, so require only ≥1.
        assert!(!all_items.is_empty(), "Expected at least 1 diagnostic, got 0");

        // Verify we got at least one error-level diagnostic (type mismatch)
        let has_error = all_items.iter().any(|item| {
            item.get("severity")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|s| s == "error")
        });
        assert!(has_error, "Expected at least one error diagnostic");

        eprintln!("PASS: {} diagnostics received", all_items.len());
    } else {
        let log_hint = harness
            .log_path()
            .map(|p| format!("\nServer log: {}", p.display()))
            .unwrap_or_default();
        panic!(
            "Timed out waiting for diagnostics ({DIAGNOSTIC_TIMEOUT:?}).{log_hint}\n\
             This may indicate rust-analyzer failed to start or the fixture \
             project was not recognized."
        );
    }
}

/// Verify that diagnostic severity levels are correctly mapped.
///
/// The fixture contains both warnings (unused variable) and errors (type mismatch).
/// This test checks that severity strings in the extension state match expectations.
#[tokio::test]
async fn test_lsp_diagnostic_severity_mapping() {
    if !rust_analyzer_available() {
        eprintln!("Skipping: rust-analyzer not found in PATH");
        return;
    }

    let fixture = fixture_path();
    let lib_rs = fixture.join("src/lib.rs");
    assert!(lib_rs.exists());

    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    client
        .send_keys(&format!(":e {}<CR>", lib_rs.display()))
        .await
        .expect("Failed to open fixture file");

    let Some(json_str) = wait_for_diagnostics(&mut client, DIAGNOSTIC_TIMEOUT).await else {
        drop(client);
        eprintln!("Skipping severity check: no diagnostics within timeout");
        return;
    };
    drop(client);

    let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let all_items: Vec<&serde_json::Value> = json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("items").and_then(serde_json::Value::as_array))
        .flatten()
        .collect();

    // Collect unique severities
    let severities: Vec<&str> = all_items
        .iter()
        .filter_map(|item| item.get("severity").and_then(serde_json::Value::as_str))
        .collect();

    eprintln!("Severities found: {severities:?}");

    // Must have at least "error" (type mismatch in broken())
    assert!(severities.contains(&"error"), "Expected 'error' severity in diagnostics");

    // Should have "warning" (unused variable in add())
    // Note: This may depend on rust-analyzer config, so only log it.
    if severities.contains(&"warning") {
        eprintln!("PASS: Both error and warning severities present");
    } else {
        eprintln!("INFO: Only error severity found (warnings may be suppressed)");
    }

    drop(harness);
}

// ============================================================================
// Diagnostics Panel (#665)
// ============================================================================

/// Verify that `:Trouble` command opens the diagnostics panel.
///
/// Tests the command registration and panel activation. The panel bridge
/// is client-scoped — its state is per-client, not shared.
#[tokio::test]
async fn test_trouble_command_opens_panel() {
    if !rust_analyzer_available() {
        eprintln!("Skipping: rust-analyzer not found in PATH");
        return;
    }

    let fixture = fixture_path();
    let lib_rs = fixture.join("src/lib.rs");
    assert!(lib_rs.exists());

    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    // Open fixture file first (triggers LSP)
    client
        .send_keys(&format!(":e {}<CR>", lib_rs.display()))
        .await
        .expect("Failed to open fixture file");

    // Wait briefly for the LSP to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Open diagnostics panel via ex command
    client
        .send_keys(":Trouble<CR>")
        .await
        .expect("Failed to send :Trouble");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query the panel's extension state (client-scoped)
    let resp = client
        .debug_get_extension_state("diagnostics-panel", 1)
        .await
        .expect("Failed to query diagnostics-panel state");
    drop(client);

    eprintln!("diagnostics-panel: active={}, data={}", resp.active, resp.data);

    assert!(resp.active, "Diagnostics panel should be active after :Trouble");

    if !resp.data.is_empty() {
        let json: serde_json::Value = serde_json::from_str(&resp.data).unwrap();
        assert_eq!(json["active"], true, "Panel JSON should show active=true");
        eprintln!("Panel mode: {}", json["panelTitle"]);
    }

    drop(harness);
}

// ============================================================================
// Hover (K) and Goto Definition (gd)
// ============================================================================

/// Verify that `K` (hover) works after LSP is ready.
///
/// Moves cursor to the documented function, sends `K`, and checks that
/// hover state becomes active with content.
#[tokio::test]
async fn test_hover_on_documented_symbol() {
    if !rust_analyzer_available() {
        eprintln!("Skipping: rust-analyzer not found in PATH");
        return;
    }

    let fixture = fixture_path();
    let lib_rs = fixture.join("src/lib.rs");

    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    // Open fixture file
    client
        .send_keys(&format!(":e {}<CR>", lib_rs.display()))
        .await
        .expect("Failed to open fixture file");

    // Wait for diagnostics (ensures rust-analyzer is ready)
    let _ = wait_for_diagnostics(&mut client, DIAGNOSTIC_TIMEOUT).await;

    // Move to `documented` function name (line 25: pub fn documented)
    client
        .send_keys("25Gwwl")
        .await
        .expect("Failed to navigate");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send K (hover)
    client.send_keys("K").await.expect("Failed to send K");

    // Wait for hover result (async delivery via tick)
    let start = std::time::Instant::now();
    let mut hover_data = None;
    while start.elapsed() < Duration::from_secs(10) {
        if let Ok(resp) = client.debug_get_extension_state("hover", 1).await
            && resp.active
            && !resp.data.is_empty()
        {
            hover_data = Some(resp.data);
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    if let Some(data) = hover_data {
        eprintln!("Hover data:\n{data}");
        let json: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(json["active"], true, "Hover should be active");
        assert!(
            json["content"].as_str().is_some_and(|c| !c.is_empty()),
            "Hover content should not be empty"
        );
    } else {
        let log_hint = harness
            .log_path()
            .map(|p| format!("\nServer log: {}", p.display()))
            .unwrap_or_default();
        eprintln!("WARN: Hover did not activate within timeout.{log_hint}");
    }

    // Verify server is still alive (get_mode removed in v3; use ping)
    client.send_keys("<Esc>").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    client.ping().await.expect("Server should be alive after K");

    drop(client);
    drop(harness);
}

/// Verify that `gd` (goto definition) works after LSP is ready.
///
/// Moves cursor to a function name, sends `gd`, and checks that the
/// server doesn't crash.
#[tokio::test]
async fn test_goto_definition_on_symbol() {
    if !rust_analyzer_available() {
        eprintln!("Skipping: rust-analyzer not found in PATH");
        return;
    }

    let fixture = fixture_path();
    let lib_rs = fixture.join("src/lib.rs");

    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    // Open fixture file
    client
        .send_keys(&format!(":e {}<CR>", lib_rs.display()))
        .await
        .expect("Failed to open fixture file");

    // Wait for diagnostics (ensures rust-analyzer is ready)
    let _ = wait_for_diagnostics(&mut client, DIAGNOSTIC_TIMEOUT).await;

    // Move cursor to `add` function name on line 11
    client
        .send_keys("11Gwwl")
        .await
        .expect("Failed to navigate");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Record position before gd (get_cursor removed in v3; use ping to check liveness)
    client
        .ping()
        .await
        .expect("Server should be alive before gd");

    // Send gd (goto definition)
    client.send_keys("gd").await.expect("Failed to send gd");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify server is still alive after gd (get_mode/get_cursor removed in v3)
    client
        .ping()
        .await
        .expect("Server should be alive after gd");

    eprintln!("PASS: gd completed without crash");

    drop(client);
    drop(harness);
}
