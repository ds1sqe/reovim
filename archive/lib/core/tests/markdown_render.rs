//! Markdown rendering capture tests
//!
//! Tests that verify markdown files are properly rendered.
//! Note: Decoration/concealment tests have been moved to integration tests
//! since they require the treesitter plugin to be fully integrated.

use {reovim_core::testing::ServerTestHarness, std::io::Write, tempfile::NamedTempFile};

/// Path to the comprehensive test markdown file
const TEST_MARKDOWN_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test_markdown.md");

/// Create a temporary markdown file with given content
fn create_markdown_file(content: &str) -> NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".md")
        .tempfile()
        .expect("Failed to create temp file");
    file.write_all(content.as_bytes())
        .expect("Failed to write content");
    file.flush().expect("Failed to flush");
    file
}

// ============================================================================
// Markdown file loading tests
// ============================================================================

/// Test that markdown headings are rendered (file opens correctly)
#[tokio::test]
async fn test_markdown_file_opens() {
    let md_content = "# Heading 1\n\nSome text here.\n";
    let file = create_markdown_file(md_content);

    let harness = ServerTestHarness::spawn_with_file(file.path().to_str().unwrap())
        .await
        .expect("Failed to spawn server with file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 24).await.expect("Failed to resize");

    // Wait for render
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let snap = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    // The heading text should be visible
    assert!(
        snap.plain_text.contains("Heading 1"),
        "Should contain heading text, got:\n{}",
        snap.plain_text
    );
}

/// Test that markdown list items are rendered
#[tokio::test]
async fn test_markdown_list_renders() {
    let md_content = "# List Test\n\n- First item\n- Second item\n- Third item\n";
    let file = create_markdown_file(md_content);

    let harness = ServerTestHarness::spawn_with_file(file.path().to_str().unwrap())
        .await
        .expect("Failed to spawn server with file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 24).await.expect("Failed to resize");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let snap = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    // List items should be visible
    assert!(
        snap.plain_text.contains("First item"),
        "Should contain list items, got:\n{}",
        snap.plain_text
    );
    assert!(snap.plain_text.contains("Second item"), "Should contain list items");
}

/// Test that code blocks are rendered
#[tokio::test]
async fn test_markdown_code_block_renders() {
    let md_content = "# Code Test\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n";
    let file = create_markdown_file(md_content);

    let harness = ServerTestHarness::spawn_with_file(file.path().to_str().unwrap())
        .await
        .expect("Failed to spawn server with file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 24).await.expect("Failed to resize");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let snap = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    // Code should be visible
    assert!(
        snap.plain_text.contains("fn main()"),
        "Should contain code, got:\n{}",
        snap.plain_text
    );
}

/// Test that all heading levels are visible
#[tokio::test]
async fn test_all_heading_levels() {
    let harness = ServerTestHarness::spawn_with_file(TEST_MARKDOWN_FILE)
        .await
        .expect("Failed to spawn server with test markdown file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 40).await.expect("Failed to resize");

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let snap = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    // All heading texts should be visible
    for i in 1..=6 {
        let heading_text = format!("Heading {i}");
        assert!(snap.plain_text.contains(&heading_text), "Should contain {heading_text}");
    }
}
