//! Help tags picker implementation

use std::{future::Future, pin::Pin};

use reovim_plugin_microscope::{
    MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
};

/// Help entry
#[derive(Debug, Clone)]
pub struct HelpEntry {
    /// Tag name
    pub tag: String,
    /// Brief description
    pub description: String,
    /// Full help content
    pub content: Vec<String>,
}

/// Picker for help tags
pub struct HelpPicker {
    /// Available help entries
    entries: Vec<HelpEntry>,
}

impl HelpPicker {
    /// Create a new help picker
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: default_help_entries(),
        }
    }

    /// Set help entries
    pub fn set_entries(&mut self, entries: Vec<HelpEntry>) {
        self.entries = entries;
    }
}

impl Default for HelpPicker {
    fn default() -> Self {
        Self::new()
    }
}

fn default_help_entries() -> Vec<HelpEntry> {
    vec![
        HelpEntry {
            tag: "modes".to_string(),
            description: "Editor modes (Normal, Insert, Visual, Command)".to_string(),
            content: vec![
                "MODES".to_string(),
                "=====".to_string(),
                String::new(),
                "Normal Mode: Navigate and edit text".to_string(),
                "  - Press 'i' to enter Insert mode".to_string(),
                "  - Press 'v' to enter Visual mode".to_string(),
                "  - Press ':' to enter Command mode".to_string(),
                String::new(),
                "Insert Mode: Insert text".to_string(),
                "  - Press Escape to return to Normal mode".to_string(),
                String::new(),
                "Visual Mode: Select text".to_string(),
                "  - Press 'y' to yank (copy)".to_string(),
                "  - Press 'd' to delete".to_string(),
                "  - Press Escape to return to Normal mode".to_string(),
                String::new(),
                "Command Mode: Execute commands".to_string(),
                "  - :w - Write file".to_string(),
                "  - :q - Quit".to_string(),
                "  - :wq - Write and quit".to_string(),
            ],
        },
        HelpEntry {
            tag: "motions".to_string(),
            description: "Cursor movement commands".to_string(),
            content: vec![
                "MOTIONS".to_string(),
                "=======".to_string(),
                String::new(),
                "h, j, k, l     - Move left, down, up, right".to_string(),
                "w              - Move to next word".to_string(),
                "b              - Move to previous word".to_string(),
                "e              - Move to end of word".to_string(),
                "0              - Move to start of line".to_string(),
                "$              - Move to end of line".to_string(),
                "gg             - Move to first line".to_string(),
                "G              - Move to last line".to_string(),
            ],
        },
        HelpEntry {
            tag: "microscope".to_string(),
            description: "Microscope fuzzy finder".to_string(),
            content: vec![
                "MICROSCOPE".to_string(),
                "=========".to_string(),
                String::new(),
                "Open Microscope:".to_string(),
                "  <Space>ff    - Find files".to_string(),
                "  <Space>fb    - Find buffers".to_string(),
                "  <Space>fg    - Live grep".to_string(),
                "  <Space>fr    - Recent files".to_string(),
                "  <Space>fc    - Command palette".to_string(),
                "  <Space>fh    - Help tags".to_string(),
                "  <Space>fk    - Keymaps".to_string(),
                String::new(),
                "Navigation:".to_string(),
                "  C-n, Down    - Next item".to_string(),
                "  C-p, Up      - Previous item".to_string(),
                "  Enter        - Confirm selection".to_string(),
                "  Escape       - Close".to_string(),
            ],
        },
        HelpEntry {
            tag: "editing".to_string(),
            description: "Text editing commands".to_string(),
            content: vec![
                "EDITING".to_string(),
                "=======".to_string(),
                String::new(),
                "Insert mode:".to_string(),
                "  i            - Insert before cursor".to_string(),
                "  a            - Insert after cursor".to_string(),
                "  A            - Insert at end of line".to_string(),
                "  o            - Open line below".to_string(),
                "  O            - Open line above".to_string(),
                String::new(),
                "Delete:".to_string(),
                "  x            - Delete character".to_string(),
                "  dd           - Delete line".to_string(),
                "  d{motion}    - Delete with motion".to_string(),
                String::new(),
                "Clipboard:".to_string(),
                "  yy           - Yank (copy) line".to_string(),
                "  p            - Paste after".to_string(),
                "  P            - Paste before".to_string(),
            ],
        },
    ]
}

impl Picker for HelpPicker {
    fn name(&self) -> &'static str {
        "help"
    }

    fn title(&self) -> &'static str {
        "Help Tags"
    }

    fn prompt(&self) -> &'static str {
        "Help> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        Box::pin(async move {
            self.entries
                .iter()
                .map(|entry| {
                    MicroscopeItem::new(
                        &entry.tag,
                        &entry.tag,
                        MicroscopeData::HelpTag(entry.tag.clone()),
                        "help",
                    )
                    .with_detail(&entry.description)
                    .with_icon('?')
                })
                .collect()
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::HelpTag(tag) => MicroscopeAction::ShowHelp(tag.clone()),
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let tag = match &item.data {
            MicroscopeData::HelpTag(t) => t.clone(),
            _ => return Box::pin(async { None }),
        };

        let content = self
            .entries
            .iter()
            .find(|e| e.tag == tag)
            .map(|e| e.content.clone());

        Box::pin(async move { content.map(PreviewContent::new) })
    }
}
