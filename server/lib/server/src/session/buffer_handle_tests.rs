use std::sync::Arc;

use {
    parking_lot::RwLock, reovim_driver_buffer::BufferCapabilities, reovim_kernel::api::v1::Buffer,
};

use super::BufferHandle;

#[test]
fn rope_capabilities() {
    let buf = Buffer::from_string("hello");
    let handle = BufferHandle::Rope(Arc::new(RwLock::new(buf)));
    assert_eq!(handle.capabilities(), BufferCapabilities::ROPE);
}

#[test]
fn rope_line_count() {
    let buf = Buffer::from_string("line1\nline2\nline3");
    let handle = BufferHandle::Rope(Arc::new(RwLock::new(buf)));
    assert_eq!(handle.line_count(), 3);
}

#[test]
fn rope_line() {
    let buf = Buffer::from_string("hello\nworld");
    let handle = BufferHandle::Rope(Arc::new(RwLock::new(buf)));
    assert_eq!(handle.line(0), Some("hello".to_string()));
    assert_eq!(handle.line(1), Some("world".to_string()));
    assert_eq!(handle.line(2), None);
}

#[test]
fn rope_file_path() {
    let mut buf = Buffer::from_string("test");
    buf.set_file_path(Some("test.txt".to_string()));
    let handle = BufferHandle::Rope(Arc::new(RwLock::new(buf)));
    assert_eq!(handle.file_path(), Some("test.txt".to_string()));
}

#[test]
fn rope_no_file_path() {
    let buf = Buffer::from_string("test");
    let handle = BufferHandle::Rope(Arc::new(RwLock::new(buf)));
    assert_eq!(handle.file_path(), None);
}

#[test]
fn rope_is_modified() {
    let buf = Buffer::from_string("test");
    let handle = BufferHandle::Rope(Arc::new(RwLock::new(buf)));
    // from_string starts as not modified
    assert!(!handle.is_modified());
}

#[test]
fn rope_content() {
    let buf = Buffer::from_string("hello\nworld");
    let handle = BufferHandle::Rope(Arc::new(RwLock::new(buf)));
    assert_eq!(handle.content(), "hello\nworld");
}
