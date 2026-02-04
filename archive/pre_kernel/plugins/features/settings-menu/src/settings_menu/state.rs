//! Settings menu state management

use std::collections::HashMap;

use {
    super::item::{ActionType, FlatItem, SettingItem, SettingSection, SettingValue},
    reovim_core::{
        config::ProfileConfig,
        option::{OptionSpec, OptionValue},
    },
    tracing::error,
};

/// Information about a setting change, used to emit `OptionChanged` events.
#[derive(Debug, Clone)]
pub struct SettingChange {
    /// The setting key (e.g., "editor.theme")
    pub key: String,
    /// The value before the change
    pub old_value: OptionValue,
    /// The value after the change
    pub new_value: OptionValue,
}

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

/// Metadata for a registered settings section
#[derive(Debug, Clone)]
pub struct SectionMeta {
    /// Display name for the section header
    pub display_name: String,
    /// Display order (lower = earlier)
    pub order: u32,
    /// Optional description
    pub description: Option<String>,
}

/// A registered option with its current value
#[derive(Debug, Clone)]
pub struct RegisteredOption {
    /// The option specification
    pub spec: OptionSpec,
    /// Current value
    pub value: OptionValue,
    /// Display order within section
    pub display_order: u32,
}

/// Settings menu state
#[derive(Debug, Clone, Default)]
pub struct SettingsMenuState {
    /// Whether the menu is visible
    pub visible: bool,
    /// Setting sections (built from registered options)
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

    // --- Dynamic Registration ---
    /// Registered sections (section_id -> metadata)
    pub registered_sections: HashMap<String, SectionMeta>,
    /// Registered options by section (section_id -> options)
    pub registered_options: HashMap<String, Vec<RegisteredOption>>,
    /// Set of registered option keys (for duplicate detection)
    registered_option_keys: std::collections::HashSet<String>,
}

impl SettingsMenuState {
    /// Create a new settings menu state
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // --- Dynamic Registration Methods ---

    /// Register a settings section.
    ///
    /// # Errors
    ///
    /// Logs an error if a section with the same ID already exists (first wins).
    pub fn register_section(&mut self, id: String, meta: SectionMeta) {
        if self.registered_sections.contains_key(&id) {
            error!("Duplicate settings section registration: '{}'. First registration wins.", id);
            return;
        }
        self.registered_sections.insert(id, meta);
    }

    /// Register an option from a `RegisterOption` event.
    ///
    /// # Errors
    ///
    /// Logs an error if an option with the same key already exists (first wins).
    pub fn register_option(&mut self, spec: &OptionSpec, value: &OptionValue) {
        // Skip options not meant for settings menu
        if !spec.show_in_menu {
            return;
        }

        let key = spec.name.to_string();
        if self.registered_option_keys.contains(&key) {
            error!("Duplicate option registration: '{}'. First registration wins.", key);
            return;
        }
        self.registered_option_keys.insert(key);

        // Determine section ID
        let section_id = spec.effective_section().to_string();

        // Create or get section entry
        let options = self.registered_options.entry(section_id).or_default();

        options.push(RegisteredOption {
            spec: spec.clone(),
            value: value.clone(),
            display_order: spec.display_order,
        });
    }

