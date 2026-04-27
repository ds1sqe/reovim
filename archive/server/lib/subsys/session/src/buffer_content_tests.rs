use std::sync::{Arc, Mutex};

use reovim_kernel::api::v1::BufferId;

use super::{BufferContentProvider, DisplayLine};

// --- Mock implementation ---

struct MockContentProvider {
    buffers: Mutex<Vec<(BufferId, Vec<u8>, bool)>>,
}

impl MockContentProvider {
    fn new() -> Self {
        Self {
            buffers: Mutex::new(Vec::new()),
        }
    }

    fn add_buffer(&self, id: BufferId, content: &[u8], modified: bool) {
        self.buffers
            .lock()
            .unwrap()
            .push((id, content.to_vec(), modified));
    }

    fn find(&self, buffer_id: BufferId) -> Option<(Vec<u8>, bool)> {
        self.buffers
            .lock()
            .unwrap()
            .iter()
            .find(|(id, _, _)| *id == buffer_id)
            .map(|(_, content, modified)| (content.clone(), *modified))
    }
}

impl BufferContentProvider for MockContentProvider {
    fn content_bytes(&self, buffer_id: BufferId) -> Option<Vec<u8>> {
        self.find(buffer_id).map(|(content, _)| content)
    }

    fn content_size(&self, buffer_id: BufferId) -> Option<u64> {
        self.find(buffer_id)
            .map(|(content, _)| content.len() as u64)
    }

    #[allow(clippy::naive_bytecount)]
    fn content_unit_count(&self, buffer_id: BufferId) -> Option<usize> {
        self.find(buffer_id)
            .map(|(content, _)| content.iter().filter(|&&b| b == b'\n').count() + 1)
    }

    fn display_lines(
        &self,
        buffer_id: BufferId,
        offset: usize,
        count: usize,
    ) -> Option<Vec<DisplayLine>> {
        let (content, _) = self.find(buffer_id)?;
        let text = String::from_utf8_lossy(&content);
        let lines: Vec<&str> = text.lines().collect();
        let end = (offset + count).min(lines.len());
        Some(
            lines[offset..end]
                .iter()
                .enumerate()
                .map(|(i, line)| DisplayLine {
                    content: (*line).to_string(),
                    unit_index: offset + i,
                })
                .collect(),
        )
    }

    fn is_modified(&self, buffer_id: BufferId) -> bool {
        self.find(buffer_id).is_some_and(|(_, modified)| modified)
    }

    fn write_to(
        &self,
        buffer_id: BufferId,
        writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        if let Some((content, _)) = self.find(buffer_id) {
            writer.write_all(&content)
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "buffer not found"))
        }
    }
}

// --- Tests ---

#[test]
fn object_safety() {
    let _: Arc<dyn BufferContentProvider> = Arc::new(MockContentProvider::new());
}

#[test]
fn content_bytes_existing_buffer() {
    let provider = MockContentProvider::new();
    let id = BufferId::from_raw(1);
    provider.add_buffer(id, b"hello world", false);
    assert_eq!(provider.content_bytes(id).unwrap(), b"hello world");
}

#[test]
fn content_bytes_missing_buffer() {
    let provider = MockContentProvider::new();
    assert!(provider.content_bytes(BufferId::from_raw(99)).is_none());
}

#[test]
fn content_size() {
    let provider = MockContentProvider::new();
    let id = BufferId::from_raw(1);
    provider.add_buffer(id, b"hello", false);
    assert_eq!(provider.content_size(id), Some(5));
}

#[test]
fn content_unit_count() {
    let provider = MockContentProvider::new();
    let id = BufferId::from_raw(1);
    provider.add_buffer(id, b"line1\nline2\nline3", false);
    assert_eq!(provider.content_unit_count(id), Some(3));
}

#[test]
fn display_lines_within_viewport() {
    let provider = MockContentProvider::new();
    let id = BufferId::from_raw(1);
    provider.add_buffer(id, b"line0\nline1\nline2\nline3", false);
    let lines = provider.display_lines(id, 1, 2).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].content, "line1");
    assert_eq!(lines[0].unit_index, 1);
    assert_eq!(lines[1].content, "line2");
    assert_eq!(lines[1].unit_index, 2);
}

#[test]
fn display_lines_missing_buffer() {
    let provider = MockContentProvider::new();
    assert!(
        provider
            .display_lines(BufferId::from_raw(99), 0, 10)
            .is_none()
    );
}

#[test]
fn is_modified_true() {
    let provider = MockContentProvider::new();
    let id = BufferId::from_raw(1);
    provider.add_buffer(id, b"data", true);
    assert!(provider.is_modified(id));
}

#[test]
fn is_modified_false() {
    let provider = MockContentProvider::new();
    let id = BufferId::from_raw(1);
    provider.add_buffer(id, b"data", false);
    assert!(!provider.is_modified(id));
}

#[test]
fn is_modified_missing_buffer() {
    let provider = MockContentProvider::new();
    assert!(!provider.is_modified(BufferId::from_raw(99)));
}

#[test]
fn write_to_existing() {
    let provider = MockContentProvider::new();
    let id = BufferId::from_raw(1);
    provider.add_buffer(id, b"saved content", false);
    let mut buf = Vec::new();
    provider.write_to(id, &mut buf).unwrap();
    assert_eq!(buf, b"saved content");
}

#[test]
fn write_to_missing() {
    let provider = MockContentProvider::new();
    let result = provider.write_to(BufferId::from_raw(99), &mut Vec::new());
    assert!(result.is_err());
}

#[test]
fn display_line_debug() {
    let line = DisplayLine {
        content: "hello".to_string(),
        unit_index: 0,
    };
    let debug = format!("{line:?}");
    assert!(debug.contains("hello"));
    assert!(debug.contains("unit_index"));
}

#[test]
fn display_line_clone() {
    let line = DisplayLine {
        content: "test".to_string(),
        unit_index: 5,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = line.clone();
    assert_eq!(cloned.content, "test");
    assert_eq!(cloned.unit_index, 5);
}

#[test]
fn concurrent_arc_access() {
    let id = BufferId::from_raw(1);

    // Add buffer through the concrete type first
    let concrete = MockContentProvider::new();
    concrete.add_buffer(id, b"concurrent", false);
    let provider: Arc<dyn BufferContentProvider> = Arc::new(concrete);

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let p = Arc::clone(&provider);
            std::thread::spawn(move || {
                let bytes = p.content_bytes(id);
                assert!(bytes.is_some());
                assert_eq!(bytes.unwrap(), b"concurrent");
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
