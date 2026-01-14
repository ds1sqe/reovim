//! Chrome rendering for the editor (status line, tab line, command line)
//!
//! This module contains rendering logic for the "chrome" - the UI elements
//! surrounding the editor content:
//! - Status line (mode, file name, position, plugin sections)
//! - Tab line (when multiple tabs are open)
//! - Command line (when in command mode)

use crate::{
    buffer::Buffer,
    command_line::CommandLine,
    frame::FrameBuffer,
    highlight::{ColorMode, Style, Theme},
    modd::ModeState,
    plugin::{SectionAlignment, StatuslineRenderContext},
};

use super::super::Screen;

// =============================================================================
// Status Line
// =============================================================================

/// Result of querying status line animation effects
pub(in crate::screen) struct StatusLineAnimationInfo {
    /// Resolved style from the animation (if bg/fg transition active)
    pub style: Option<Style>,
    /// Sweep configuration for position-based glow (if sweep active)
    pub sweep: Option<crate::animation::SweepConfig>,
}

impl Screen {
    /// Render status line to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::similar_names)]
    #[allow(clippy::too_many_lines)]
    pub(in crate::screen) fn render_status_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        mode: &ModeState,
        current_buffer: Option<&Buffer>,
        pending_keys: &str,
        _last_command: &str,
        theme: &Theme,
        _color_mode: ColorMode,
        plugin_state: &std::sync::Arc<crate::plugin::PluginStateRegistry>,
        display_registry: &crate::display::DisplayRegistry,
    ) {
        let y = self.size.height.saturating_sub(1);
        let mut x = 0u16;
        let status_line_width = self.size.width;

        // Get base styles
        let interactor_style = &theme.statusline.interactor;
        let base_mode_style = display_registry.mode_style(mode, theme);
        let separator = &theme.statusline.separator;

        // Check for animation effects on the status line
        let anim_info = Self::get_animation_info(plugin_state);

        // Use animated style if present (for mode flash), otherwise use base
        let mode_style = anim_info.style.as_ref().unwrap_or(base_mode_style);

        // Helper to apply sweep effect at current position
        let apply_sweep = |style: &Style, pos_x: u16| -> Style {
            anim_info.sweep.as_ref().map_or_else(
                || style.clone(),
                |sweep| {
                    let position = f32::from(pos_x) / f32::from(status_line_width.max(1));
                    Self::apply_sweep_to_style(style, sweep, position)
                },
            )
        };

        // === Section 1: Interactor (e.g., "Editor", "Explorer") ===
        // Query DisplayRegistry for plugin-provided display info
        let (interactor_text, interactor_icon, actual_interactor_style) =
            display_registry.get_display(mode).map_or_else(
                || {
                    // Fallback for unregistered components
                    (
                        format!(" {} ", mode.interactor_id.0),
                        "",               // No icon fallback
                        interactor_style, // theme.statusline.interactor
                    )
                },
                |info| {
                    // Use plugin-provided info, with dynamic display support
                    let text = info.dynamic_display.as_ref().map_or_else(
                        || info.display_string.to_string(),
                        |dynamic| {
                            let ctx = crate::display::DisplayContext { plugin_state };
                            dynamic(&ctx)
                        },
                    );
                    (text, info.icon, &info.style)
                },
            );

        // Render icon if present
        for ch in interactor_icon.chars() {
            if x < buffer.width() {
                let style = apply_sweep(actual_interactor_style, x);
                buffer.put_char(x, y, ch, &style);
                x += 1;
            }
        }

        // Render text
        for ch in interactor_text.chars() {
            if x < buffer.width() {
                let style = apply_sweep(actual_interactor_style, x);
                buffer.put_char(x, y, ch, &style);
                x += 1;
            }
        }

        // Powerline separator: interactor -> mode
        let sep_style = Self::create_separator_style(actual_interactor_style, mode_style);
        for ch in separator.left.chars() {
            if x < buffer.width() {
                let style = apply_sweep(&sep_style, x);
                buffer.put_char(x, y, ch, &style);
                x += 1;
            }
        }

        // === Section 2: Mode (e.g., "Normal", "Insert", "Visual") ===
        let mode_name = display_registry.mode_name(mode);
        let mode_text = format!(" {mode_name} ");
        for ch in mode_text.chars() {
            if x < buffer.width() {
                let style = apply_sweep(mode_style, x);
                buffer.put_char(x, y, ch, &style);
                x += 1;
            }
        }

        // Powerline separator: mode -> background
        let bg_style = &theme.statusline.background;
        let sep_style2 = Self::create_separator_style(mode_style, bg_style);
        for ch in separator.left.chars() {
            if x < buffer.width() {
                let style = apply_sweep(&sep_style2, x);
                buffer.put_char(x, y, ch, &style);
                x += 1;
            }
        }

        // File name
        let file_style = &theme.statusline.filename;
        if let Some(buf) = current_buffer {
            let file_name = buf.file_path.as_deref().unwrap_or("[No Name]");
            let file_text = format!(" {file_name} ");
            for ch in file_text.chars() {
                if x < buffer.width() {
                    let style = apply_sweep(file_style, x);
                    buffer.put_char(x, y, ch, &style);
                    x += 1;
                }
            }

            // Modified indicator
            if buf.modified {
                let modified_style = &theme.statusline.modified;
                let style = apply_sweep(modified_style, x);
                buffer.put_char(x, y, '[', &style);
                x += 1;
                let style = apply_sweep(modified_style, x);
                buffer.put_char(x, y, '+', &style);
                x += 1;
                let style = apply_sweep(modified_style, x);
                buffer.put_char(x, y, ']', &style);
                x += 1;
            }
        }

        // === Plugin sections ===
        // Get sections from provider (if any)
        let plugin_sections = plugin_state.statusline_provider().map(|provider| {
            let (active_buffer_id, buffer_content, cursor_row, cursor_col) =
                current_buffer.map_or((None, None, None, None), |buf| {
                    let snapshot = crate::buffer::BufferSnapshot::from_buffer(buf);
                    (
                        Some(buf.id),
                        Some(snapshot.content()),
                        Some(u32::from(buf.cur.y)),
                        Some(u32::from(buf.cur.x)),
                    )
                });

            let ctx = StatuslineRenderContext {
                plugin_state,
                screen_width: self.size.width,
                status_row: y,
                active_buffer_id,
                buffer_content,
                cursor_row,
                cursor_col,
            };
            let mut sections = provider.render_sections(&ctx);

            // Sort by alignment and priority
            sections.sort_by(|a, b| {
                a.alignment
                    .cmp(&b.alignment)
                    .then_with(|| a.priority.cmp(&b.priority))
            });
            sections
        });

        // Render left-aligned sections
        if let Some(ref sections) = plugin_sections {
            for section in sections
                .iter()
                .filter(|s| s.alignment == SectionAlignment::Left)
            {
                let section_style = section.style.as_ref().unwrap_or(bg_style);
                for ch in section.text.chars() {
                    if x < buffer.width() {
                        let style = apply_sweep(section_style, x);
                        buffer.put_char(x, y, ch, &style);
                        x += 1;
                    }
                }
            }
        }

        // Calculate right section total width
        let right_section_width: u16 = plugin_sections.as_ref().map_or(0, |sections| {
            sections
                .iter()
                .filter(|s| s.alignment == SectionAlignment::Right)
                .map(|s| s.text.chars().count() as u16)
                .sum()
        });

        // Fill middle with background style (account for right sections)
        let fill_end = self
            .size
            .width
            .saturating_sub(pending_keys.len() as u16 + 10 + right_section_width);
        let bg_style = &theme.statusline.background;
        while x < fill_end {
            let style = apply_sweep(bg_style, x);
            buffer.put_char(x, y, ' ', &style);
            x += 1;
        }

        // Render right-aligned sections (before position info)
        if let Some(ref sections) = plugin_sections {
            for section in sections
                .iter()
                .filter(|s| s.alignment == SectionAlignment::Right)
            {
                let section_style = section.style.as_ref().unwrap_or(bg_style);
                for ch in section.text.chars() {
                    if x < buffer.width() {
                        let style = apply_sweep(section_style, x);
                        buffer.put_char(x, y, ch, &style);
                        x += 1;
                    }
                }
            }
        }

        // Position info (right side)
        if let Some(buf) = current_buffer {
            let pos_text = format!("{}:{} ", buf.cur.y + 1, buf.cur.x + 1);
            let pos_style = &theme.statusline.position;

            // Right-align position
            let pos_start = self
                .size
                .width
                .saturating_sub(pos_text.len() as u16 + pending_keys.len() as u16);
            for (i, ch) in pos_text.chars().enumerate() {
                let px = pos_start + i as u16;
                if px < buffer.width() {
                    let style = apply_sweep(pos_style, px);
                    buffer.put_char(px, y, ch, &style);
                }
            }
        }

        // Pending keys (far right)
        if !pending_keys.is_empty() {
            let keys_style = &theme.statusline.filetype;
            let keys_start = self.size.width.saturating_sub(pending_keys.len() as u16);
            for (i, ch) in pending_keys.chars().enumerate() {
                let px = keys_start + i as u16;
                if px < buffer.width() {
                    let style = apply_sweep(keys_style, px);
                    buffer.put_char(px, y, ch, &style);
                }
            }
        }
    }

    /// Create a separator style that transitions between two backgrounds
    pub(in crate::screen) fn create_separator_style(from: &Style, to: &Style) -> Style {
        // For powerline separators, fg is the "from" background, bg is the "to" background
        Style::new().fg_opt(from.bg).bg_opt(to.bg)
    }

    /// Get animated style and sweep info from active `StatusLine` effects
    ///
    /// Queries the animation state for active effects targeting `UiElement::StatusLine`
    /// and returns both the resolved style and sweep configuration if present.
    /// Uses non-blocking `try_read()` to avoid blocking the render loop.
    pub(in crate::screen) fn get_animation_info(
        plugin_state: &std::sync::Arc<crate::plugin::PluginStateRegistry>,
    ) -> StatusLineAnimationInfo {
        use crate::animation::{EffectTarget, UiElementId};

        let default = StatusLineAnimationInfo {
            style: None,
            sweep: None,
        };

        // Get the shared animation state
        let Some(animation_state) = plugin_state.animation_state() else {
            return default;
        };

        // Try non-blocking read - if locked, skip animation for this frame
        let Ok(state_guard) = animation_state.try_read() else {
            return default;
        };

        // Look for StatusLine effects
        let target = EffectTarget::UiElement(UiElementId::StatusLine);
        let Some(effects) = state_guard.get_effects(&target) else {
            return default;
        };

        // Get the highest priority effect
        let Some(effect) = effects.iter().max_by_key(|e| e.priority) else {
            return default;
        };

        // Extract style and sweep info
        let style = if effect.style.fg.is_some() || effect.style.bg.is_some() {
            Some(effect.resolve_style())
        } else {
            None
        };
        let sweep = effect.sweep;

        drop(state_guard);
        StatusLineAnimationInfo { style, sweep }
    }

    /// Brighten a color for sweep glow effect
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(in crate::screen) fn brighten_color_for_sweep(
        color: reovim_sys::style::Color,
        intensity: f32,
    ) -> reovim_sys::style::Color {
        use reovim_sys::style::Color;

        // Extract RGB components
        let (r, g, b) = match color {
            Color::Rgb { r, g, b } => (r, g, b),
            Color::AnsiValue(n) => Self::sweep_ansi_to_rgb(n),
            Color::Black => (0, 0, 0),
            Color::DarkGrey | Color::Reset => (128, 128, 128),
            Color::Red => (255, 0, 0),
            Color::DarkRed => (139, 0, 0),
            Color::Green => (0, 255, 0),
            Color::DarkGreen => (0, 100, 0),
            Color::Yellow => (255, 255, 0),
            Color::DarkYellow => (128, 128, 0),
            Color::Blue => (0, 0, 255),
            Color::DarkBlue => (0, 0, 139),
            Color::Magenta => (255, 0, 255),
            Color::DarkMagenta => (139, 0, 139),
            Color::Cyan => (0, 255, 255),
            Color::DarkCyan => (0, 139, 139),
            Color::White => (255, 255, 255),
            Color::Grey => (192, 192, 192),
        };

        // Blend towards white using mul_add for better precision
        let blend = |c: u8| -> u8 {
            let f = f32::from(c) / 255.0;
            let brightened = (1.0 - f).mul_add(intensity, f);
            (brightened.min(1.0) * 255.0) as u8
        };

        Color::Rgb {
            r: blend(r),
            g: blend(g),
            b: blend(b),
        }
    }

    /// Convert ANSI 256 color to RGB (for sweep effect)
    pub(in crate::screen) const fn sweep_ansi_to_rgb(n: u8) -> (u8, u8, u8) {
        match n {
            0 => (0, 0, 0),
            1 => (128, 0, 0),
            2 => (0, 128, 0),
            3 => (128, 128, 0),
            4 => (0, 0, 128),
            5 => (128, 0, 128),
            6 => (0, 128, 128),
            7 => (192, 192, 192),
            8 => (128, 128, 128),
            9 => (255, 0, 0),
            10 => (0, 255, 0),
            11 => (255, 255, 0),
            12 => (0, 0, 255),
            13 => (255, 0, 255),
            14 => (0, 255, 255),
            15 => (255, 255, 255),
            16..=231 => {
                let idx = n - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let r_val = if r == 0 { 0 } else { 55 + r * 40 };
                let g_val = if g == 0 { 0 } else { 55 + g * 40 };
                let b_val = if b == 0 { 0 } else { 55 + b * 40 };
                (r_val, g_val, b_val)
            }
            232..=255 => {
                let gray = 8 + (n - 232) * 10;
                (gray, gray, gray)
            }
        }
    }

    /// Apply sweep glow to a style at a given position
    pub(in crate::screen) fn apply_sweep_to_style(
        base_style: &Style,
        sweep: &crate::animation::SweepConfig,
        position: f32,
    ) -> Style {
        let brightness = sweep.brightness_at(position);
        if brightness <= 0.0 {
            return base_style.clone();
        }

        let mut style = base_style.clone();
        if let Some(bg) = style.bg {
            style.bg = Some(Self::brighten_color_for_sweep(bg, brightness));
        }
        if let Some(fg) = style.fg {
            // Slightly brighten fg too for glow effect
            style.fg = Some(Self::brighten_color_for_sweep(fg, brightness * 0.3));
        }
        style
    }
}

