//! Effect types for the animation system
//!
//! Effects define what visual changes should occur and where they should be applied.

use std::time::{Duration, Instant};

use {
    crate::highlight::{Attributes, HighlightGroup, Style},
    reovim_sys::style::Color,
};

use super::color::AnimatedColor;

/// Configuration for a sweep/glowing reflection effect
///
/// A sweep effect creates a glowing highlight that moves across a UI element,
/// like a reflection moving across a status bar.
#[derive(Debug, Clone, Copy)]
pub struct SweepConfig {
    /// Current position of the sweep center (0.0 = left, 1.0 = right)
    pub phase: f32,
    /// Period for one full sweep cycle (left→right→left) in milliseconds
    pub period_ms: u32,
    /// Width of the glow as a fraction of total width (0.0 - 1.0)
    pub width: f32,
    /// Intensity of the glow (brightness boost, 0.0 - 1.0)
    pub intensity: f32,
    /// Direction: 1.0 = moving right, -1.0 = moving left
    direction: f32,
}

impl SweepConfig {
    /// Create a new sweep configuration
    #[must_use]
    pub const fn new(period_ms: u32, width: f32, intensity: f32) -> Self {
        Self {
            phase: 0.0,
            period_ms,
            width,
            intensity,
            direction: 1.0, // Start moving right
        }
    }

    /// Advance the sweep by delta time
    #[allow(clippy::cast_precision_loss)]
    pub fn tick(&mut self, delta_ms: f32) {
        // Calculate phase delta (half period for one direction)
        let half_period = self.period_ms as f32 / 2.0;
        let phase_delta = delta_ms / half_period;

        self.phase += phase_delta * self.direction;

        // Bounce at edges (ping-pong)
        if self.phase >= 1.0 {
            self.phase = 1.0;
            self.direction = -1.0;
        } else if self.phase <= 0.0 {
            self.phase = 0.0;
            self.direction = 1.0;
        }
    }

    /// Calculate brightness boost for a given position
    ///
    /// Returns a value 0.0-1.0 representing how much to brighten the cell
    /// based on distance from the sweep center.
    #[must_use]
    pub fn brightness_at(&self, position: f32) -> f32 {
        let distance = (position - self.phase).abs();
        let half_width = self.width / 2.0;

        if distance > half_width {
            0.0
        } else {
            // Gaussian-like falloff for smooth glow
            let normalized = distance / half_width;
            let falloff = 1.0 - normalized * normalized;
            falloff * self.intensity
        }
    }
}

/// Unique identifier for an effect
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectId(u64);

impl EffectId {
    /// Create a new effect ID
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the inner ID value
    #[must_use]
    pub const fn inner(self) -> u64 {
        self.0
    }
}

/// Identifies what element(s) an effect applies to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EffectTarget {
    /// Apply to a highlight group
    HighlightGroup(HighlightGroup),

    /// Apply to a UI element
    UiElement(UiElementId),

    /// Apply to a specific cell region (buffer-relative)
    CellRegion {
        buffer_id: usize,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    },
}

/// Identifiers for UI elements that can have effects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiElementId {
    /// Cursor position
    Cursor,
    /// Status line
    StatusLine,
    /// Command line
    CommandLine,
    /// Tab line
    TabLine,
    /// Line numbers gutter
    LineNumbers,
    /// Custom named element
    Custom(u32),
}

/// Style with optional animation on fg/bg colors
#[derive(Debug, Clone)]
pub struct AnimatedStyle {
    /// Foreground color (optional, may be animated)
    pub fg: Option<AnimatedColor>,
    /// Background color (optional, may be animated)
    pub bg: Option<AnimatedColor>,
    /// Text attributes (not animated)
    pub attributes: Attributes,
}

impl AnimatedStyle {
    /// Create a new empty animated style
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: Attributes::new(),
        }
    }

    /// Create a pulsing background style
    #[must_use]
    pub const fn pulse_bg(from: Color, to: Color, period_ms: u32) -> Self {
        Self {
            fg: None,
            bg: Some(AnimatedColor::pulse(from, to, period_ms)),
            attributes: Attributes::new(),
        }
    }

    /// Create a pulsing foreground style
    #[must_use]
    pub const fn pulse_fg(from: Color, to: Color, period_ms: u32) -> Self {
        Self {
            fg: Some(AnimatedColor::pulse(from, to, period_ms)),
            bg: None,
            attributes: Attributes::new(),
        }
    }

    /// Create a shimmer background style
    #[must_use]
    pub const fn shimmer_bg(base: Color, intensity: f32, period_ms: u32) -> Self {
        Self {
            fg: None,
            bg: Some(AnimatedColor::shimmer(base, intensity, period_ms)),
            attributes: Attributes::new(),
        }
    }

    /// Create a transition background style
    #[must_use]
    pub const fn transition_bg(from: Color, to: Color, duration_ms: u32) -> Self {
        Self {
            fg: None,
            bg: Some(AnimatedColor::transition(from, to, duration_ms)),
            attributes: Attributes::new(),
        }
    }

    /// Resolve to a concrete Style at current phase
    #[must_use]
    pub fn resolve(&self) -> Style {
        Style {
            fg: self.fg.as_ref().map(AnimatedColor::resolve),
            bg: self.bg.as_ref().map(AnimatedColor::resolve),
            attributes: self.attributes,
            underline_color: None,
        }
    }

    /// Check if any component is animated
    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.fg.as_ref().is_some_and(AnimatedColor::is_animated)
            || self.bg.as_ref().is_some_and(AnimatedColor::is_animated)
    }

    /// Check if this is a finite animation that has completed
    #[must_use]
    pub fn is_complete(&self) -> bool {
        let fg_complete = self.fg.as_ref().is_none_or(AnimatedColor::is_complete);
        let bg_complete = self.bg.as_ref().is_none_or(AnimatedColor::is_complete);
        fg_complete && bg_complete
    }

    /// Advance the animation by the given delta time
    pub fn tick(&mut self, delta_ms: f32) {
        if let Some(ref mut fg) = self.fg {
            fg.tick(delta_ms);
        }
        if let Some(ref mut bg) = self.bg {
            bg.tick(delta_ms);
        }
    }
}

