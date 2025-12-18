//! Which-key popup panel for displaying available keybindings

use std::collections::BTreeMap;

use crate::{
    event::WhichKeyBinding,
    highlight::{ColorMode, Style, Theme},
    overlay::{OverlayBounds, OverlayGeometry},
};

/// Default panel width in characters
const DEFAULT_PANEL_WIDTH: u16 = 40;

/// Group display order and labels
const GROUP_ORDER: &[(&str, &str)] = &[
    ("motion", "Motion"),
    ("operator", "Operator"),
    ("textobj", "Text Object"),
    ("mode", "Mode"),
    ("edit", "Edit"),
    ("jump", "Jump"),
    ("fold", "Fold"),
    ("telescope", "Telescope"),
    ("window", "Window"),
    ("misc", "Misc"),
];

/// Panel configuration
#[derive(Debug, Clone)]
pub struct WhichKeyConfig {
    /// Panel width (in characters)
    pub width: u16,
    /// Separator between key and description
    pub separator: &'static str,
}

impl Default for WhichKeyConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_PANEL_WIDTH,
            separator: " -> ",
        }
    }
}

/// State for the which-key popup panel
#[derive(Default)]
pub struct WhichKeyPanel {
    /// Whether the panel is visible
    pub visible: bool,
    /// Current prefix being shown
    pub prefix: String,
    /// Available bindings for the current prefix
    pub bindings: Vec<WhichKeyBinding>,
    /// Panel configuration
    pub config: WhichKeyConfig,
}

impl WhichKeyPanel {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the panel with bindings for a prefix
    pub fn show(&mut self, prefix: String, bindings: Vec<WhichKeyBinding>) {
        self.visible = true;
        self.prefix = prefix;
        self.bindings = bindings;
    }

    /// Hide the panel
    pub fn hide(&mut self) {
        self.visible = false;
        self.prefix.clear();
        self.bindings.clear();
    }

