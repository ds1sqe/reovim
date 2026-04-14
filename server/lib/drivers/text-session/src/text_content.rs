//! Text-domain content provider wrapping `TextBufferRegistry`.
//!
//! Implements [`BufferContentProvider`] for the text domain by delegating
//! to the existing `TextBufferRegistry` from `reovim-provider-text`.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::BufferId,
    reovim_provider_text::TextBufferRegistry,
    reovim_subsys_session::{BufferContentProvider, DisplayLine, Viewport},
};

/// Text-domain content provider wrapping `TextBufferRegistry`.
///
/// `Arc`-compatible for async-safe access across `.await` boundaries.
pub struct TextContentProvider {
    registry: Arc<TextBufferRegistry>,
}

impl TextContentProvider {
    /// Create a new text content provider wrapping the given registry.
    #[must_use]
    pub const fn new(registry: Arc<TextBufferRegistry>) -> Self {
        Self { registry }
    }
}

impl BufferContentProvider for TextContentProvider {
    fn content_bytes(&self, buffer_id: BufferId) -> Option<Vec<u8>> {
        let buffer = self.registry.get(buffer_id)?;
        let guard = buffer.read();
        Some(guard.content_bytes())
    }

    fn content_size(&self, buffer_id: BufferId) -> Option<u64> {
        let buffer = self.registry.get(buffer_id)?;
        let guard = buffer.read();
        Some(guard.byte_len() as u64)
    }

    fn content_unit_count(&self, buffer_id: BufferId) -> Option<usize> {
        let buffer = self.registry.get(buffer_id)?;
        let guard = buffer.read();
        Some(guard.line_count())
    }

    fn display_lines(&self, buffer_id: BufferId, viewport: &Viewport) -> Option<Vec<DisplayLine>> {
        let buffer = self.registry.get(buffer_id)?;
        let guard = buffer.read();
        let line_count = guard.line_count();

        let start = viewport.scroll_top;
        let end = (start + viewport.height as usize).min(line_count);

        let mut lines = Vec::with_capacity(end.saturating_sub(start));
        for idx in start..end {
            if let Some(line_text) = guard.line(idx) {
                lines.push(DisplayLine {
                    content: line_text.into_owned(),
                    unit_index: idx,
                });
            }
        }
        Some(lines)
    }

    fn is_modified(&self, buffer_id: BufferId) -> bool {
        self.registry
            .get(buffer_id)
            .is_some_and(|buffer| buffer.read().is_modified())
    }

    fn write_to(
        &self,
        buffer_id: BufferId,
        writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        let Some(buffer) = self.registry.get(buffer_id) else {
            return Ok(());
        };
        let guard = buffer.read();
        guard.write_to(writer)
    }
}

#[cfg(test)]
#[path = "text_content_tests.rs"]
mod tests;
