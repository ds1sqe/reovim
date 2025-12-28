//! Landing page content with animated lion ASCII art
//!
//! Three responsive sizes with unique animations:
//! - **Large** (35x16+): Roar animation - dramatic mouth opening with sound waves
//! - **Medium** (30x12+): Sleep animation - eyelids closing with zzZ
//! - **Small** (fallback): Breathing animation - subtle mane expansion

mod frames;
mod sprite;

pub use frames::LogoSize;
pub use sprite::{AnimationMode, AsciiSprite};

use frames::{LARGE_ROAR_FRAMES, MEDIUM_SLEEP_FRAMES, SMALL_BREATHING_FRAMES};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Landing page state with animation
pub struct LandingState {
    sprite: AsciiSprite,
    size: LogoSize,
}

impl LandingState {
    /// Create a new landing state for the given terminal dimensions
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let size = LogoSize::from_dimensions(width, height);
        let (frames, mode) = match size {
            LogoSize::Large => (LARGE_ROAR_FRAMES, AnimationMode::Loop),
            LogoSize::Medium => (MEDIUM_SLEEP_FRAMES, AnimationMode::PingPong),
            LogoSize::Small => (SMALL_BREATHING_FRAMES, AnimationMode::Loop),
        };

        let sprite = AsciiSprite::new(
            frames,
            size.frame_duration_ms(),
            size.pause_duration_ms(),
            mode,
        );

        Self { sprite, size }
    }

    /// Advance animation by `delta_ms`, returns true if content changed
    pub fn tick(&mut self, delta_ms: f32) -> bool {
        self.sprite.tick(delta_ms)
    }

    /// Generate the current landing page content
    #[must_use]
    pub fn generate(&self, width: u16, height: u16) -> String {
        generate_with_frame(self.sprite.current_frame(), width, height)
    }

    /// Get current logo size
    #[must_use]
    pub const fn size(&self) -> LogoSize {
        self.size
    }

    /// Resize to new terminal dimensions
    pub fn resize(&mut self, width: u16, height: u16) {
        let new_size = LogoSize::from_dimensions(width, height);
        if new_size != self.size {
            *self = Self::new(width, height);
        }
    }
}

/// Generate landing page content (static, for backward compatibility)
#[must_use]
pub fn generate(width: u16, height: u16) -> String {
    let size = LogoSize::from_dimensions(width, height);
    let frame = size.frames().first().copied().unwrap_or(&[]);
    generate_with_frame(frame, width, height)
}

/// Generate landing page with a specific animation frame
fn generate_with_frame(frame: &[&str], width: u16, height: u16) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Help hints
    let version_line = format!("version {VERSION}");
    let hints: [&str; 7] = [
        "",
        &version_line,
        "",
        "Type :q<Enter> to exit",
        "Type i to enter insert mode",
        "Type :help<Enter> for help",
        "",
    ];

    // Calculate total content height
    let content_height = frame.len() + hints.len();
    let start_row = if height as usize > content_height {
        (height as usize - content_height) / 2
    } else {
        0
    };

    // Add empty lines for vertical centering
    for _ in 0..start_row {
        lines.push(String::new());
    }

    // Add frame lines (centered as a block)
    let frame_width = frame.iter().map(|l| l.len()).max().unwrap_or(0);
    let frame_padding = if (width as usize) > frame_width {
        (width as usize - frame_width) / 2
    } else {
        0
    };
    for line in frame {
        lines.push(format!("{:padding$}{}", "", line, padding = frame_padding));
    }

    // Add hint lines (centered)
    for hint in &hints {
        lines.push(center_text(hint, width));
    }

    // Fill remaining lines
    while lines.len() < height as usize {
        lines.push(String::new());
    }

    lines.join("\n")
}

/// Center text within the given width
fn center_text(text: &str, width: u16) -> String {
    let text_len = text.chars().count();
    if text_len >= width as usize {
        return text.to_string();
    }
    let padding = (width as usize - text_len) / 2;
    format!("{:padding$}{}", "", text, padding = padding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_selection() {
        // Large terminal (50+ cols, 24+ rows)
        assert_eq!(LogoSize::from_dimensions(80, 24), LogoSize::Large);
        assert_eq!(LogoSize::from_dimensions(50, 24), LogoSize::Large);
        // Medium terminal (35+ cols, 16+ rows)
        assert_eq!(LogoSize::from_dimensions(40, 18), LogoSize::Medium);
        assert_eq!(LogoSize::from_dimensions(49, 23), LogoSize::Medium); // Just under large
        // Small terminal (fallback)
        assert_eq!(LogoSize::from_dimensions(20, 10), LogoSize::Small);
        assert_eq!(LogoSize::from_dimensions(34, 15), LogoSize::Small); // Just under medium
    }

    #[test]
    fn test_generate_not_empty() {
        let content = generate(80, 24);
        assert!(!content.is_empty());
        assert!(content.contains("REOVIM"));
    }

    #[test]
    fn test_landing_state_animation() {
        let mut state = LandingState::new(80, 24);
        let initial = state.generate(80, 24);

        // Tick past frame duration
        state.tick(300.0);
        let after_tick = state.generate(80, 24);

        // Content should have changed (different frame)
        assert_ne!(initial, after_tick);
    }
}