impl Default for AnimatedStyle {
    fn default() -> Self {
        Self::new()
    }
}

/// An active animation effect
#[derive(Debug, Clone)]
pub struct Effect {
    /// Unique identifier
    pub id: EffectId,
    /// What this effect applies to
    pub target: EffectTarget,
    /// The animated style
    pub style: AnimatedStyle,
    /// When the effect started
    pub started_at: Instant,
    /// Optional duration (None = infinite loop)
    pub duration: Option<Duration>,
    /// Priority for effect composition (higher wins)
    pub priority: u8,
    /// Optional sweep configuration for position-based glow effects
    pub sweep: Option<SweepConfig>,
}

impl Effect {
    /// Create a new effect
    #[must_use]
    pub fn new(id: EffectId, target: EffectTarget, style: AnimatedStyle) -> Self {
        Self {
            id,
            target,
            style,
            started_at: Instant::now(),
            duration: None,
            priority: 0,
            sweep: None,
        }
    }

    /// Set effect duration
    #[must_use]
    pub const fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Set effect priority
    #[must_use]
    pub const fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Add sweep configuration for position-based glow
    #[must_use]
    pub const fn with_sweep(mut self, sweep: SweepConfig) -> Self {
        self.sweep = Some(sweep);
        self
    }

    /// Check if the effect has expired
    ///
    /// Effects expire when:
    /// - They have a duration and that duration has elapsed
    /// - OR they have no duration and the style animation is complete
    ///   (but sweep effects never expire based on style alone)
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let result = self.duration.map_or_else(
            || {
                // Effects with a sweep should NOT expire based on style completion
                // They are meant to run indefinitely until explicitly stopped
                if self.sweep.is_some() {
                    tracing::debug!(
                        "==> Effect {:?} is_expired: has sweep, never expires",
                        self.id
                    );
                    false
                } else {
                    let complete = self.style.is_complete();
                    tracing::debug!(
                        "==> Effect {:?} is_expired: no duration, style.is_complete={}",
                        self.id,
                        complete
                    );
                    complete
                }
            },
            |duration| {
                let elapsed = self.started_at.elapsed();
                let expired = elapsed >= duration;
                tracing::debug!(
                    "==> Effect {:?} is_expired: elapsed={:?}, duration={:?}, expired={}",
                    self.id,
                    elapsed,
                    duration,
                    expired
                );
                expired
            },
        );
        tracing::info!("==> Effect {:?} is_expired() = {}", self.id, result);
        result
    }

    /// Advance the effect by the given delta time
    pub fn tick(&mut self, delta_ms: f32) {
        self.style.tick(delta_ms);
        if let Some(ref mut sweep) = self.sweep {
            sweep.tick(delta_ms);
        }
    }

    /// Resolve the effect's style at current time
    #[must_use]
    pub fn resolve_style(&self) -> Style {
        self.style.resolve()
    }

    /// Check if this effect has an active sweep
    #[must_use]
    pub const fn has_sweep(&self) -> bool {
        self.sweep.is_some()
    }

    /// Get the sweep configuration if present
    #[must_use]
    pub const fn sweep_config(&self) -> Option<&SweepConfig> {
        self.sweep.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_id() {
        let id = EffectId::new(42);
        assert_eq!(id.inner(), 42);
    }

    #[test]
    fn test_animated_style_pulse() {
        let style = AnimatedStyle::pulse_bg(Color::Black, Color::White, 1000);
        assert!(style.is_animated());
        assert!(!style.is_complete());
    }

    #[test]
    fn test_effect_expiration() {
        let effect = Effect::new(
            EffectId::new(1),
            EffectTarget::UiElement(UiElementId::Cursor),
            AnimatedStyle::new(),
        )
        .with_duration(Duration::from_millis(1));

        // Effect should expire after waiting
        std::thread::sleep(Duration::from_millis(5));
        assert!(effect.is_expired());
    }

    #[test]
    fn test_effect_priority() {
        let effect = Effect::new(
            EffectId::new(1),
            EffectTarget::UiElement(UiElementId::StatusLine),
            AnimatedStyle::new(),
        )
        .with_priority(100);

        assert_eq!(effect.priority, 100);
    }
}
