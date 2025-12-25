//! Animated color types and interpolation utilities
//!
//! This module provides color types that can change over time,
//! used for visual effects like pulsing, shimmering, and transitions.

use reovim_sys::style::Color;

use super::easing::EasingFn;

/// A color that can be static or animated over time
#[derive(Debug, Clone)]
pub enum AnimatedColor {
    /// Static color (no animation)
    Static(Color),

    /// Pulsing between two colors
    Pulse {
        /// Starting color
        from: Color,
        /// Ending color
        to: Color,
        /// Duration of one full cycle in milliseconds
        period_ms: u32,
        /// Current phase (0.0 - 1.0), updated by tick
        phase: f32,
    },

    /// Shimmer effect (subtle brightness variation)
    Shimmer {
        /// Base color
        base: Color,
        /// Brightness variation intensity (0.0 - 1.0)
        intensity: f32,
        /// Period in milliseconds
        period_ms: u32,
        /// Current phase (0.0 - 1.0)
        phase: f32,
    },

    /// Smooth transition from one color to another
    Transition {
        /// Starting color
        from: Color,
        /// Target color
        to: Color,
        /// Total duration in milliseconds
        duration_ms: u32,
        /// Progress (0.0 = `from`, 1.0 = `to`)
        progress: f32,
        /// Easing function
        easing: EasingFn,
    },
}

impl AnimatedColor {
    /// Create a static (non-animated) color
    #[must_use]
    pub const fn new(color: Color) -> Self {
        Self::Static(color)
    }

    /// Create a pulsing color effect
    #[must_use]
    pub const fn pulse(from: Color, to: Color, period_ms: u32) -> Self {
        Self::Pulse {
            from,
            to,
            period_ms,
            phase: 0.0,
        }
    }

    /// Create a shimmer effect
    #[must_use]
    pub const fn shimmer(base: Color, intensity: f32, period_ms: u32) -> Self {
        Self::Shimmer {
            base,
            intensity,
            period_ms,
            phase: 0.0,
        }
    }

    /// Create a transition effect
    #[must_use]
    pub const fn transition(from: Color, to: Color, duration_ms: u32) -> Self {
        Self::Transition {
            from,
            to,
            duration_ms,
            progress: 0.0,
            easing: EasingFn::EaseOut,
        }
    }

    /// Resolve to a concrete Color at current phase
    #[must_use]
    pub fn resolve(&self) -> Color {
        match self {
            Self::Static(c) => *c,
            Self::Pulse {
                from, to, phase, ..
            } => {
                // Sinusoidal interpolation for smooth pulsing
                let t = EasingFn::Sine.apply(*phase);
                lerp_color(*from, *to, t)
            }
            Self::Shimmer {
                base,
                intensity,
                phase,
                ..
            } => {
                // Subtle brightness variation using sine wave
                let t = EasingFn::Sine.apply(*phase);
                adjust_brightness(*base, t * intensity)
            }
            Self::Transition {
                from,
                to,
                progress,
                easing,
                ..
            } => {
                let t = easing.apply(*progress);
                lerp_color(*from, *to, t)
            }
        }
    }

    /// Returns true if this color is animated (needs ticking)
    #[must_use]
    pub const fn is_animated(&self) -> bool {
        !matches!(self, Self::Static(_))
    }

    /// Returns true if this is a finite transition that has completed
    #[must_use]
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Transition { progress, .. } => *progress >= 1.0,
            Self::Static(_) | Self::Pulse { .. } | Self::Shimmer { .. } => false,
        }
    }

    /// Advance the animation by the given delta time in milliseconds
    #[allow(clippy::cast_precision_loss)]
    pub fn tick(&mut self, delta_ms: f32) {
        match self {
            Self::Static(_) => {}
            Self::Pulse {
                period_ms, phase, ..
            }
            | Self::Shimmer {
                period_ms, phase, ..
            } => {
                let period = *period_ms as f32;
                *phase = (*phase + delta_ms / period) % 1.0;
            }
            Self::Transition {
                duration_ms,
                progress,
                ..
            } => {
                let duration = *duration_ms as f32;
                *progress = (*progress + delta_ms / duration).min(1.0);
            }
        }
    }
}

/// Linear interpolation between two colors
#[must_use]
pub fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);

    // Extract RGB components from both colors
    let (r1, g1, b1) = color_to_rgb(from);
    let (r2, g2, b2) = color_to_rgb(to);

    // Interpolate each component
    let r = lerp_u8(r1, r2, t);
    let g = lerp_u8(g1, g2, t);
    let b = lerp_u8(b1, b2, t);

    Color::Rgb { r, g, b }
}

