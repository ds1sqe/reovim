#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Startup landing screen chrome module (#657: animated).
//!
//! Displays a centered overlay with ASCII art, version info, and quick-action
//! hints when the editor starts with no file argument. Dismissed on any user
//! interaction (cursor move, mode change, or buffer update).
//!
//! # Animation (#657)
//!
//! The border and logo cycle through a breathing color animation (6 frames,
//! 500ms per frame). Every 8 seconds, a brief roar flash plays (4 frames,
//! 100ms per frame) before returning to breathing.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use {
    reovim_arch::clock::{Clock, SystemClock},
    reovim_client_driver::{
        BufferId, BufferUpdateEvent, ChromePosition, ChromeSurface, Rect, Style, types::Color,
    },
    reovim_client_subsys_module::{
        ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities, ProbeResult, Version,
    },
};

/// ASCII art logo lines (compact, ~40 chars wide).
const LOGO: &[&str] = &[
    r"  ____                  _            ",
    r" |  _ \ ___  _____   _(_)_ __ ___   ",
    r" | |_) / _ \/ _ \ \ / / | '_ ` _ \  ",
    r" |  _ <  __/ (_) \ V /| | | | | | | ",
    r" |_| \_\___|\___/ \_/ |_|_| |_| |_| ",
];

/// Version string derived at compile time.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Quick-action hint lines.
const ACTIONS: &[&str] = &[
    "  e         New file",
    "  :e <file> Open file",
    "  :q        Quit",
    "  ?         Help",
];

/// Footer text.
const FOOTER: &str = "Press any key to start";

/// Minimum terminal width to render the landing screen.
const MIN_WIDTH: u16 = 44;

/// Minimum terminal height to render the landing screen.
const MIN_HEIGHT: u16 = 16;

/// Box width (accommodates logo + padding).
const BOX_WIDTH: u16 = 42;

const KIND: &str = "landing";

// =============================================================================
// Animation constants
// =============================================================================

/// Breathing color palette (6 frames, cycling).
const BREATHING_COLORS: [Color; 6] = [
    Color::Rgb {
        r: 0,
        g: 180,
        b: 220,
    }, // Bright cyan
    Color::Rgb {
        r: 0,
        g: 160,
        b: 200,
    }, // Medium cyan
    Color::Rgb {
        r: 0,
        g: 140,
        b: 180,
    }, // Subdued cyan
    Color::Rgb {
        r: 0,
        g: 120,
        b: 160,
    }, // Dim cyan
    Color::Rgb {
        r: 0,
        g: 140,
        b: 180,
    }, // Subdued (return)
    Color::Rgb {
        r: 0,
        g: 160,
        b: 200,
    }, // Medium (return)
];

/// Roar color palette (4 frames, one-shot).
const ROAR_COLORS: [Color; 4] = [
    Color::White,
    Color::Rgb {
        r: 255,
        g: 200,
        b: 50,
    }, // Gold flash
    Color::Rgb {
        r: 0,
        g: 200,
        b: 255,
    }, // Bright cyan return
    Color::Cyan, // Normal
];

/// Duration per breathing frame.
const BREATHING_FRAME_DURATION: Duration = Duration::from_millis(500);

/// Duration per roar frame.
const ROAR_FRAME_DURATION: Duration = Duration::from_millis(100);

/// Interval between roar triggers.
const ROAR_INTERVAL: Duration = Duration::from_secs(8);

/// Startup landing screen module.
///
/// Starts active and dismisses permanently on the first user interaction.
/// Animates the border and logo with a breathing color cycle (#657).
pub struct LandingModule {
    dismissed: bool,
    clock: Arc<dyn Clock>,
    /// Time when the last frame advanced.
    last_frame_time: Instant,
    /// Current frame index.
    current_frame: usize,
    /// Whether roar animation is playing.
    roar_active: bool,
    /// Time of last roar start (for interval tracking).
    last_roar_time: Instant,
}

impl LandingModule {
    /// Create a new landing module (starts active, with system clock).
    #[must_use]
    pub fn new() -> Self {
        let clock = Arc::new(SystemClock);
        let now = clock.now();
        Self {
            dismissed: false,
            clock,
            last_frame_time: now,
            current_frame: 0,
            roar_active: false,
            last_roar_time: now,
        }
    }

    /// Create with custom clock (for deterministic testing).
    #[must_use]
    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        let now = clock.now();
        Self {
            dismissed: false,
            clock,
            last_frame_time: now,
            current_frame: 0,
            roar_active: false,
            last_roar_time: now,
        }
    }

    /// Get the current animation color for the border and logo.
    const fn animation_color(&self) -> Color {
        if self.roar_active {
            ROAR_COLORS[self.current_frame % ROAR_COLORS.len()]
        } else {
            BREATHING_COLORS[self.current_frame % BREATHING_COLORS.len()]
        }
    }
}

