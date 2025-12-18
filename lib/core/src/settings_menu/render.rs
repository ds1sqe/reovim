//! Settings menu rendering

use {
    super::{
        item::{FlatItem, SettingValue},
        state::{SettingsInputMode, SettingsMenuState},
    },
    crate::highlight::{ColorMode, Theme},
};

/// Box drawing characters
const BORDER_TOP_LEFT: char = '┌';
const BORDER_TOP_RIGHT: char = '┐';
const BORDER_BOTTOM_LEFT: char = '└';
const BORDER_BOTTOM_RIGHT: char = '┘';
const BORDER_HORIZONTAL: char = '─';
const BORDER_VERTICAL: char = '│';

/// Setting type indicators
const CHECKBOX_CHECKED: &str = "☑";
const CHECKBOX_UNCHECKED: &str = "☐";
const CHOICE_INDICATOR: &str = "●";
const ACTION_INDICATOR: &str = "○";

impl SettingsMenuState {
    /// Render the settings menu to a vector of positioned lines
    ///
    /// Returns `Vec<(line_content, x, y)>` for screen rendering
    #[must_use]
    pub fn render(&self, theme: &Theme, color_mode: ColorMode) -> Vec<(String, u16, u16)> {
        if !self.visible {
            return Vec::new();
        }

        let mut lines = Vec::new();
        let layout = &self.layout;
        let width = layout.width as usize;

        // Style codes
        let border_style = theme.telescope.border.to_ansi_start(color_mode);
        let normal_style = theme.telescope.normal.to_ansi_start(color_mode);
        let selected_style = theme.telescope.selected.to_ansi_start(color_mode);
        let reset = "\x1b[0m";

        // Top border with title
        let title = " Settings ";
        let title_pos = (width.saturating_sub(title.len())) / 2;
        let top_border = format!(
            "{border_style}{BORDER_TOP_LEFT}{left}{title}{right}{BORDER_TOP_RIGHT}{reset}",
            left = BORDER_HORIZONTAL
                .to_string()
                .repeat(title_pos.saturating_sub(1)),
            right = BORDER_HORIZONTAL
                .to_string()
                .repeat(width.saturating_sub(title_pos + title.len() + 1)),
        );
        lines.push((top_border, layout.x, layout.y));

        // Content area
        let content_width = width.saturating_sub(4); // 2 for borders, 2 for padding
        let mut y = layout.y + 1;

        // Calculate visible range
        let visible_start = self.scroll_offset;
        let visible_end = (visible_start + layout.visible_items).min(self.flat_items.len());

        for (idx, flat_item) in self.flat_items[visible_start..visible_end]
            .iter()
            .enumerate()
        {
            let global_idx = visible_start + idx;
            let is_selected = global_idx == self.selected_index;

            let line_content = match flat_item {
                FlatItem::SectionHeader(name) => Self::render_section_header(name, content_width),
                FlatItem::Setting {
                    section_idx,
                    item_idx,
                } => self
                    .sections
                    .get(*section_idx)
                    .and_then(|s| s.items.get(*item_idx))
                    .map_or_else(
                        || Self::render_empty_line(content_width),
                        |item| {
                            Self::render_setting_item(
                                &item.label,
                                &item.value,
                                content_width,
                                is_selected,
                            )
                        },
                    ),
            };

            // Wrap with border and selection styling
            let styled_line = if is_selected && flat_item.is_setting() {
                format!(
                    "{border_style}{BORDER_VERTICAL}{reset} {selected_style}{line_content}{reset} {border_style}{BORDER_VERTICAL}{reset}"
                )
            } else {
                format!(
                    "{border_style}{BORDER_VERTICAL}{reset} {normal_style}{line_content}{reset} {border_style}{BORDER_VERTICAL}{reset}"
                )
            };

            lines.push((styled_line, layout.x, y));
            y += 1;
        }

        // Fill remaining visible area with empty lines
        let lines_rendered = visible_end - visible_start;
        for _ in lines_rendered..layout.visible_items {
            let empty = Self::render_empty_line(content_width);
            let line = format!(
                "{border_style}{BORDER_VERTICAL}{reset} {normal_style}{empty}{reset} {border_style}{BORDER_VERTICAL}{reset}"
            );
            lines.push((line, layout.x, y));
            y += 1;
        }

        // Footer: either input prompt or keybinding hints
        let footer_line = if self.input_mode == SettingsInputMode::TextInput {
            let input_line =
                Self::render_input_prompt(&self.input_prompt, &self.input_buffer, content_width);
            format!(
                "{border_style}{BORDER_VERTICAL}{reset} {selected_style}{input_line}{reset} {border_style}{BORDER_VERTICAL}{reset}"
            )
        } else {
            let footer = Self::render_footer(content_width);
            format!(
                "{border_style}{BORDER_VERTICAL}{reset} {normal_style}{footer}{reset} {border_style}{BORDER_VERTICAL}{reset}"
            )
        };
        lines.push((footer_line, layout.x, y));
        y += 1;

        // Bottom border
        let bottom_border = format!(
            "{border_style}{BORDER_BOTTOM_LEFT}{}{BORDER_BOTTOM_RIGHT}{reset}",
            BORDER_HORIZONTAL
                .to_string()
                .repeat(width.saturating_sub(2)),
        );
        lines.push((bottom_border, layout.x, y));

        lines
    }

