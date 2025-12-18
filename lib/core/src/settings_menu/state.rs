//! Settings menu state management

use {
    super::item::{ActionType, FlatItem, SettingItem, SettingSection, SettingValue},
    crate::config::ProfileConfig,
};

/// Input mode for the settings menu
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsInputMode {
    /// Normal navigation mode
    #[default]
    Normal,
    /// Typing a number value
    NumberInput,
    /// Typing a text value (e.g., profile name)
    TextInput,
}

/// Message severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Success,
    Error,
}

/// Layout information for the settings menu
#[derive(Debug, Clone, Default)]
pub struct MenuLayout {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub visible_items: usize,
}

/// Settings menu state
#[derive(Debug, Clone, Default)]
pub struct SettingsMenuState {
    /// Whether the menu is visible
    pub visible: bool,
    /// Setting sections
    pub sections: Vec<SettingSection>,
    /// Flattened items for navigation
    pub flat_items: Vec<FlatItem>,
    /// Currently selected flat item index
    pub selected_index: usize,
    /// Scroll offset for long lists
    pub scroll_offset: usize,
    /// Layout dimensions
    pub layout: MenuLayout,
    /// Current input mode
    pub input_mode: SettingsInputMode,
    /// Input buffer for text/number entry
    pub input_buffer: String,
    /// Input prompt label
    pub input_prompt: String,
    /// Pending action that triggered input mode
    pub pending_action: Option<ActionType>,
    /// Status message to display
    pub message: Option<(String, MessageKind)>,
}

impl SettingsMenuState {
    /// Create a new settings menu state
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the settings menu with current profile settings
    pub fn open(&mut self, profile: &ProfileConfig, profile_name: &str) {
        self.visible = true;
        self.scroll_offset = 0;
        self.input_mode = SettingsInputMode::Normal;
        self.input_buffer.clear();
        self.input_prompt.clear();
        self.pending_action = None;
        self.message = None;
        self.populate_from_profile(profile, profile_name);

        // Select first actual setting (skip section headers)
        self.selected_index = 0;
        for (i, item) in self.flat_items.iter().enumerate() {
            if item.is_setting() {
                self.selected_index = i;
                break;
            }
        }
    }

    /// Close the settings menu
    pub fn close(&mut self) {
        self.visible = false;
        self.input_mode = SettingsInputMode::Normal;
        self.input_buffer.clear();
        self.input_prompt.clear();
        self.pending_action = None;
        self.message = None;
    }

    /// Populate settings from a profile config
    fn populate_from_profile(&mut self, profile: &ProfileConfig, profile_name: &str) {
        self.sections.clear();
        self.sections.push(Self::build_editor_section(profile));
        self.sections.push(Self::build_cursor_section());
        self.sections.push(Self::build_window_section(profile));
        self.sections
            .push(Self::build_profile_section(profile_name));
        self.rebuild_flat_items();
    }

    /// Build the editor settings section
    fn build_editor_section(profile: &ProfileConfig) -> SettingSection {
        let editor = &profile.editor;
        SettingSection {
            name: "Editor".to_string(),
            items: vec![
                SettingItem {
                    key: "editor.theme",
                    label: "Theme".to_string(),
                    description: Some("Color theme".to_string()),
                    value: SettingValue::Choice {
                        options: vec![
                            "dark".to_string(),
                            "light".to_string(),
                            "tokyonight".to_string(),
                        ],
                        selected: match editor.theme.as_str() {
                            "light" => 1,
                            "tokyonight" => 2,
                            _ => 0,
                        },
                    },
                },
                SettingItem {
                    key: "editor.colormode",
                    label: "Color Mode".to_string(),
                    description: Some("Terminal color support".to_string()),
                    value: SettingValue::Choice {
                        options: vec![
                            "ansi".to_string(),
                            "256".to_string(),
                            "truecolor".to_string(),
                        ],
                        selected: match editor.colormode.as_str() {
                            "ansi" => 0,
                            "256" => 1,
                            _ => 2,
                        },
                    },
                },
                SettingItem {
                    key: "editor.number",
                    label: "Line Numbers".to_string(),
                    description: Some("Show line numbers".to_string()),
                    value: SettingValue::Bool(editor.number),
                },
                SettingItem {
                    key: "editor.relativenumber",
                    label: "Relative Numbers".to_string(),
                    description: Some("Show relative line numbers".to_string()),
                    value: SettingValue::Bool(editor.relativenumber),
                },
                SettingItem {
                    key: "editor.indentguide",
                    label: "Indent Guides".to_string(),
                    description: Some("Show indentation guides".to_string()),
                    value: SettingValue::Bool(editor.indentguide),
                },
                SettingItem {
                    key: "editor.scrollbar",
                    label: "Scrollbar".to_string(),
                    description: Some("Show scrollbar".to_string()),
                    value: SettingValue::Bool(editor.scrollbar),
                },
                SettingItem {
                    key: "editor.tabwidth",
                    label: "Tab Width".to_string(),
                    description: Some("Spaces per tab".to_string()),
                    value: SettingValue::Number {
                        value: i32::from(editor.tabwidth),
                        min: 1,
                        max: 8,
                        step: 1,
                    },
                },
            ],
        }
    }

