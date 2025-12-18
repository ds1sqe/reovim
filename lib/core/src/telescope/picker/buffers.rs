//! Buffer picker implementation

use std::{future::Future, pin::Pin};

use crate::telescope::{
    item::{TelescopeData, TelescopeItem},
    state::PreviewContent,
};

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
pub struct BuffersPicker;

impl BuffersPicker {
    /// Create a new buffers picker
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<TelescopeItem>> + Send + '_>> {
        let buffers = ctx.buffers.clone();
        Box::pin(async move {
            buffers
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
        _item: &TelescopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        // Preview would require buffer content which isn't available in this context
        // For now, just return None
        Box::pin(async { None })
    }
}