    /// Render a section header
    fn render_section_header(name: &str, width: usize) -> String {
        let header = format!("[{name}]");
        let padding = width.saturating_sub(header.len());
        format!("{header}{}", " ".repeat(padding))
    }

    /// Render a setting item line
    fn render_setting_item(
        label: &str,
        value: &SettingValue,
        width: usize,
        is_selected: bool,
    ) -> String {
        match value {
            SettingValue::Bool(b) => {
                let checkbox = if *b {
                    CHECKBOX_CHECKED
                } else {
                    CHECKBOX_UNCHECKED
                };
                let text = format!("{checkbox} {label}");
                let padding = width.saturating_sub(text.chars().count());
                format!("{text}{}", " ".repeat(padding))
            }
            SettingValue::Choice { options, selected } => {
                let current = options.get(*selected).map_or("", String::as_str);
                let hint = Self::format_choice_hint(options.len(), is_selected);
                let dots = Self::make_dots(label, current, &hint, width);
                let text = format!("{CHOICE_INDICATOR} {label} {dots} {current}  {hint}");
                let padding = width.saturating_sub(text.chars().count());
                format!("{text}{}", " ".repeat(padding))
            }
            SettingValue::Number { value: val, .. } => {
                let hint = if is_selected { "[+/-]" } else { "" };
                let val_str = val.to_string();
                let dots = Self::make_dots(label, &val_str, hint, width);
                let text = format!("{CHOICE_INDICATOR} {label} {dots} {val_str}  {hint}");
                let padding = width.saturating_sub(text.chars().count());
                format!("{text}{}", " ".repeat(padding))
            }
            SettingValue::Display(s) => {
                let dots = Self::make_dots(label, s, "", width);
                let text = format!("{CHOICE_INDICATOR} {label} {dots} {s}");
                let padding = width.saturating_sub(text.chars().count());
                format!("{text}{}", " ".repeat(padding))
            }
            SettingValue::Action(_) => {
                let text = format!("{ACTION_INDICATOR} {label}");
                let padding = width.saturating_sub(text.chars().count());
                format!("{text}{}", " ".repeat(padding))
            }
        }
    }

    /// Create dots between label and value
    fn make_dots(label: &str, value: &str, hint: &str, width: usize) -> String {
        // Account for: indicator (2) + spaces (4) + label + value + hint
        let used = 2 + 4 + label.len() + value.len() + hint.len();
        let dots_len = width.saturating_sub(used).max(3);
        ".".repeat(dots_len)
    }

    /// Format choice hint like [1/2/3]
    fn format_choice_hint(count: usize, is_selected: bool) -> String {
        if !is_selected {
            return String::new();
        }
        let numbers: Vec<String> = (1..=count).map(|n| n.to_string()).collect();
        format!("[{}]", numbers.join("/"))
    }

    /// Render an empty line
    fn render_empty_line(width: usize) -> String {
        " ".repeat(width)
    }

    /// Render the footer with keybinding hints
    fn render_footer(width: usize) -> String {
        let hints = "j/k: navigate  Space: toggle  h/l: cycle  Esc: close";
        let padding = width.saturating_sub(hints.len());
        let left_pad = padding / 2;
        let right_pad = padding - left_pad;
        format!("{}{hints}{}", " ".repeat(left_pad), " ".repeat(right_pad))
    }

    /// Render the input prompt line
    fn render_input_prompt(prompt: &str, value: &str, width: usize) -> String {
        // Format: "Profile name: my-profile█"
        let cursor = "█";
        let text = format!("{prompt}: {value}{cursor}");
        let char_count = text.chars().count();
        let padding = width.saturating_sub(char_count);
        format!("{text}{}", " ".repeat(padding))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{config::ProfileConfig, highlight::Theme},
    };

    #[test]
    fn test_render_not_visible() {
        let state = SettingsMenuState::new();
        let theme = Theme::default();
        let lines = state.render(&theme, ColorMode::TrueColor);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_render_visible() {
        let mut state = SettingsMenuState::new();
        let profile = ProfileConfig::default();
        state.open(&profile, "default");
        state.calculate_layout(120, 40);

        let theme = Theme::default();
        let lines = state.render(&theme, ColorMode::TrueColor);

        // Should have: top border + visible items + footer + bottom border
        assert!(!lines.is_empty());
        // First line should be top border
        assert!(lines[0].0.contains("Settings"));
    }

    #[test]
    fn test_section_header_format() {
        let header = SettingsMenuState::render_section_header("Editor", 40);
        assert!(header.starts_with("[Editor]"));
    }
}
