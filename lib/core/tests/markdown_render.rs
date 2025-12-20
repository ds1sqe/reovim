//! Markdown rendering capture tests
//!
//! Tests that verify markdown decorations (concealment, backgrounds) are
//! properly applied during rendering.

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
// Markdown heading tests
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

/// Test that heading markers are concealed (# replaced with icon)
#[tokio::test]
async fn test_heading_marker_concealed() {
    let md_content = "# Heading One\n\nText after heading.\n";
    let file = create_markdown_file(md_content);

    let harness = ServerTestHarness::spawn_with_file(file.path().to_str().unwrap())
        .await
        .expect("Failed to spawn server with file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 24).await.expect("Failed to resize");

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let snap = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    // The heading text should be visible
    assert!(
        snap.plain_text.contains("Heading One"),
        "Should contain heading text, got:\n{}",
        snap.plain_text
    );

    // Check for concealment - the raw "# " should NOT appear (should be replaced with icon)
    let has_raw_marker = snap
        .plain_text
        .lines()
        .any(|l| l.contains("# Heading") && !l.contains("󰉫"));

    assert!(!has_raw_marker, "Heading marker should be concealed, got:\n{}", snap.plain_text);

    // Verify the icon is present in the first row
    let first_row = snap.cells.first().expect("Should have first row");
    let has_icon = first_row.iter().any(|c| c.char == '󰉫');
    assert!(has_icon, "First row should contain heading icon (󰉫)");
}

/// Test that inline emphasis markers are concealed
#[tokio::test]
async fn test_inline_emphasis_concealed() {
    let md_content = "# Test\n\nThis is *italic* and **bold** text.\n";
    let file = create_markdown_file(md_content);

    let harness = ServerTestHarness::spawn_with_file(file.path().to_str().unwrap())
        .await
        .expect("Failed to spawn server with file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 24).await.expect("Failed to resize");

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let snap = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    println!("=== Visual Snapshot ===");
    println!("{}", snap.plain_text);
    println!("=======================");

    // The content should be visible
    assert!(
        snap.plain_text.contains("italic"),
        "Should contain 'italic', got:\n{}",
        snap.plain_text
    );
    assert!(
        snap.plain_text.contains("bold"),
        "Should contain 'bold', got:\n{}",
        snap.plain_text
    );

    // Check that emphasis markers are hidden
    // The rendered line should have "italic" without the surrounding asterisks
    let text_line = snap.plain_text.lines().find(|l| l.contains("italic"));
    if let Some(line) = text_line {
        // After concealment, we should see "italic" but not "*italic*"
        let has_asterisk_around = line.contains("*italic*") || line.contains("**bold**");
        assert!(!has_asterisk_around, "Emphasis markers should be concealed, got: {line}");
    }
}

/// Test that link syntax is concealed
#[tokio::test]
async fn test_inline_link_concealed() {
    let md_content = "# Test\n\nClick [here](http://example.com) for more.\n";
    let file = create_markdown_file(md_content);

    let harness = ServerTestHarness::spawn_with_file(file.path().to_str().unwrap())
        .await
        .expect("Failed to spawn server with file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 24).await.expect("Failed to resize");

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let snap = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    println!("=== Link Snapshot ===");
    println!("{}", snap.plain_text);
    println!("=====================");

    // The link text should be visible
    assert!(
        snap.plain_text.contains("here"),
        "Should contain link text 'here', got:\n{}",
        snap.plain_text
    );

    // The URL should be hidden (concealed)
    let has_visible_url = snap.plain_text.contains("example.com");
    assert!(!has_visible_url, "Link URL should be concealed, got:\n{}", snap.plain_text);
}

/// Test that insert mode shows raw markdown (no concealment)
#[tokio::test]
async fn test_insert_mode_shows_raw() {
    let md_content = "# Test\n\nThis is *italic* text.\n";
    let file = create_markdown_file(md_content);

    let harness = ServerTestHarness::spawn_with_file(file.path().to_str().unwrap())
        .await
        .expect("Failed to spawn server with file");

    let mut client = harness.client().await.expect("Failed to connect");
    client.resize(80, 24).await.expect("Failed to resize");

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // First check normal mode - emphasis should be concealed
    let snap_normal = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    println!("=== Normal Mode ===");
    println!("{}", snap_normal.plain_text);

    // In normal mode, asterisks should be hidden
    let has_asterisks_normal = snap_normal
        .plain_text
        .lines()
        .any(|l| l.contains("*italic*"));
    assert!(
        !has_asterisks_normal,
        "Normal mode should conceal asterisks, got:\n{}",
        snap_normal.plain_text
    );

    // Enter insert mode (press 'i')
    client.keys("i").await.expect("Failed to send keys");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Capture in insert mode
    let snap_insert = client
        .visual_snapshot()
        .await
        .expect("Failed to capture snapshot");

    println!("=== Insert Mode ===");
    println!("{}", snap_insert.plain_text);

    // In insert mode, raw markdown should be visible (asterisks shown)
    let has_asterisks_insert = snap_insert
        .plain_text
        .lines()
        .any(|l| l.contains("*italic*"));
    assert!(
        has_asterisks_insert,
        "Insert mode should show raw markdown with asterisks, got:\n{}",
        snap_insert.plain_text
    );
}

// ============================================================================
// Comprehensive test using test_markdown.md
// ============================================================================

/// Test comprehensive markdown rendering with the test file containing all tags
#[tokio::test]
async fn test_comprehensive_markdown_file() {
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

    println!("=== Comprehensive Markdown Snapshot ===");
    println!("{}", snap.plain_text);
    println!("========================================");

    // Heading 1 should be visible with icon (# concealed)
    assert!(
        snap.plain_text.contains("Heading 1"),
        "Should contain Heading 1"
    );

    // Inline formatting - text visible, markers hidden
    assert!(
        snap.plain_text.contains("italic"),
        "Should contain italic text"
    );
    assert!(
        snap.plain_text.contains("bold"),
        "Should contain bold text"
    );

    // Check emphasis markers are concealed
    let paragraph_line = snap.plain_text.lines().find(|l| l.contains("italic"));
    if let Some(line) = paragraph_line {
        assert!(
            !line.contains("*italic*"),
            "Emphasis markers should be concealed"
        );
        assert!(
            !line.contains("**bold**"),
            "Strong markers should be concealed"
        );
    }

    // Link text visible, URL hidden
    assert!(snap.plain_text.contains("here"), "Should contain link text");
    assert!(
        !snap.plain_text.contains("example.com"),
        "Link URL should be concealed"
    );

    // List items visible
    assert!(
        snap.plain_text.contains("First item"),
        "Should contain list items"
    );
    assert!(
        snap.plain_text.contains("Second item"),
        "Should contain list items"
    );

    // Numbered list
    assert!(
        snap.plain_text.contains("Numbered one"),
        "Should contain numbered list"
    );

    // Code block content visible
    assert!(
        snap.plain_text.contains("fn main()"),
        "Should contain code block"
    );

    // Table content visible
    assert!(
        snap.plain_text.contains("Header 1"),
        "Should contain table header"
    );
    assert!(
        snap.plain_text.contains("Cell 1"),
        "Should contain table cells"
    );

    // Blockquote visible
    assert!(
        snap.plain_text.contains("blockquote"),
        "Should contain blockquote"
    );

    // TODO: Test inline code styling (background highlight)
    // TODO: Test strikethrough concealment (hide ~~ markers)
    // TODO: Test image alt text concealment
    // Note: These elements may be below viewport in 40-line window

    // Verify heading icons are present
    let first_row = snap.cells.first().expect("Should have first row");
    let has_h1_icon = first_row.iter().any(|c| c.char == '󰉫');
    assert!(has_h1_icon, "First row should contain H1 icon (󰉫)");
}

/// Test that all heading levels have proper icons
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
        assert!(
            snap.plain_text.contains(&heading_text),
            "Should contain {heading_text}"
        );
    }

    // Raw heading markers should be concealed
    for i in 1..=6 {
        let raw_marker = format!("{} Heading", "#".repeat(i));
        let has_raw = snap.plain_text.lines().any(|l| l.starts_with(&raw_marker));
        assert!(
            !has_raw,
            "H{i} marker should be concealed, should not start with '{raw_marker}'"
        );
    }
}
