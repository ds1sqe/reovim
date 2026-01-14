//! Keymaps picker implementation

use std::{future::Future, pin::Pin};

use {
    reovim_core::bind::{CommandRef, KeyMap, KeymapScope},
    reovim_plugin_microscope::{
        MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
    },
};

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

        for (scope, map) in keymap.iter_scopes() {
            let mode_name = Self::scope_to_mode_name(scope);
            for (key, inner) in map {
                if let Some(cmd_ref) = &inner.command {
                    let command = match cmd_ref {
                        CommandRef::Registered(id) => id.as_str().to_string(),
                        CommandRef::Inline(cmd) => cmd.name().to_string(),
                    };
                    entries.push(KeymapEntry {
                        mode: mode_name.to_string(),
                        key: key.to_string(),
                        command,
                        description: inner.description.clone(),
                    });
                }
            }
        }

        // Sort by mode, then by key
        entries.sort_by(|a, b| a.mode.cmp(&b.mode).then_with(|| a.key.cmp(&b.key)));

        entries
    }

    /// Convert a `KeymapScope` to a display mode name
    ///
    /// Note: This uses static mode names. Plugin-provided display names
    /// should be queried from `DisplayRegistry` when available.
    const fn scope_to_mode_name(scope: &KeymapScope) -> &'static str {
        use reovim_core::bind::{EditModeKind, SubModeKind};

        match scope {
            KeymapScope::Component { id: _, mode } => {
                // Generic mode names - plugins register their own display names
                // via DisplayRegistry. Core editor and plugin components use same names.
                match mode {
                    EditModeKind::Normal => "Normal",
                    EditModeKind::Insert => "Insert",
                    EditModeKind::Visual => "Visual",
                }
            }
            KeymapScope::SubMode(submode) => match submode {
                SubModeKind::Command => "Command",
                SubModeKind::OperatorPending => "Operator",
                // Generic interactor handling - plugins register their own display names
                SubModeKind::Interactor(_) => "Interactor",
            },
            KeymapScope::DefaultNormal => "Default (Normal)",
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
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        Box::pin(async move {
            self.keymaps
                .iter()
                .map(|km| {
                    let display = format!("[{}] {} -> {}", km.mode, km.key, km.command);
                    MicroscopeItem::new(
                        &display,
                        &display,
                        MicroscopeData::Keymap {
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

    fn on_select(&self, _item: &MicroscopeItem) -> MicroscopeAction {
        // Keymaps are informational, just close on select
        MicroscopeAction::Close
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let data = item.data.clone();
        let description = item.detail.clone();

        Box::pin(async move {
            if let MicroscopeData::Keymap { mode, key, command } = data {
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
