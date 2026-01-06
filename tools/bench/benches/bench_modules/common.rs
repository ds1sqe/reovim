//! Common utilities for render benchmarks

use reovim_core::{
    buffer::{Buffer, TextOps},
    content::WindowContentSource,
    screen::{
        Position,
        window::{Anchor, LineNumber, Window},
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
        sign_column_width: None,
        scrollbar_enabled: false,
        cursor: Position { x: 0, y: 0 },
        desired_col: None,
        border_config: None,
    }
}