    /// Build the cursor settings section (placeholder for future schema additions)
    fn build_cursor_section() -> SettingSection {
        SettingSection {
            name: "Cursor".to_string(),
            items: vec![
                SettingItem {
                    key: "cursor.style",
                    label: "Style".to_string(),
                    description: Some("Cursor shape".to_string()),
                    value: SettingValue::Choice {
                        options: vec![
                            "block".to_string(),
                            "line".to_string(),
                            "underline".to_string(),
                        ],
                        selected: 0,
                    },
                },
                SettingItem {
                    key: "cursor.blink",
                    label: "Blink".to_string(),
                    description: Some("Cursor blinking".to_string()),
                    value: SettingValue::Bool(false),
                },
            ],
        }
    }

    /// Build the window settings section
    fn build_window_section(profile: &ProfileConfig) -> SettingSection {
        let window = &profile.window;
        SettingSection {
            name: "Window".to_string(),
            items: vec![
                SettingItem {
                    key: "window.default_split",
                    label: "Default Split".to_string(),
                    description: Some("Direction for new splits".to_string()),
                    value: SettingValue::Choice {
                        options: vec!["vertical".to_string(), "horizontal".to_string()],
                        selected: usize::from(window.default_split == "horizontal"),
                    },
                },
                SettingItem {
                    key: "window.explorer_width",
                    label: "Explorer Width".to_string(),
                    description: Some("File explorer width".to_string()),
                    value: SettingValue::Number {
                        value: 30,
                        min: 20,
                        max: 60,
                        step: 5,
                    },
                },
            ],
        }
    }

    /// Build the profile management section
    fn build_profile_section(profile_name: &str) -> SettingSection {
        SettingSection {
            name: "Profile".to_string(),
            items: vec![
                SettingItem {
                    key: "profile.current",
                    label: "Current".to_string(),
                    description: Some("Active profile".to_string()),
                    value: SettingValue::Display(profile_name.to_string()),
                },
                SettingItem {
                    key: "profile.save",
                    label: "Save as...".to_string(),
                    description: Some("Save current settings to profile".to_string()),
                    value: SettingValue::Action(ActionType::SaveProfile),
                },
                SettingItem {
                    key: "profile.load",
                    label: "Load...".to_string(),
                    description: Some("Load a different profile".to_string()),
                    value: SettingValue::Action(ActionType::LoadProfile),
                },
            ],
        }
    }

    /// Rebuild the flat items list from sections
    fn rebuild_flat_items(&mut self) {
        self.flat_items.clear();
        for (section_idx, section) in self.sections.iter().enumerate() {
            self.flat_items
                .push(FlatItem::SectionHeader(section.name.clone()));
            for item_idx in 0..section.items.len() {
                self.flat_items.push(FlatItem::Setting {
                    section_idx,
                    item_idx,
                });
            }
        }
    }

    /// Calculate layout based on screen dimensions
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn calculate_layout(&mut self, screen_width: u16, screen_height: u16) {
        // 60% width, 80% height - centered
        let width = ((f32::from(screen_width) * 0.6) as u16).clamp(50, 80);
        let height = ((f32::from(screen_height) * 0.8) as u16).max(15);

        let x = (screen_width.saturating_sub(width)) / 2;
        let y = (screen_height.saturating_sub(height)) / 2;

        // Reserve 4 lines for border (2) + header (1) + footer (1)
        let visible_items = (height.saturating_sub(4)) as usize;

        self.layout = MenuLayout {
            x,
            y,
            width,
            height,
            visible_items,
        };
    }

