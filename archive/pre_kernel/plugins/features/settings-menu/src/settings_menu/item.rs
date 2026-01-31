//! Settings menu item types

use reovim_core::option::{OptionConstraint, OptionSpec, OptionValue};

/// A section grouping related settings
#[derive(Debug, Clone)]
pub struct SettingSection {
    /// Section name (e.g., "Editor", "Cursor", "Window")
    pub name: String,
    /// Settings in this section
    pub items: Vec<SettingItem>,
}

/// A single setting item
#[derive(Debug, Clone)]
pub struct SettingItem {
    /// Internal key (e.g., "editor.theme", "plugin.treesitter.timeout")
    ///
    /// Changed from `&'static str` to `String` to support dynamic registration.
    pub key: String,
    /// Display label
    pub label: String,
    /// Help text (optional)
    pub description: Option<String>,
    /// The setting value and type
    pub value: SettingValue,
}

/// Types of setting values
#[derive(Debug, Clone)]
pub enum SettingValue {
    /// Boolean toggle (checkbox)
    Bool(bool),
    /// Multiple choice (dropdown)
    Choice {
        options: Vec<String>,
        selected: usize,
    },
    /// Numeric value with bounds
    Number {
        value: i32,
        min: i32,
        max: i32,
        step: i32,
    },
    /// Read-only display value
    Display(String),
    /// Action button
    Action(ActionType),
}

/// Types of actions that can be triggered
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    SaveProfile,
    LoadProfile,
    ResetToDefault,
}

/// Flattened item for navigation (includes section headers)
#[derive(Debug, Clone)]
pub enum FlatItem {
    /// Section header (not selectable for value changes)
    SectionHeader(String),
    /// Setting item reference
    Setting { section_idx: usize, item_idx: usize },
}

impl SettingValue {
    /// Toggle a boolean value
    pub const fn toggle(&mut self) {
        if let Self::Bool(b) = self {
            *b = !*b;
        }
    }

    /// Cycle to next choice option
    pub const fn cycle_next(&mut self) {
        if let Self::Choice { options, selected } = self {
            *selected = (*selected + 1) % options.len();
        }
    }

    /// Cycle to previous choice option
    pub const fn cycle_prev(&mut self) {
        if let Self::Choice { options, selected } = self {
            if *selected == 0 {
                *selected = options.len() - 1;
            } else {
                *selected -= 1;
            }
        }
    }

    /// Quick select a choice by index (1-based)
    pub const fn quick_select(&mut self, index: u8) {
        if let Self::Choice { options, selected } = self {
            let idx = (index as usize).saturating_sub(1);
            if idx < options.len() {
                *selected = idx;
            }
        }
    }

    /// Increment a number value
    pub fn increment(&mut self) {
        if let Self::Number {
            value, max, step, ..
        } = self
        {
            *value = (*value + *step).min(*max);
        }
    }

    /// Decrement a number value
    pub fn decrement(&mut self) {
        if let Self::Number {
            value, min, step, ..
        } = self
        {
            *value = (*value - *step).max(*min);
        }
    }

    /// Get the current value as a display string
    #[must_use]
    pub fn display_value(&self) -> String {
        match self {
            Self::Bool(b) => if *b { "on" } else { "off" }.to_string(),
            Self::Choice { options, selected } => {
                options.get(*selected).cloned().unwrap_or_default()
            }
            Self::Number { value, .. } => value.to_string(),
            Self::Display(s) => s.clone(),
            Self::Action(_) => String::new(),
        }
    }

    /// Check if this is a toggleable bool
    #[must_use]
    pub const fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// Check if this is a choice/cycle value
    #[must_use]
    pub const fn is_choice(&self) -> bool {
        matches!(self, Self::Choice { .. })
    }

    /// Check if this is a number value
    #[must_use]
    pub const fn is_number(&self) -> bool {
        matches!(self, Self::Number { .. })
    }

    /// Check if this is an action
    #[must_use]
    pub const fn is_action(&self) -> bool {
        matches!(self, Self::Action(_))
    }

    /// Check if this is a display-only value
    #[must_use]
    pub const fn is_display(&self) -> bool {
        matches!(self, Self::Display(_))
    }

    /// Get choice options count (for rendering [1/2/3] hints)
    #[must_use]
    pub const fn choice_count(&self) -> Option<usize> {
        if let Self::Choice { options, .. } = self {
            Some(options.len())
        } else {
            None
        }
    }
}

impl FlatItem {
    /// Check if this is a section header
    #[must_use]
    pub const fn is_header(&self) -> bool {
        matches!(self, Self::SectionHeader(_))
    }

    /// Check if this is a setting
    #[must_use]
    pub const fn is_setting(&self) -> bool {
        matches!(self, Self::Setting { .. })
    }
}

// --- Conversions between OptionValue (core) and SettingValue (plugin) ---

impl From<&OptionValue> for SettingValue {
    /// Convert an `OptionValue` to a `SettingValue` without constraint information.
    ///
    /// For integers, uses unbounded min/max. Use `SettingValue::from_option_with_constraint`
    /// for proper bounds.
    fn from(opt: &OptionValue) -> Self {
        match opt {
            OptionValue::Bool(b) => Self::Bool(*b),
            OptionValue::Integer(i) => Self::Number {
                value: *i as i32,
                min: i32::MIN,
                max: i32::MAX,
                step: 1,
            },
            OptionValue::String(s) => Self::Display(s.clone()),
            OptionValue::Choice { value, choices } => Self::Choice {
                options: choices.clone(),
                selected: choices.iter().position(|c| c == value).unwrap_or(0),
            },
        }
    }
}

impl SettingValue {
    /// Create a `SettingValue` from an `OptionValue` with constraint information.
    ///
    /// Uses the constraint's min/max for integer bounds.
    #[must_use]
    pub fn from_option_with_constraint(opt: &OptionValue, constraint: &OptionConstraint) -> Self {
        match opt {
            OptionValue::Bool(b) => Self::Bool(*b),
            OptionValue::Integer(i) => Self::Number {
                value: *i as i32,
                min: constraint.min.map(|v| v as i32).unwrap_or(i32::MIN),
                max: constraint.max.map(|v| v as i32).unwrap_or(i32::MAX),
                step: 1,
            },
            OptionValue::String(s) => Self::Display(s.clone()),
            OptionValue::Choice { value, choices } => Self::Choice {
                options: choices.clone(),
                selected: choices.iter().position(|c| c == value).unwrap_or(0),
            },
        }
    }

    /// Create a `SettingItem` from an `OptionSpec` with its current value.
    #[must_use]
    pub fn item_from_spec(spec: &OptionSpec, value: &OptionValue) -> SettingItem {
        SettingItem {
            key: spec.name.to_string(),
            label: spec.description.to_string(),
            description: Some(spec.description.to_string()),
            value: Self::from_option_with_constraint(value, &spec.constraint),
        }
    }

    /// Convert this `SettingValue` back to an `OptionValue`.
    ///
    /// Note: `Display` and `Action` variants cannot be converted and return `None`.
    #[must_use]
    pub fn to_option_value(&self) -> Option<OptionValue> {
        match self {
            Self::Bool(b) => Some(OptionValue::Bool(*b)),
            Self::Number { value, .. } => Some(OptionValue::Integer(i64::from(*value))),
            Self::Choice { options, selected } => {
                let value = options.get(*selected).cloned().unwrap_or_default();
                Some(OptionValue::Choice {
                    value,
                    choices: options.clone(),
                })
            }
            Self::Display(_) | Self::Action(_) => None,
        }
    }
}