// =============================================================================
// Tab Line
// =============================================================================

impl Screen {
    /// Render tab line to frame buffer
    ///
    /// Displays tabs when multiple tabs are open. Shows active tab with
    /// highlighted style and inactive tabs with dimmed style.
    pub(in crate::screen) fn render_tab_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        _color_mode: ColorMode,
        theme: &Theme,
    ) {
        let tabs = self.tab_manager().tab_info();
        if tabs.len() <= 1 {
            return;
        }

        let mut x = 0u16;
        for tab in &tabs {
            let style = if tab.is_active {
                &theme.tab.active
            } else {
                &theme.tab.inactive
            };

            let label = format!(" {} ", tab.label);
            for ch in label.chars() {
                if x < buffer.width() {
                    buffer.put_char(x, 0, ch, style);
                    x += 1;
                }
            }
        }

        // Fill rest with tab fill style
        let fill_style = &theme.tab.fill;
        while x < buffer.width() {
            buffer.put_char(x, 0, ' ', fill_style);
            x += 1;
        }
    }
}

// =============================================================================
// Command Line
// =============================================================================

impl Screen {
    /// Render command line to frame buffer
    ///
    /// Displays the command prompt (":") followed by user input on the
    /// bottom line of the screen.
    #[allow(clippy::cast_possible_truncation)]
    pub(in crate::screen) fn render_command_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        cmd_line: &CommandLine,
        theme: &Theme,
    ) {
        let y = self.size.height.saturating_sub(1);
        let style = &theme.base.default;

        // Write colon prompt
        buffer.put_char(0, y, ':', style);

        // Write command text
        for (i, ch) in cmd_line.input.chars().enumerate() {
            let x = 1 + i as u16;
            if x < buffer.width() {
                buffer.put_char(x, y, ch, style);
            }
        }

        // Clear rest of line
        let input_len = cmd_line.input.len() as u16;
        for x in (1 + input_len)..buffer.width() {
            buffer.put_char(x, y, ' ', style);
        }
    }
}

// =============================================================================
// Style Extension Trait
// =============================================================================

/// Extension trait for Style to allow optional color setting (used in screen rendering)
pub(in crate::screen) trait StyleExt {
    fn fg_opt(self, color: Option<reovim_sys::style::Color>) -> Self;
    fn bg_opt(self, color: Option<reovim_sys::style::Color>) -> Self;
}

impl StyleExt for Style {
    fn fg_opt(mut self, color: Option<reovim_sys::style::Color>) -> Self {
        self.fg = color;
        self
    }

    fn bg_opt(mut self, color: Option<reovim_sys::style::Color>) -> Self {
        self.bg = color;
        self
    }
}