impl Default for LandingModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for LandingModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Landing"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        vec![]
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Overlay
    }

    fn chrome_priority(&self) -> u16 {
        30
    }

    fn on_cursor_update(&mut self, _buffer_id: BufferId, _line: usize, _col: usize) {
        self.dismissed = true;
    }

    fn on_mode_change(&mut self, _mode: &str) {
        self.dismissed = true;
    }

    fn on_buffer_update(&mut self, _event: &BufferUpdateEvent) {
        self.dismissed = true;
    }

    fn tick(&mut self) -> bool {
        if self.dismissed {
            return false;
        }

        let now = self.clock.now();

        let frame_duration = if self.roar_active {
            ROAR_FRAME_DURATION
        } else {
            BREATHING_FRAME_DURATION
        };

        let elapsed = now.duration_since(self.last_frame_time);
        if elapsed < frame_duration {
            return false;
        }

        // Advance frame
        self.current_frame += 1;
        self.last_frame_time = now;

        // Handle roar completion
        if self.roar_active && self.current_frame >= ROAR_COLORS.len() {
            self.roar_active = false;
            self.current_frame = 0;
            self.last_roar_time = now;
        }

        // Check if it's time for a roar
        if !self.roar_active && now.duration_since(self.last_roar_time) >= ROAR_INTERVAL {
            self.roar_active = true;
            self.current_frame = 0;
            self.last_frame_time = now;
        }

        true
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn ChromeSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if self.dismissed {
            return;
        }

        let (width, height) = (bounds.width, bounds.height);

        // Graceful degradation: skip if terminal too small.
        if width < MIN_WIDTH || height < MIN_HEIGHT {
            return;
        }

        // Compute layout.
        //   logo:    5 lines
        //   version: 1 line
        //   blank:   1 line
        //   actions: 4 lines
        //   blank:   1 line
        //   footer:  1 line
        //   borders: 2 lines (top + bottom)
        //   padding: 1 line top + 1 line bottom
        let content_height: u16 = 5 + 1 + 1 + 4 + 1 + 1; // 13
        let box_height = content_height + 2 + 2; // 17 (borders + padding)

        let box_x = width.saturating_sub(BOX_WIDTH) / 2;
        let box_y = height.saturating_sub(box_height) / 2;

        let anim_color = self.animation_color();
        let border_style = Style::new().fg(anim_color);
        let bg = Color::Rgb {
            r: 30,
            g: 30,
            b: 46,
        };
        let bg_style = Style::new().bg(bg);

        // Fill background inside box.
        surface.fill(
            Rect {
                x: box_x,
                y: box_y,
                width: BOX_WIDTH,
                height: box_height,
            },
            ' ',
            bg_style,
        );

        // Draw border.
        reovim_client_driver::chrome_utils::render_box_border(
            surface,
            box_x,
            box_y,
            BOX_WIDTH,
            box_height,
            &border_style,
        );

        // Content starts inside the box (1 border + 1 padding).
        let content_x = box_x + 2;
        let inner_width = BOX_WIDTH.saturating_sub(4);
        let mut y = box_y + 2;

        // Logo (centered within inner width).
        let logo_style = Style::new().fg(anim_color).bg(bg);
        for line in LOGO {
            let line_len = line.len() as u16;
            let logo_x = content_x + inner_width.saturating_sub(line_len) / 2;
            surface.write_styled(logo_x, y, line, logo_style.clone());
            y += 1;
        }

        // Version string (centered).
        let version_text = format!("v{VERSION}");
        let version_len = version_text.len() as u16;
        let version_style = Style::new().fg(Color::White).bg(bg);
        let version_x = content_x + inner_width.saturating_sub(version_len) / 2;
        surface.write_styled(version_x, y, &version_text, version_style);
        y += 1;

        // Blank line.
        y += 1;

        // Action hints.
        let action_style = Style::new().fg(Color::White).bg(bg);
        let key_style = Style::new().fg(Color::Yellow).bg(bg);
        for line in ACTIONS {
            // Highlight the key portion (up to first space after leading spaces).
            let trimmed = line.trim_start();
            let leading = line.len() - trimmed.len();
            let key_end = trimmed.find(' ').map_or(trimmed.len(), |i| i + leading);

            surface.write_styled(content_x, y, &line[..key_end], key_style.clone());
            #[allow(clippy::cast_possible_truncation)]
            let key_end_u16 = key_end as u16;
            surface.write_styled(
                content_x + key_end_u16,
                y,
                &line[key_end..],
                action_style.clone(),
            );
            y += 1;
        }

        // Blank line.
        y += 1;

        // Footer (centered, dimmed).
        let footer_len = FOOTER.len() as u16;
        let footer_style = Style::new().fg(Color::DarkGrey).bg(bg);
        let footer_x = content_x + inner_width.saturating_sub(footer_len) / 2;
        surface.write_styled(footer_x, y, FOOTER, footer_style);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
