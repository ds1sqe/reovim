//! Animation effects for visual feedback
//!
//! This module contains animation-related methods for the Runtime, including:
//! - Mode transition animations (status line flash)
//! - Idle shimmer effect
//! - Yank/paste visual feedback animations
//! - Color manipulation utilities

use std::time::Duration;

use crate::{
    animation::{AnimatedStyle, Effect, EffectId, EffectTarget},
    modd::{EditMode, ModeState, SubMode},
};

use super::Runtime;

impl Runtime {
    /// Start the idle shimmer effect on the status line
    pub(crate) fn start_idle_shimmer(&mut self) {
        use crate::animation::{SweepConfig, UiElementId};

        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Create a sweep effect: 2 second period, 20% width glow, 40% intensity
        let sweep = SweepConfig::new(2000, 0.2, 0.4);

        // Create shimmer effect with sweep
        let shimmer_effect = Effect::new(
            EffectId::new(0), // ID assigned by handle.start()
            EffectTarget::UiElement(UiElementId::StatusLine),
            AnimatedStyle::new(), // Base style, shimmer comes from sweep
        )
        .with_sweep(sweep)
        .with_priority(50); // Lower priority than mode flash

        if handle.start(shimmer_effect).is_some() {
            self.idle_shimmer_active = true;
            tracing::debug!("Started idle shimmer effect");
        }
    }

    /// Stop the idle shimmer effect
    pub(crate) fn stop_idle_shimmer(&mut self) {
        use crate::animation::UiElementId;

        if let Some(handle) = self.plugin_state.animation_handle() {
            handle.stop_target(EffectTarget::UiElement(UiElementId::StatusLine));
            self.idle_shimmer_active = false;
            tracing::debug!("Stopped idle shimmer effect");
        }
    }

    /// Trigger visual animation effects when mode changes
    ///
    /// Creates a brief status line flash effect to visually indicate mode transitions.
    /// The effect is a bright flash that fades to the mode's background color.
    pub(crate) fn trigger_mode_animation(&self, new_mode: &ModeState) {
        use crate::animation::UiElementId;

        // Get animation handle from plugin state
        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Get the new mode's background color for the flash effect
        let mode_color = self.get_mode_bg_color(new_mode);

        // Create a bright flash color (boost the mode color towards white)
        let flash_color = Self::brighten_color(mode_color, 0.6);

        // Create a status line flash effect: bright flash → mode color
        // This provides a "glowing" indication of mode change
        let flash_effect = Effect::new(
            EffectId::new(0), // ID will be assigned by handle.start()
            EffectTarget::UiElement(UiElementId::StatusLine),
            AnimatedStyle::transition_bg(
                flash_color,
                mode_color,
                250, // 250ms duration for smooth fade
            ),
        )
        .with_duration(Duration::from_millis(250));

        // Start the flash effect
        if let Some(id) = handle.start(flash_effect) {
            tracing::debug!("Started status line flash animation with effect id {:?}", id);
        }
    }