    /// Move selection to next item
    pub fn select_next(&mut self) {
        if self.flat_items.is_empty() {
            return;
        }

        let mut next = self.selected_index + 1;
        while next < self.flat_items.len() {
            if self.flat_items[next].is_setting() {
                self.selected_index = next;
                self.ensure_visible();
                return;
            }
            next += 1;
        }
        // Wrap to first setting
        for (i, item) in self.flat_items.iter().enumerate() {
            if item.is_setting() {
                self.selected_index = i;
                self.ensure_visible();
                return;
            }
        }
    }

    /// Move selection to previous item
    pub fn select_prev(&mut self) {
        if self.flat_items.is_empty() {
            return;
        }

        let mut prev = self.selected_index.saturating_sub(1);
        loop {
            if self.flat_items[prev].is_setting() {
                self.selected_index = prev;
                self.ensure_visible();
                return;
            }
            if prev == 0 {
                break;
            }
            prev -= 1;
        }
        // Wrap to last setting
        for i in (0..self.flat_items.len()).rev() {
            if self.flat_items[i].is_setting() {
                self.selected_index = i;
                self.ensure_visible();
                return;
            }
        }
    }

    /// Ensure the selected item is visible
    const fn ensure_visible(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.layout.visible_items {
            self.scroll_offset = self
                .selected_index
                .saturating_sub(self.layout.visible_items)
                + 1;
        }
    }

    /// Get the currently selected setting item (mutable)
    pub fn selected_item_mut(&mut self) -> Option<&mut SettingItem> {
        let flat_item = self.flat_items.get(self.selected_index)?;
        if let FlatItem::Setting {
            section_idx,
            item_idx,
        } = flat_item
        {
            self.sections
                .get_mut(*section_idx)?
                .items
                .get_mut(*item_idx)
        } else {
            None
        }
    }

    /// Get the currently selected setting item (immutable)
    #[must_use]
    pub fn selected_item(&self) -> Option<&SettingItem> {
        let flat_item = self.flat_items.get(self.selected_index)?;
        if let FlatItem::Setting {
            section_idx,
            item_idx,
        } = flat_item
        {
            self.sections.get(*section_idx)?.items.get(*item_idx)
        } else {
            None
        }
    }

    /// Toggle the selected boolean setting. Returns true if toggled.
    pub fn toggle_selected(&mut self) -> bool {
        if let Some(item) = self.selected_item_mut()
            && item.value.is_bool()
        {
            item.value.toggle();
            true
        } else {
            false
        }
    }