/// Adjust brightness of a color
///
/// # Arguments
/// * `color` - Base color
/// * `delta` - Brightness adjustment (-1.0 to 1.0, positive = brighter)
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn adjust_brightness(color: Color, delta: f32) -> Color {
    let (r, g, b) = color_to_rgb(color);

    // Convert to HSL-like brightness adjustment
    let adjust = |c: u8| -> u8 {
        let f = f32::from(c) / 255.0;
        let adjusted = if delta > 0.0 {
            (1.0 - f).mul_add(delta, f) // Lighten
        } else {
            f * (1.0 + delta) // Darken
        };
        (adjusted.clamp(0.0, 1.0) * 255.0) as u8
    };

    Color::Rgb {
        r: adjust(r),
        g: adjust(g),
        b: adjust(b),
    }
}

/// Convert any Color to RGB components
#[allow(clippy::match_same_arms)]
const fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb { r, g, b } => (r, g, b),
        Color::AnsiValue(n) => ansi256_to_rgb(n),
        Color::Black => (0, 0, 0),
        Color::DarkGrey => (128, 128, 128),
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
        Color::Reset => (128, 128, 128), // Default gray
    }
}

/// Convert ANSI 256 color to RGB
const fn ansi256_to_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0 => (0, 0, 0),        // Black
        1 => (128, 0, 0),      // Maroon
        2 => (0, 128, 0),      // Green
        3 => (128, 128, 0),    // Olive
        4 => (0, 0, 128),      // Navy
        5 => (128, 0, 128),    // Purple
        6 => (0, 128, 128),    // Teal
        7 => (192, 192, 192),  // Silver
        8 => (128, 128, 128),  // Gray
        9 => (255, 0, 0),      // Red
        10 => (0, 255, 0),     // Lime
        11 => (255, 255, 0),   // Yellow
        12 => (0, 0, 255),     // Blue
        13 => (255, 0, 255),   // Fuchsia
        14 => (0, 255, 255),   // Aqua
        15 => (255, 255, 255), // White
        16..=231 => {
            // 216 color cube (6x6x6)
            let idx = n - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            // Convert 6-level component to 8-bit RGB (inline to satisfy const fn)
            let r_val = if r == 0 { 0 } else { 55 + r * 40 };
            let g_val = if g == 0 { 0 } else { 55 + g * 40 };
            let b_val = if b == 0 { 0 } else { 55 + b * 40 };
            (r_val, g_val, b_val)
        }
        232..=255 => {
            // 24 grayscale colors
            let gray = 8 + (n - 232) * 10;
            (gray, gray, gray)
        }
    }
}

/// Linear interpolation for u8 values
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a = f32::from(a);
    let b = f32::from(b);
    (b - a).mul_add(t, a).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_color() {
        let color = AnimatedColor::new(Color::Red);
        assert!(!color.is_animated());
        // Static colors are returned as-is (no conversion)
        assert_eq!(color.resolve(), Color::Red);
    }

    #[test]
    fn test_lerp_color() {
        let black = Color::Rgb { r: 0, g: 0, b: 0 };
        let white = Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        };

        let mid = lerp_color(black, white, 0.5);
        if let Color::Rgb { r, g, b } = mid {
            assert!((i16::from(r) - 128).abs() <= 1);
            assert!((i16::from(g) - 128).abs() <= 1);
            assert!((i16::from(b) - 128).abs() <= 1);
        } else {
            panic!("Expected RGB color");
        }
    }

    #[test]
    fn test_pulse_tick() {
        let mut pulse = AnimatedColor::pulse(Color::Black, Color::White, 1000);
        assert!(pulse.is_animated());

        // Tick halfway through
        pulse.tick(500.0);
        if let AnimatedColor::Pulse { phase, .. } = pulse {
            assert!((phase - 0.5).abs() < 0.01);
        }
    }

    #[test]
    fn test_transition_complete() {
        let mut trans = AnimatedColor::transition(Color::Black, Color::White, 100);
        assert!(!trans.is_complete());

        trans.tick(100.0);
        assert!(trans.is_complete());
    }

    #[test]
    fn test_adjust_brightness() {
        let gray = Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        };

        // Brighten
        let brighter = adjust_brightness(gray, 0.5);
        if let Color::Rgb { r, g, b } = brighter {
            assert!(r > 128);
            assert!(g > 128);
            assert!(b > 128);
        }

        // Darken
        let darker = adjust_brightness(gray, -0.5);
        if let Color::Rgb { r, g, b } = darker {
            assert!(r < 128);
            assert!(g < 128);
            assert!(b < 128);
        }
    }
}
