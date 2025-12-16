//! Keymaps picker implementation

use std::future::Future;
use std::pin::Pin;

use crate::telescope::item::{TelescopeData, TelescopeItem};
use crate::telescope::state::PreviewContent;

use super::{Picker, PickerContext, TelescopeAction};

/// Keymap entry for display
#[derive(Debug, Clone)]
pub struct KeymapEntry {
    /// Mode name
    pub mode: String,
    /// Key sequence
    pub key: String,
    /// Command name
    pub command: String,
    /// Description
    pub description: Option<String>,
}

/// Picker for viewing keymaps
pub struct KeymapsPicker {
    /// Available keymaps
    keymaps: Vec<KeymapEntry>,
}

impl KeymapsPicker {
    /// Create a new keymaps picker
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keymaps: Vec::new(),
        }
    }

    /// Set keymaps from bind module
    pub fn set_keymaps(&mut self, keymaps: Vec<KeymapEntry>) {
        self.keymaps = keymaps;
    }
}

impl Default for KeymapsPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for KeymapsPicker {
    fn name(&self) -> &'static str {
        "keymaps"
    }

    fn title(&self) -> &'static str {
        "Keymaps"
    }

    fn prompt(&self) -> &'static str {
        "Keymaps> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<TelescopeItem>> + Send + '_>> {
        Box::pin(async move {
            self.keymaps
                .iter()
                .map(|km| {
                    let display = format!("[{}] {} -> {}", km.mode, km.key, km.command);
                    TelescopeItem::new(
                        &display,
                        &display,
                        TelescopeData::Keymap {
                            mode: km.mode.clone(),
                            key: km.key.clone(),
                            command: km.command.clone(),
                        },
                        "keymaps",
                    )
                    .with_detail(km.description.as_deref().unwrap_or(""))
                })
                .collect()
        })
    }

    fn on_select(&self, _item: &TelescopeItem) -> TelescopeAction {
        // Keymaps are informational, just close on select
        TelescopeAction::Close
    }

    fn preview(
        &self,
        item: &TelescopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let data = item.data.clone();
        let description = item.detail.clone();

        Box::pin(async move {
            if let TelescopeData::Keymap { mode, key, command } = data {
                let mut lines = vec![
                    "Keymap Details".to_string(),
                    "==============".to_string(),
                    String::new(),
                    format!("Mode:    {mode}"),
                    format!("Key:     {key}"),
                    format!("Command: {command}"),
                ];

                if let Some(desc) = description {
                    lines.push(String::new());
                    lines.push(format!("Description: {desc}"));
                }

                Some(PreviewContent::new(lines))
            } else {
                None
            }
        })
    }
}
