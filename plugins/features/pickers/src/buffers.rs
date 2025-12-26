//! Buffer picker implementation

use std::{future::Future, pin::Pin};

use reovim_plugin_microscope::{
    MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
};

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
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        let buffers = ctx.buffers.clone();
        Box::pin(async move {
            buffers
                .iter()
                .map(|buf| {
                    let icon = if buf.modified { '*' } else { ' ' };
                    MicroscopeItem::new(
                        buf.id.to_string(),
                        &buf.name,
                        MicroscopeData::BufferId(buf.id),
                        "buffers",
                    )
                    .with_icon(icon)
                    .with_detail(format!("#{}", buf.id))
                })
                .collect()
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::BufferId(id) => MicroscopeAction::SwitchBuffer(*id),
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        _item: &MicroscopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        // Preview would require buffer content which isn't available in this context
        // For now, just return None
        Box::pin(async { None })
    }
}
