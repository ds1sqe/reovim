//! Keymaps picker implementation

use std::{future::Future, pin::Pin};

use crate::{
    bind::{CommandRef, KeyMap},
    telescope::{
        item::{TelescopeData, TelescopeItem},
        state::PreviewContent,
    },
};

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
    /// Create a new keymaps picker populated with default keymaps
    #[must_use]
    pub fn new() -> Self {
        let keymap = KeyMap::with_defaults();
        let keymaps = Self::extract_keymaps(&keymap);
        Self { keymaps }
    }

    /// Extract all keymaps from a `KeyMap` into `KeymapEntry` items
    fn extract_keymaps(keymap: &KeyMap) -> Vec<KeymapEntry> {
        let mut entries = Vec::new();

        // Helper to extract entries from a mode's keymap
        let mut extract_mode =
            |mode_name: &str, map: &std::collections::HashMap<String, crate::bind::KeyMapInner>| {
                for (key, inner) in map {
                    if let Some(cmd_ref) = &inner.command {
                        let command = match cmd_ref {
                            CommandRef::Registered(id) => id.as_str().to_string(),
                            CommandRef::Inline(cmd) => cmd.name().to_string(),
                        };
                        entries.push(KeymapEntry {
                            mode: mode_name.to_string(),
                            key: key.clone(),
                            command,
                            description: inner.hint.clone(),
                        });
                    }
                }
            };

        extract_mode("Normal", &keymap.normal);
        extract_mode("Insert", &keymap.insert);
        extract_mode("Visual", &keymap.visual);
        extract_mode("Command", &keymap.command);
        extract_mode("Explorer", &keymap.explorer);
        extract_mode("Explorer Input", &keymap.explorer_input);
        extract_mode("Operator", &keymap.operator_pending);
        extract_mode("Telescope Normal", &keymap.telescope_normal);
        extract_mode("Telescope Insert", &keymap.telescope_insert);
        extract_mode("Leap", &keymap.leap);

        // Sort by mode, then by key
        entries.sort_by(|a, b| a.mode.cmp(&b.mode).then_with(|| a.key.cmp(&b.key)));

        entries
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
