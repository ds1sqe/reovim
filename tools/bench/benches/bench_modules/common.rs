//! Common utilities for render benchmarks

use {
    reovim_core::{
        buffer::{Buffer, TextOps},
        content::WindowContentSource,
        screen::{
            Position,
            window::{Anchor, LineNumber, Window},
        },
    },
    std::{
        collections::BTreeMap,
        io::{self, Write},
    },
};

/// Create a buffer with the specified number of lines
pub fn create_buffer(lines: usize) -> Buffer {
    let content = (0..lines)
        .map(|i| format!("Line {i} with some typical text content for benchmarking"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut buf = Buffer::empty(0);
    buf.set_content(&content);
    buf
}

/// Create a large realistic buffer (source code-like content)
#[allow(dead_code)]
pub fn create_realistic_buffer(lines: usize) -> Buffer {
    let content = (0..lines)
        .map(|i| match i % 10 {
            0 => format!("fn function_{i}(arg1: i32, arg2: String) -> Result<(), Error> {{"),
            1 => "    let mut result = Vec::new();".to_string(),
            2 => format!("    for item in collection_{i}.iter() {{"),
            3 => "        if item.is_valid() && item.check_condition() {".to_string(),
            4 => format!("            result.push(process_item_{i:04}(item));"),
            5 => "        } else {".to_string(),
            6 => "            log::warn!(\"Invalid item encountered\");".to_string(),
            7 => "        }".to_string(),
            8 => "    }".to_string(),
            _ => "}\n".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut buf = Buffer::empty(0);
    buf.set_content(&content);
    buf
}

/// Create a window with typical editor dimensions
pub fn create_window(height: u16) -> Window {
    Window {
        id: 0,
        source: WindowContentSource::FileBuffer {
            buffer_id: 0,
            buffer_anchor: Anchor { x: 0, y: 0 },
        },
        anchor: Anchor { x: 0, y: 0 },
        width: 120,
        height,
        z_order: 0,
        is_active: true,
        is_floating: false,
        line_number: Some(LineNumber::default()),
        scrollbar_enabled: false,
        cursor: Position { x: 0, y: 0 },
        desired_col: None,
        border_config: None,
    }
}

/// A mock writer that counts bytes written
#[allow(dead_code)]
pub struct MockWriter {
    pub bytes_written: usize,
}

impl MockWriter {
    #[must_use]
    #[allow(dead_code)]
    pub const fn new() -> Self {
        Self { bytes_written: 0 }
    }
}

impl Default for MockWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes_written += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Create a buffer map from an existing buffer
#[allow(dead_code)]
pub fn buffer_to_map(buffer: Buffer) -> BTreeMap<usize, Buffer> {
    let mut map = BTreeMap::new();
    map.insert(0, buffer);
    map
}