    /// Render the panel content as lines.
    ///
    /// Returns a Vec of tuples containing the line content and its x/y position.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn render(
        &self,
        screen_width: u16,
        screen_height: u16,
        color_mode: ColorMode,
        theme: &Theme,
    ) -> Vec<(String, u16, u16)> {
        if !self.visible || self.bindings.is_empty() {
            return Vec::new();
        }

        let mut lines = Vec::new();
        let panel_width = self.config.width.min(screen_width / 3);
        let panel_x = screen_width.saturating_sub(panel_width);

        // Group bindings by category
        let grouped = self.group_bindings();

        // Calculate total content lines (bindings + group headers)
        let total_lines: usize = grouped
            .iter()
            .map(|(_, bindings)| bindings.len() + 1) // +1 for group header
            .sum();

        // Calculate panel height (content + main header + footer)
        let max_height = screen_height.saturating_sub(2); // Leave room for status line
        let content_height = ((total_lines + 2) as u16).min(max_height);
        let panel_y = screen_height
            .saturating_sub(content_height)
            .saturating_sub(1);

        // Use theme styles
        let styles = &theme.whichkey;

        // Header line
        let header = self.format_header(panel_width as usize, &styles.border, color_mode);
        lines.push((header, panel_x, panel_y));

        // Render grouped bindings
        let mut current_y = panel_y + 1;
        let max_y = panel_y + content_height - 1;

        for (group_label, bindings) in &grouped {
            if current_y >= max_y {
                break;
            }

            // Group header
            let group_header = Self::format_group_header(
                group_label,
                panel_width as usize,
                &styles.border,
                color_mode,
            );
            lines.push((group_header, panel_x, current_y));
            current_y += 1;

            // Bindings in this group
            for binding in bindings {
                if current_y >= max_y {
                    break;
                }

                let line = self.format_binding(
                    binding,
                    panel_width as usize,
                    &styles.key,
                    &styles.description,
                    &styles.prefix,
                    &styles.background,
                    color_mode,
                );
                lines.push((line, panel_x, current_y));
                current_y += 1;
            }
        }

        // Footer line
        let footer = Self::format_footer(panel_width as usize, &styles.border, color_mode);
        lines.push((footer, panel_x, current_y));

        lines
    }

    /// Group bindings by their category, maintaining display order
    fn group_bindings(&self) -> Vec<(&str, Vec<&WhichKeyBinding>)> {
        // First, collect bindings into groups
        let mut groups: BTreeMap<&str, Vec<&WhichKeyBinding>> = BTreeMap::new();
        let mut ungrouped: Vec<&WhichKeyBinding> = Vec::new();

        for binding in &self.bindings {
            if let Some(ref group) = binding.group {
                groups.entry(group.as_str()).or_default().push(binding);
            } else {
                ungrouped.push(binding);
            }
        }

        // Build result in display order
        let mut result: Vec<(&str, Vec<&WhichKeyBinding>)> = Vec::new();

        for &(group_key, group_label) in GROUP_ORDER {
            if let Some(bindings) = groups.remove(group_key) {
                result.push((group_label, bindings));
            }
        }

        // Add any remaining groups not in ORDER (shouldn't happen but be safe)
        for (group_key, bindings) in groups {
            result.push((group_key, bindings));
        }

        // Add ungrouped bindings at the end
        if !ungrouped.is_empty() {
            result.push(("Other", ungrouped));
        }

        result
    }

    fn format_header(&self, width: usize, bg_style: &Style, color_mode: ColorMode) -> String {
        let title = format!(" {} ", self.prefix);
        let padding = width.saturating_sub(title.len());
        let left_pad = padding / 2;
        let right_pad = padding - left_pad;

        format!(
            "{}{}{}{}{}",
            bg_style.to_ansi_start(color_mode),
            "─".repeat(left_pad),
            title,
            "─".repeat(right_pad),
            Style::ansi_reset()
        )
    }

    fn format_group_header(
        label: &str,
        width: usize,
        bg_style: &Style,
        color_mode: ColorMode,
    ) -> String {
        let title = format!("─ {label} ");
        let padding = width.saturating_sub(title.chars().count() + 2); // +2 for border chars

        format!(
            "{}│{}{}│{}",
            bg_style.to_ansi_start(color_mode),
            title,
            "─".repeat(padding),
            Style::ansi_reset()
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn format_binding(
        &self,
        binding: &WhichKeyBinding,
        width: usize,
        key_style: &Style,
        desc_style: &Style,
        prefix_style: &Style,
        bg_style: &Style,
        color_mode: ColorMode,
    ) -> String {
        let prefix_indicator = if binding.is_prefix { "+" } else { " " };
        let key_part = format!("{}{}", prefix_indicator, binding.key);
        let sep = self.config.separator;

        // Calculate available space for description
        let used = key_part.chars().count() + sep.len() + 2; // +2 for border chars
        let desc_width = width.saturating_sub(used);

        // Truncate description if needed
        let desc: String = if binding.description.chars().count() > desc_width {
            let truncated: String = binding
                .description
                .chars()
                .take(desc_width.saturating_sub(3))
                .collect();
            format!("{truncated}...")
        } else {
            binding.description.clone()
        };

        let padding = desc_width.saturating_sub(desc.chars().count());

        // Use prefix style for prefix indicator if it's a prefix
        let indicator_style = if binding.is_prefix {
            prefix_style
        } else {
            key_style
        };

        format!(
            "{}│{}{}{}{}{}{}{}│{}",
            bg_style.to_ansi_start(color_mode),
            indicator_style.to_ansi_start(color_mode),
            key_part,
            Style::ansi_reset(),
            desc_style.to_ansi_start(color_mode),
            sep,
            desc,
            " ".repeat(padding),
            Style::ansi_reset()
        )
    }

    fn format_footer(width: usize, bg_style: &Style, color_mode: ColorMode) -> String {
        format!(
            "{}{}{}",
            bg_style.to_ansi_start(color_mode),
            "─".repeat(width),
            Style::ansi_reset()
        )
    }
}

// === Overlay Trait Implementations ===

impl OverlayGeometry for WhichKeyPanel {
    /// Compute bounds for the which-key panel
    ///
    /// Panel is positioned at the right edge, bottom of screen.
    #[allow(clippy::cast_possible_truncation)]
    fn compute_bounds(&self, screen_width: u16, screen_height: u16) -> OverlayBounds {
        let panel_width = self.config.width.min(screen_width / 3);
        let panel_x = screen_width.saturating_sub(panel_width);

        // Height based on bindings count
        let max_height = screen_height.saturating_sub(2);
        let content_height = (self.bindings.len() as u16 + 2).min(max_height);
        let panel_y = screen_height
            .saturating_sub(content_height)
            .saturating_sub(1);

        OverlayBounds::new(panel_x, panel_y, panel_width, content_height)
    }
}
