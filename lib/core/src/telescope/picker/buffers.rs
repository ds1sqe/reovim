//! Buffer picker implementation

use std::future::Future;
use std::pin::Pin;

use crate::telescope::item::{TelescopeData, TelescopeItem};
use crate::telescope::state::PreviewContent;

use super::{Picker, PickerContext, TelescopeAction};

/// Buffer info passed from runtime
#[derive(Debug, Clone)]
pub struct BufferInfo {
    /// Buffer ID
    pub id: usize,
    /// Buffer name/path
    pub name: String,
    /// Whether the buffer is modified
    pub modified: bool,
    /// Preview lines
    pub preview_lines: Vec<String>,
}

/// Picker for switching between open buffers
pub struct BuffersPicker {
    /// Available buffers (set by runtime before opening)
    buffers: Vec<BufferInfo>,
}

impl BuffersPicker {
    /// Create a new buffers picker
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffers: Vec::new(),
        }
    }

    /// Set available buffers
    pub fn set_buffers(&mut self, buffers: Vec<BufferInfo>) {
        self.buffers = buffers;
    }
}

impl Default for BuffersPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for BuffersPicker {
    fn name(&self) -> &'static str {
        "buffers"
    }

    fn title(&self) -> &'static str {
        "Find Buffers"
    }

    fn prompt(&self) -> &'static str {
        "Buffers> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<TelescopeItem>> + Send + '_>> {
        Box::pin(async move {
            self.buffers
                .iter()
                .map(|buf| {
                    let icon = if buf.modified { '*' } else { ' ' };
                    TelescopeItem::new(
                        buf.id.to_string(),
                        &buf.name,
                        TelescopeData::BufferId(buf.id),
                        "buffers",
                    )
                    .with_icon(icon)
                    .with_detail(format!("#{}", buf.id))
                })
                .collect()
        })
    }

    fn on_select(&self, item: &TelescopeItem) -> TelescopeAction {
        match &item.data {
            TelescopeData::BufferId(id) => TelescopeAction::SwitchBuffer(*id),
            _ => TelescopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &TelescopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let buffer_id = match &item.data {
            TelescopeData::BufferId(id) => *id,
            _ => return Box::pin(async { None }),
        };

        let preview_lines = self
            .buffers
            .iter()
            .find(|b| b.id == buffer_id)
            .map(|b| b.preview_lines.clone());

        Box::pin(async move {
            preview_lines.map(PreviewContent::new)
        })
    }
}
