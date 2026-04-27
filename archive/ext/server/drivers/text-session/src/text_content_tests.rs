use std::sync::Arc;

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::BufferId,
    reovim_provider_text::{Buffer, TextBufferRegistry},
    reovim_subsys_session::BufferContentProvider,
};

use super::*;

fn setup_registry_with_buffer(content: &str) -> (Arc<TextBufferRegistry>, BufferId) {
    let registry = Arc::new(TextBufferRegistry::new());
    let buffer = Buffer::from_string(content);
    let buffer_id = buffer.id();
    registry.register(Arc::new(RwLock::new(buffer)));
    (registry, buffer_id)
}

#[test]
fn test_content_bytes() {
    let (registry, buffer_id) = setup_registry_with_buffer("hello world");
    let provider = TextContentProvider::new(registry);

    let bytes = provider.content_bytes(buffer_id).unwrap();
    assert_eq!(bytes, b"hello world");
}

#[test]
fn test_content_bytes_nonexistent_buffer() {
    let registry = Arc::new(TextBufferRegistry::new());
    let provider = TextContentProvider::new(registry);
    let fake_id = BufferId::new();

    assert!(provider.content_bytes(fake_id).is_none());
}

#[test]
fn test_content_size() {
    let (registry, buffer_id) = setup_registry_with_buffer("hello");
    let provider = TextContentProvider::new(registry);

    let size = provider.content_size(buffer_id).unwrap();
    assert_eq!(size, 5);
}

#[test]
fn test_content_size_nonexistent() {
    let registry = Arc::new(TextBufferRegistry::new());
    let provider = TextContentProvider::new(registry);

    assert!(provider.content_size(BufferId::new()).is_none());
}

#[test]
fn test_content_unit_count() {
    let (registry, buffer_id) = setup_registry_with_buffer("line1\nline2\nline3");
    let provider = TextContentProvider::new(registry);

    let count = provider.content_unit_count(buffer_id).unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_content_unit_count_single_line() {
    let (registry, buffer_id) = setup_registry_with_buffer("hello");
    let provider = TextContentProvider::new(registry);

    let count = provider.content_unit_count(buffer_id).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_content_unit_count_nonexistent() {
    let registry = Arc::new(TextBufferRegistry::new());
    let provider = TextContentProvider::new(registry);

    assert!(provider.content_unit_count(BufferId::new()).is_none());
}

#[test]
fn test_display_lines_full_viewport() {
    let (registry, buffer_id) = setup_registry_with_buffer("line1\nline2\nline3");
    let provider = TextContentProvider::new(registry);

    let lines = provider.display_lines(buffer_id, 0, 10).unwrap();

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].content, "line1");
    assert_eq!(lines[0].unit_index, 0);
    assert_eq!(lines[1].content, "line2");
    assert_eq!(lines[1].unit_index, 1);
    assert_eq!(lines[2].content, "line3");
    assert_eq!(lines[2].unit_index, 2);
}

#[test]
fn test_display_lines_partial_viewport() {
    let (registry, buffer_id) = setup_registry_with_buffer("a\nb\nc\nd\ne");
    let provider = TextContentProvider::new(registry);

    // Only show 2 lines
    let lines = provider.display_lines(buffer_id, 0, 2).unwrap();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].unit_index, 0);
    assert_eq!(lines[1].unit_index, 1);
}

#[test]
fn test_display_lines_scrolled() {
    let (registry, buffer_id) = setup_registry_with_buffer("a\nb\nc\nd\ne");
    let provider = TextContentProvider::new(registry);

    let lines = provider.display_lines(buffer_id, 2, 2).unwrap();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].content, "c");
    assert_eq!(lines[0].unit_index, 2);
    assert_eq!(lines[1].content, "d");
    assert_eq!(lines[1].unit_index, 3);
}

#[test]
fn test_display_lines_scrolled_past_end() {
    let (registry, buffer_id) = setup_registry_with_buffer("a\nb");
    let provider = TextContentProvider::new(registry);

    let lines = provider.display_lines(buffer_id, 1, 10).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].content, "b");
    assert_eq!(lines[0].unit_index, 1);
}

#[test]
fn test_display_lines_nonexistent() {
    let registry = Arc::new(TextBufferRegistry::new());
    let provider = TextContentProvider::new(registry);

    assert!(provider.display_lines(BufferId::new(), 0, 10).is_none());
}

#[test]
fn test_is_modified_fresh_buffer() {
    let (registry, buffer_id) = setup_registry_with_buffer("hello");
    let provider = TextContentProvider::new(registry);

    assert!(!provider.is_modified(buffer_id));
}

#[test]
fn test_is_modified_nonexistent() {
    let registry = Arc::new(TextBufferRegistry::new());
    let provider = TextContentProvider::new(registry);

    assert!(!provider.is_modified(BufferId::new()));
}

#[test]
fn test_write_to() {
    let (registry, buffer_id) = setup_registry_with_buffer("hello world");
    let provider = TextContentProvider::new(registry);

    let mut output = Vec::new();
    provider.write_to(buffer_id, &mut output).unwrap();
    assert_eq!(output, b"hello world");
}

#[test]
fn test_write_to_nonexistent() {
    let registry = Arc::new(TextBufferRegistry::new());
    let provider = TextContentProvider::new(registry);

    let mut output = Vec::new();
    // Should succeed (no-op for nonexistent buffer)
    provider.write_to(BufferId::new(), &mut output).unwrap();
    assert!(output.is_empty());
}

#[test]
fn test_object_safety() {
    let registry = Arc::new(TextBufferRegistry::new());
    let provider = TextContentProvider::new(registry);
    let _: Arc<dyn BufferContentProvider> = Arc::new(provider);
}