    /// Rebuild sections from registered options.
    ///
    /// This method is called when opening the settings menu to build
    /// the `sections` and `flat_items` from the dynamically registered
    /// sections and options.
    pub fn rebuild_from_registered(&mut self) {
        self.sections.clear();

        // Collect sections with their order
        let mut section_entries: Vec<(String, u32, String)> = self
            .registered_sections
            .iter()
            .map(|(id, meta)| (id.clone(), meta.order, meta.display_name.clone()))
            .collect();

        // Add sections for options that don't have explicit section registration
        for section_id in self.registered_options.keys() {
            if !self.registered_sections.contains_key(section_id) {
                // Auto-create section with default order
                section_entries.push((section_id.clone(), 100, section_id.clone()));
            }
        }

        // Sort sections by order
        section_entries.sort_by_key(|(_, order, _)| *order);

        // Build sections
        for (section_id, _, display_name) in section_entries {
            let mut items = Vec::new();

            if let Some(options) = self.registered_options.get(&section_id) {
                // Sort options by display_order
                let mut sorted_options: Vec<_> = options.iter().collect();
                sorted_options.sort_by_key(|opt| opt.display_order);

                for opt in sorted_options {
                    items.push(SettingValue::item_from_spec(&opt.spec, &opt.value));
                }
            }

            if !items.is_empty() {
                self.sections.push(SettingSection {
                    name: display_name,
                    items,
                });
            }
        }

        self.rebuild_flat_items();
    }