    /// Brighten a color by blending it towards white
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) fn brighten_color(
        color: reovim_sys::style::Color,
        intensity: f32,
    ) -> reovim_sys::style::Color {
        use reovim_sys::style::Color;

        // Extract RGB components
        let (r, g, b) = match color {
            Color::Rgb { r, g, b } => (r, g, b),
            Color::AnsiValue(n) => Self::ansi_to_rgb(n),
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

    /// Convert ANSI 256 color to RGB
    pub(crate) const fn ansi_to_rgb(n: u8) -> (u8, u8, u8) {
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

    /// Get the background color for a mode from the theme
    pub(crate) fn get_mode_bg_color(&self, mode: &ModeState) -> reovim_sys::style::Color {
        let mode_styles = &self.theme.statusline.mode;

        // Check sub-modes first
        match &mode.sub_mode {
            SubMode::Command => {
                return mode_styles
                    .command
                    .bg
                    .unwrap_or(reovim_sys::style::Color::Blue);
            }
            SubMode::OperatorPending { .. } => {
                return mode_styles
                    .operator_pending
                    .bg
                    .unwrap_or(reovim_sys::style::Color::Yellow);
            }
            SubMode::Interactor(_) | SubMode::None => {}
        }

        // Then check edit mode
        match &mode.edit_mode {
            EditMode::Normal => mode_styles
                .normal
                .bg
                .unwrap_or(reovim_sys::style::Color::Blue),
            EditMode::Insert(_) => mode_styles
                .insert
                .bg
                .unwrap_or(reovim_sys::style::Color::Green),
            EditMode::Visual(_) => mode_styles
                .visual
                .bg
                .unwrap_or(reovim_sys::style::Color::Magenta),
        }
    }

    /// Trigger yank blink animation for the yanked region
    ///
    /// Creates a brief flash effect on the yanked text range to provide visual feedback.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn trigger_yank_animation(
        &self,
        buffer_id: usize,
        start_pos: crate::screen::Position,
        motion: crate::motion::Motion,
        count: usize,
        _line_count: usize,
    ) {
        use crate::buffer::calculate_motion;

        let Some(buffer) = self.buffers.get(&buffer_id) else {
            return;
        };

        // Calculate the target position
        let target = calculate_motion(&buffer.contents, start_pos, motion, count);

        // For linewise motions, animate entire lines
        if motion.is_linewise() {
            let start_line = start_pos.y.min(target.y);
            let end_line = start_pos.y.max(target.y);

            // Get the length of the last line for end column
            #[allow(clippy::cast_possible_truncation)]
            let end_col = buffer
                .contents
                .get(end_line as usize)
                .map_or(0, |line| line.inner.len() as u16);

            self.trigger_yank_range_animation(
                buffer_id,
                crate::screen::Position {
                    x: 0,
                    y: start_line,
                },
                crate::screen::Position {
                    x: end_col,
                    y: end_line,
                },
            );
        } else {
            // For characterwise motions, use position-based range
            let (start, end) =
                if start_pos.y < target.y || (start_pos.y == target.y && start_pos.x <= target.x) {
                    (start_pos, target)
                } else {
                    (target, start_pos)
                };

            // Adjust end for inclusive motions
            let end = if motion.is_inclusive() {
                crate::screen::Position {
                    x: end.x.saturating_add(1),
                    y: end.y,
                }
            } else {
                end
            };

            self.trigger_yank_range_animation(buffer_id, start, end);
        }
    }

    /// Trigger yank blink animation for a specific range
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn trigger_yank_range_animation(
        &self,
        buffer_id: usize,
        start: crate::screen::Position,
        end: crate::screen::Position,
    ) {
        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Create a brief flash effect on the yanked range
        // Use a semi-transparent yellow/gold color for yank highlight
        let yank_color = reovim_sys::style::Color::Rgb {
            r: 255,
            g: 220,
            b: 100,
        };
        let transparent = reovim_sys::style::Color::Rgb {
            r: 40,
            g: 40,
            b: 50,
        };

        // Create cell region effect
        let yank_effect = Effect::new(
            EffectId::new(0),
            EffectTarget::CellRegion {
                buffer_id,
                start_line: u32::from(start.y),
                start_col: u32::from(start.x),
                end_line: u32::from(end.y),
                end_col: u32::from(end.x),
            },
            AnimatedStyle::transition_bg(yank_color, transparent, 200),
        )
        .with_duration(Duration::from_millis(200))
        .with_priority(100); // High priority for yank feedback

        if let Some(id) = handle.start(yank_effect) {
            tracing::debug!(
                "Started yank blink animation with effect id {:?} for range {:?}-{:?}",
                id,
                start,
                end
            );
        }
    }

    /// Trigger a paste animation to highlight the pasted region
    ///
    /// Creates a pulsing cyan effect that oscillates between bright and dark cyan
    /// over 600ms (2 complete pulse cycles). This provides visual feedback when
    /// text is pasted, similar to the yank animation but with a different color
    /// and animation type.
    ///
    /// # Arguments
    /// * `buffer_id` - The buffer where paste occurred
    /// * `start` - Starting position of the pasted region
    /// * `end` - Ending position of the pasted region
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn trigger_paste_animation(
        &self,
        buffer_id: usize,
        start: crate::screen::Position,
        end: crate::screen::Position,
    ) {
        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Bright cyan for paste highlight
        let paste_bright = reovim_sys::style::Color::Rgb {
            r: 100,
            g: 220,
            b: 255,
        };
        // Darker cyan for pulse low point
        let paste_dark = reovim_sys::style::Color::Rgb {
            r: 50,
            g: 110,
            b: 150,
        };

        // Create pulse effect (cyan oscillation, 300ms period)
        let paste_effect = Effect::new(
            EffectId::new(0),
            EffectTarget::CellRegion {
                buffer_id,
                start_line: u32::from(start.y),
                start_col: u32::from(start.x),
                end_line: u32::from(end.y),
                end_col: u32::from(end.x),
            },
            AnimatedStyle::pulse_bg(paste_bright, paste_dark, 300),
        )
        .with_duration(Duration::from_millis(600)) // 2 full pulse cycles
        .with_priority(100); // High priority for paste feedback

        if let Some(id) = handle.start(paste_effect) {
            tracing::debug!(
                "Started paste pulse animation with effect id {:?} for range {:?}-{:?}",
                id,
                start,
                end
            );
        }
    }
}