    /// Cycle to next value for selected setting. Returns true if changed.
    pub fn cycle_next_selected(&mut self) -> bool {
        if let Some(item) = self.selected_item_mut() {
            match &item.value {
                SettingValue::Choice { .. } => {
                    item.value.cycle_next();
                    true
                }
                SettingValue::Number { .. } => {
                    item.value.increment();
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Cycle to previous value for selected setting. Returns true if changed.
    pub fn cycle_prev_selected(&mut self) -> bool {
        if let Some(item) = self.selected_item_mut() {
            match &item.value {
                SettingValue::Choice { .. } => {
                    item.value.cycle_prev();
                    true
                }
                SettingValue::Number { .. } => {
                    item.value.decrement();
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Quick select for the selected setting. Returns true if changed.
    pub fn quick_select(&mut self, index: u8) -> bool {
        if let Some(item) = self.selected_item_mut()
            && item.value.is_choice()
        {
            item.value.quick_select(index);
            true
        } else {
            false
        }
    }

    /// Increment the selected number setting. Returns true if changed.
    pub fn increment_selected(&mut self) -> bool {
        if let Some(item) = self.selected_item_mut()
            && item.value.is_number()
        {
            item.value.increment();
            true
        } else {
            false
        }
    }

    /// Decrement the selected number setting. Returns true if changed.
    pub fn decrement_selected(&mut self) -> bool {
        if let Some(item) = self.selected_item_mut()
            && item.value.is_number()
        {
            item.value.decrement();
            true
        } else {
            false
        }
    }

    /// Get the action type if the selected item is an action
    #[must_use]
    pub fn get_selected_action(&self) -> Option<ActionType> {
        if let Some(item) = self.selected_item()
            && let SettingValue::Action(action_type) = &item.value
        {
            return Some(*action_type);
        }
        None
    }

    /// Set a message to display
    pub fn set_message(&mut self, message: String, kind: MessageKind) {
        self.message = Some((message, kind));
    }

    /// Clear any message
    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Get the current profile config from settings state
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn to_profile_config(&self) -> ProfileConfig {
        let mut config = ProfileConfig::default();

        for section in &self.sections {
            for item in &section.items {
                match item.key {
                    "editor.theme" => {
                        config.editor.theme = item.value.display_value();
                    }
                    "editor.colormode" => {
                        config.editor.colormode = item.value.display_value();
                    }
                    "editor.number" => {
                        if let SettingValue::Bool(b) = &item.value {
                            config.editor.number = *b;
                        }
                    }
                    "editor.relativenumber" => {
                        if let SettingValue::Bool(b) = &item.value {
                            config.editor.relativenumber = *b;
                        }
                    }
                    "editor.indentguide" => {
                        if let SettingValue::Bool(b) = &item.value {
                            config.editor.indentguide = *b;
                        }
                    }
                    "editor.scrollbar" => {
                        if let SettingValue::Bool(b) = &item.value {
                            config.editor.scrollbar = *b;
                        }
                    }
                    "editor.tabwidth" => {
                        if let SettingValue::Number { value, .. } = &item.value {
                            config.editor.tabwidth = value.clamp(&1, &8).unsigned_abs() as u8;
                        }
                    }
                    "window.default_split" => {
                        config.window.default_split = item.value.display_value();
                    }
                    _ => {}
                }
            }
        }

        config
    }

    // --- Text Input Mode Methods ---

    /// Enter text input mode for a specific action
    pub fn enter_text_input(&mut self, action: ActionType, prompt: &str, default_value: &str) {
        self.input_mode = SettingsInputMode::TextInput;
        self.pending_action = Some(action);
        self.input_prompt = prompt.to_string();
        self.input_buffer = default_value.to_string();
    }

    /// Exit text input mode without confirming
    pub fn cancel_text_input(&mut self) {
        self.input_mode = SettingsInputMode::Normal;
        self.pending_action = None;
        self.input_prompt.clear();
        self.input_buffer.clear();
    }

    /// Add a character to the input buffer
    pub fn input_char(&mut self, c: char) {
        if self.input_mode == SettingsInputMode::TextInput {
            // Only allow valid filename characters
            if c.is_alphanumeric() || c == '_' || c == '-' {
                self.input_buffer.push(c);
            }
        }
    }

    /// Remove the last character from the input buffer
    pub fn input_backspace(&mut self) {
        if self.input_mode == SettingsInputMode::TextInput {
            self.input_buffer.pop();
        }
    }

    /// Check if we're in text input mode
    #[must_use]
    pub const fn is_text_input_mode(&self) -> bool {
        matches!(self.input_mode, SettingsInputMode::TextInput)
    }

    /// Get the current input value (for confirming)
    #[must_use]
    pub fn get_input_value(&self) -> &str {
        &self.input_buffer
    }

    /// Take the pending action (consumes it)
    pub const fn take_pending_action(&mut self) -> Option<ActionType> {
        self.pending_action.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_menu_open_close() {
        let mut state = SettingsMenuState::new();
        assert!(!state.visible);

        let profile = ProfileConfig::default();
        state.open(&profile, "default");
        assert!(state.visible);
        assert!(!state.sections.is_empty());
        assert!(!state.flat_items.is_empty());

        state.close();
        assert!(!state.visible);
    }

    #[test]
    fn test_navigation() {
        let mut state = SettingsMenuState::new();
        let profile = ProfileConfig::default();
        state.open(&profile, "default");
        state.calculate_layout(120, 40);

        // First selectable item should be a setting (skip header)
        let initial = state.selected_index;
        assert!(state.flat_items[initial].is_setting());

        // Move next
        state.select_next();
        assert!(state.selected_index > initial);

        // Move prev
        state.select_prev();
        assert_eq!(state.selected_index, initial);
    }

    #[test]
    fn test_toggle() {
        let mut state = SettingsMenuState::new();
        let profile = ProfileConfig::default();
        state.open(&profile, "default");

        // Find a boolean setting
        while state.selected_item().is_some() {
            if let Some(item) = state.selected_item()
                && item.value.is_bool()
            {
                break;
            }
            state.select_next();
        }

        if let Some(item) = state.selected_item()
            && let SettingValue::Bool(initial) = item.value
        {
            state.toggle_selected();
            if let Some(item) = state.selected_item()
                && let SettingValue::Bool(after) = item.value
            {
                assert_ne!(initial, after);
            }
        }
    }
}