    /// Open the settings menu with current profile settings
    pub fn open(&mut self, _profile: &ProfileConfig, profile_name: &str) {
        self.visible = true;
        self.scroll_offset = 0;
        self.input_mode = SettingsInputMode::Normal;
        self.input_buffer.clear();
        self.input_prompt.clear();
        self.pending_action = None;
        self.message = None;

        // Build sections from dynamically registered options
        self.rebuild_from_registered();

        // Add profile management section (special case - not a registered option)
        self.sections
            .push(Self::build_profile_section(profile_name));
        self.rebuild_flat_items();

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

    /// Update a registered option value when receiving OptionChanged events.
    ///
    /// This method updates both the registered_options storage and any
    /// currently displayed sections.
    pub fn update_option_value(&mut self, key: &str, value: &OptionValue) {
        // Update in registered_options
        for options in self.registered_options.values_mut() {
            for opt in options {
                if opt.spec.name == key {
                    opt.value = value.clone();
                    break;
                }
            }
        }

        // Update in displayed sections (if menu is open)
        if self.visible {
            for section in &mut self.sections {
                for item in &mut section.items {
                    if item.key == key {
                        item.value = SettingValue::from_option_with_constraint(
                            value,
                            &self
                                .registered_options
                                .values()
                                .flatten()
                                .find(|o| o.spec.name == key)
                                .map(|o| o.spec.constraint.clone())
                                .unwrap_or_default(),
                        );
                        return;
                    }
                }
            }
        }
    }

    /// Build the profile management section
    fn build_profile_section(profile_name: &str) -> SettingSection {
        SettingSection {
            name: "Profile".to_string(),
            items: vec![
                SettingItem {
                    key: "profile.current".to_string(),
                    label: "Current".to_string(),
                    description: Some("Active profile".to_string()),
                    value: SettingValue::Display(profile_name.to_string()),
                },
                SettingItem {
                    key: "profile.save".to_string(),
                    label: "Save as...".to_string(),
                    description: Some("Save current settings to profile".to_string()),
                    value: SettingValue::Action(ActionType::SaveProfile),
                },
                SettingItem {
                    key: "profile.load".to_string(),
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

    /// Calculate layout based on screen dimensions and content
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn calculate_layout(&mut self, screen_width: u16, screen_height: u16) {
        // Width: 50% of screen, clamped between 40 and 60
        let width = ((f32::from(screen_width) * 0.5) as u16).clamp(40, 60);

        // Height: based on content + borders (2 for top/bottom)
        let content_items = self.flat_items.len();
        let content_height = (content_items + 2) as u16; // items + top/bottom border

        // Clamp height to reasonable bounds (min 10, max 80% of screen)
        let max_height = ((f32::from(screen_height) * 0.8) as u16).max(10);
        let height = content_height.clamp(10, max_height);

        // Center on screen
        let x = (screen_width.saturating_sub(width)) / 2;
        let y = (screen_height.saturating_sub(height)) / 2;

        // Visible items = height - 2 (borders)
        let visible_items = (height.saturating_sub(2)) as usize;

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

    /// Toggle the selected boolean setting. Returns change info if toggled.
    pub fn toggle_selected(&mut self) -> Option<SettingChange> {
        // Get the key and check if it's a boolean
        let (key, section_idx, item_idx) = {
            let flat_item = self.flat_items.get(self.selected_index)?;
            if let FlatItem::Setting {
                section_idx,
                item_idx,
            } = flat_item
            {
                let item = self.sections.get(*section_idx)?.items.get(*item_idx)?;
                if !item.value.is_bool() {
                    return None;
                }
                (item.key.clone(), *section_idx, *item_idx)
            } else {
                return None;
            }
        };

        // Toggle the value in sections
        let item = self
            .sections
            .get_mut(section_idx)?
            .items
            .get_mut(item_idx)?;
        let old_value = item.value.to_option_value()?;
        item.value.toggle();
        let new_value = item.value.to_option_value()?;

        // Also update registered_options
        for options in self.registered_options.values_mut() {
            for opt in options.iter_mut() {
                if opt.spec.name == key {
                    opt.value = new_value.clone();
                    break;
                }
            }
        }

        Some(SettingChange {
            key,
            old_value,
            new_value,
        })
    }

    /// Helper to sync a value change to registered_options
    fn sync_to_registered(&mut self, key: &str, new_value: &OptionValue) {
        for options in self.registered_options.values_mut() {
            for opt in options.iter_mut() {
                if opt.spec.name == key {
                    opt.value = new_value.clone();
                    return;
                }
            }
        }
    }

    /// Cycle to next value for selected setting. Returns change info if changed.
    pub fn cycle_next_selected(&mut self) -> Option<SettingChange> {
        let (key, section_idx, item_idx) = {
            let flat_item = self.flat_items.get(self.selected_index)?;
            if let FlatItem::Setting {
                section_idx,
                item_idx,
            } = flat_item
            {
                let item = self.sections.get(*section_idx)?.items.get(*item_idx)?;
                match &item.value {
                    SettingValue::Choice { .. } | SettingValue::Number { .. } => {}
                    _ => return None,
                }
                (item.key.clone(), *section_idx, *item_idx)
            } else {
                return None;
            }
        };

        let item = self
            .sections
            .get_mut(section_idx)?
            .items
            .get_mut(item_idx)?;
        let old_value = item.value.to_option_value()?;
        match &item.value {
            SettingValue::Choice { .. } => item.value.cycle_next(),
            SettingValue::Number { .. } => item.value.increment(),
            _ => return None,
        }
        let new_value = item.value.to_option_value()?;
        self.sync_to_registered(&key, &new_value);
        Some(SettingChange {
            key,
            old_value,
            new_value,
        })
    }

    /// Cycle to previous value for selected setting. Returns change info if changed.
    pub fn cycle_prev_selected(&mut self) -> Option<SettingChange> {
        let (key, section_idx, item_idx) = {
            let flat_item = self.flat_items.get(self.selected_index)?;
            if let FlatItem::Setting {
                section_idx,
                item_idx,
            } = flat_item
            {
                let item = self.sections.get(*section_idx)?.items.get(*item_idx)?;
                match &item.value {
                    SettingValue::Choice { .. } | SettingValue::Number { .. } => {}
                    _ => return None,
                }
                (item.key.clone(), *section_idx, *item_idx)
            } else {
                return None;
            }
        };

        let item = self
            .sections
            .get_mut(section_idx)?
            .items
            .get_mut(item_idx)?;
        let old_value = item.value.to_option_value()?;
        match &item.value {
            SettingValue::Choice { .. } => item.value.cycle_prev(),
            SettingValue::Number { .. } => item.value.decrement(),
            _ => return None,
        }
        let new_value = item.value.to_option_value()?;
        self.sync_to_registered(&key, &new_value);
        Some(SettingChange {
            key,
            old_value,
            new_value,
        })
    }

    /// Quick select for the selected setting. Returns change info if changed.
    pub fn quick_select(&mut self, index: u8) -> Option<SettingChange> {
        let (key, section_idx, item_idx) = {
            let flat_item = self.flat_items.get(self.selected_index)?;
            if let FlatItem::Setting {
                section_idx,
                item_idx,
            } = flat_item
            {
                let item = self.sections.get(*section_idx)?.items.get(*item_idx)?;
                if !item.value.is_choice() {
                    return None;
                }
                (item.key.clone(), *section_idx, *item_idx)
            } else {
                return None;
            }
        };

        let item = self
            .sections
            .get_mut(section_idx)?
            .items
            .get_mut(item_idx)?;
        let old_value = item.value.to_option_value()?;
        item.value.quick_select(index);
        let new_value = item.value.to_option_value()?;
        self.sync_to_registered(&key, &new_value);
        Some(SettingChange {
            key,
            old_value,
            new_value,
        })
    }

    /// Increment the selected number setting. Returns change info if changed.
    pub fn increment_selected(&mut self) -> Option<SettingChange> {
        let (key, section_idx, item_idx) = {
            let flat_item = self.flat_items.get(self.selected_index)?;
            if let FlatItem::Setting {
                section_idx,
                item_idx,
            } = flat_item
            {
                let item = self.sections.get(*section_idx)?.items.get(*item_idx)?;
                if !item.value.is_number() {
                    return None;
                }
                (item.key.clone(), *section_idx, *item_idx)
            } else {
                return None;
            }
        };

        let item = self
            .sections
            .get_mut(section_idx)?
            .items
            .get_mut(item_idx)?;
        let old_value = item.value.to_option_value()?;
        item.value.increment();
        let new_value = item.value.to_option_value()?;
        self.sync_to_registered(&key, &new_value);
        Some(SettingChange {
            key,
            old_value,
            new_value,
        })
    }

    /// Decrement the selected number setting. Returns change info if changed.
    pub fn decrement_selected(&mut self) -> Option<SettingChange> {
        let (key, section_idx, item_idx) = {
            let flat_item = self.flat_items.get(self.selected_index)?;
            if let FlatItem::Setting {
                section_idx,
                item_idx,
            } = flat_item
            {
                let item = self.sections.get(*section_idx)?.items.get(*item_idx)?;
                if !item.value.is_number() {
                    return None;
                }
                (item.key.clone(), *section_idx, *item_idx)
            } else {
                return None;
            }
        };

        let item = self
            .sections
            .get_mut(section_idx)?
            .items
            .get_mut(item_idx)?;
        let old_value = item.value.to_option_value()?;
        item.value.decrement();
        let new_value = item.value.to_option_value()?;
        self.sync_to_registered(&key, &new_value);
        Some(SettingChange {
            key,
            old_value,
            new_value,
        })
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

    /// Get the current profile config from settings state.
    ///
    /// This iterates over registered options and maps them to profile fields.
    /// Note: The proper flow is for OptionChanged events to update OptionRegistry,
    /// which then persists to profile. This method provides backwards compatibility.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn to_profile_config(&self) -> ProfileConfig {
        let mut config = ProfileConfig::default();

        // Iterate over all registered options
        for options in self.registered_options.values() {
            for opt in options {
                let key = opt.spec.name.as_ref();
                match key {
                    "number" => {
                        if let OptionValue::Bool(b) = &opt.value {
                            config.editor.number = *b;
                        }
                    }
                    "relativenumber" => {
                        if let OptionValue::Bool(b) = &opt.value {
                            config.editor.relativenumber = *b;
                        }
                    }
                    "tabwidth" => {
                        if let OptionValue::Integer(i) = &opt.value {
                            config.editor.tabwidth = (*i).clamp(1, 8) as u8;
                        }
                    }
                    "expandtab" => {
                        if let OptionValue::Bool(b) = &opt.value {
                            config.editor.expandtab = *b;
                        }
                    }
                    "indentguide" => {
                        if let OptionValue::Bool(b) = &opt.value {
                            config.editor.indentguide = *b;
                        }
                    }
                    "scrollbar" => {
                        if let OptionValue::Bool(b) = &opt.value {
                            config.editor.scrollbar = *b;
                        }
                    }
                    "scrolloff" => {
                        if let OptionValue::Integer(i) = &opt.value {
                            config.editor.scrolloff = (*i).max(0) as u16;
                        }
                    }
                    "theme" => {
                        if let OptionValue::Choice { value, .. } = &opt.value {
                            config.editor.theme = value.clone();
                        }
                    }
                    "colormode" => {
                        if let OptionValue::Choice { value, .. } = &opt.value {
                            config.editor.colormode = value.clone();
                        }
                    }
                    "splitbelow" => {
                        if let OptionValue::Bool(b) = &opt.value {
                            // If splitbelow is true, default_split should be "horizontal"
                            if *b {
                                config.window.default_split = "horizontal".to_string();
                            }
                        }
                    }
                    "splitright" => {
                        if let OptionValue::Bool(b) = &opt.value {
                            // If splitright is true, default_split should be "vertical"
                            if *b {
                                config.window.default_split = "vertical".to_string();
                            }
                        }
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

    /// Helper to register test options for settings menu tests
    fn register_test_options(state: &mut SettingsMenuState) {
        // Register a test section
        state.register_section(
            "Test".to_string(),
            SectionMeta {
                display_name: "Test".to_string(),
                order: 0,
                description: None,
            },
        );

        // Register a boolean option
        let bool_spec = OptionSpec::new("test_bool", "Test boolean", OptionValue::Bool(true))
            .with_section("Test")
            .with_display_order(10);
        state.register_option(&bool_spec, &OptionValue::Bool(true));

        // Register a choice option
        let choice_spec = OptionSpec::new(
            "test_choice",
            "Test choice",
            OptionValue::Choice {
                value: "a".to_string(),
                choices: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            },
        )
        .with_section("Test")
        .with_display_order(20);
        state.register_option(
            &choice_spec,
            &OptionValue::Choice {
                value: "a".to_string(),
                choices: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            },
        );
    }

    #[test]
    fn test_settings_menu_open_close() {
        let mut state = SettingsMenuState::new();
        assert!(!state.visible);

        // Register options before opening
        register_test_options(&mut state);

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

        // Register options before opening
        register_test_options(&mut state);

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

        // Register options before opening
        register_test_options(&mut state);

        let profile = ProfileConfig::default();
        state.open(&profile, "default");

        // Find a boolean setting
        let mut found_bool = false;
        for _ in 0..state.flat_items.len() {
            if let Some(item) = state.selected_item()
                && item.value.is_bool()
            {
                found_bool = true;
                break;
            }
            state.select_next();
        }

        assert!(found_bool, "Should find at least one boolean setting");

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

    /// Test the line number toggle scenario
    ///
    /// This simulates what happens when a user:
    /// 1. Opens settings menu (<SPC>s)
    /// 2. Navigates to "Show line numbers" option
    /// 3. Toggles it with Space
    /// 4. Views the rendered output
    #[test]
    fn test_line_number_toggle_scenario() {
        use {crate::settings_menu::item::SettingValue, reovim_core::option::OptionCategory};

        let mut state = SettingsMenuState::new();

        // Register the Editor section (like CorePlugin does)
        state.register_section(
            "Editor".to_string(),
            SectionMeta {
                display_name: "Editor".to_string(),
                order: 0,
                description: Some("Core editor settings".into()),
            },
        );

        // Register the "number" option (line numbers) like CorePlugin does
        let number_spec = OptionSpec::new("number", "Show line numbers", OptionValue::Bool(true))
            .with_short("nu")
            .with_category(OptionCategory::Editor)
            .with_section("Editor")
            .with_display_order(10);
        state.register_option(&number_spec, &OptionValue::Bool(true));

        // Open the menu
        let profile = ProfileConfig::default();
        state.open(&profile, "default");
        state.calculate_layout(120, 40);

        // Find the "number" setting
        let mut found_number = false;
        for _ in 0..state.flat_items.len() {
            if let Some(item) = state.selected_item()
                && item.key == "number"
            {
                found_number = true;
                break;
            }
            state.select_next();
        }

        assert!(found_number, "Should find the 'number' setting");

        // Verify initial value is true
        let item = state.selected_item().expect("Should have selected item");
        assert_eq!(item.key, "number");
        match &item.value {
            SettingValue::Bool(b) => assert!(*b, "Initial value should be true"),
            _ => panic!("number should be a Bool type"),
        }

        // Toggle the setting
        let change = state.toggle_selected();
        assert!(change.is_some(), "Toggle should return a SettingChange");

        let change = change.unwrap();
        assert_eq!(change.key, "number");
        assert_eq!(change.old_value, OptionValue::Bool(true));
        assert_eq!(change.new_value, OptionValue::Bool(false));

        // Verify sections are updated (what render reads)
        let item_after = state
            .selected_item()
            .expect("Should still have selected item");
        assert_eq!(item_after.key, "number");
        match &item_after.value {
            SettingValue::Bool(b) => assert!(!*b, "Value after toggle should be false"),
            _ => panic!("number should still be a Bool type"),
        }

        // Verify registered_options are synced
        let registered = state
            .registered_options
            .get("Editor")
            .expect("Should have Editor section in registered_options");
        let number_opt = registered
            .iter()
            .find(|o| o.spec.name == "number")
            .expect("Should find number in registered_options");
        assert_eq!(
            number_opt.value,
            OptionValue::Bool(false),
            "registered_options should have updated value"
        );

        // Test toggling back
        let change2 = state.toggle_selected();
        assert!(change2.is_some(), "Second toggle should return a SettingChange");

        let item_final = state.selected_item().expect("Should have selected item");
        match &item_final.value {
            SettingValue::Bool(b) => assert!(*b, "Value after second toggle should be true"),
            _ => panic!("number should still be a Bool type"),
        }
    }

    /// Test that the value displayed in render reflects toggle changes
    #[test]
    fn test_render_reflects_toggle() {
        use reovim_core::option::OptionCategory;

        let mut state = SettingsMenuState::new();

        // Register section and option
        state.register_section(
            "Editor".to_string(),
            SectionMeta {
                display_name: "Editor".to_string(),
                order: 0,
                description: None,
            },
        );

        let number_spec = OptionSpec::new("number", "Show line numbers", OptionValue::Bool(true))
            .with_category(OptionCategory::Editor)
            .with_section("Editor")
            .with_display_order(10);
        state.register_option(&number_spec, &OptionValue::Bool(true));

        // Open menu
        state.open(&ProfileConfig::default(), "default");
        state.calculate_layout(120, 40);

        // Navigate to number setting
        for _ in 0..state.flat_items.len() {
            if let Some(item) = state.selected_item()
                && item.key == "number"
            {
                break;
            }
            state.select_next();
        }

        // Get display value BEFORE toggle
        let before = state.selected_item().unwrap().value.display_value();
        assert_eq!(before, "on", "Display should be 'on' initially");

        // Toggle
        state.toggle_selected();

        // Get display value AFTER toggle (simulates what render reads)
        let after = state.selected_item().unwrap().value.display_value();
        assert_eq!(after, "off", "Display should be 'off' after toggle");

        // Simulate re-reading state (like render does with Clone::clone)
        let cloned_state = state.clone();
        let cloned_item = cloned_state.selected_item().unwrap();
        let cloned_display = cloned_item.value.display_value();
        assert_eq!(cloned_display, "off", "Cloned state should also show 'off'");
    }
}
