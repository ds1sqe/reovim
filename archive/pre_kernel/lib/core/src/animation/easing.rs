//! Easing functions for smooth animations
//!
//! Easing functions control how animations progress over time.
//! They transform a linear progress value (0.0 to 1.0) into a curved value.

/// Easing function types for animation curves
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EasingFn {
    /// Linear interpolation (no easing)
    #[default]
    Linear,
    /// Slow start, fast end
    EaseIn,
    /// Fast start, slow end
    EaseOut,
    /// Slow start and end
    EaseInOut,
    /// Sinusoidal oscillation (for pulse/breathing effects)
    Sine,
}

impl EasingFn {
    /// Apply the easing function to a progress value
    ///
    /// # Arguments
    /// * `t` - Progress value from 0.0 to 1.0
    ///
    /// # Returns
    /// Eased value, typically 0.0 to 1.0 (can exceed for bounce effects)
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => (1.0 - t).mul_add(-(1.0 - t), 1.0),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0f32).mul_add(t, 2.0).powi(2) / 2.0
                }
            }
            Self::Sine => {
                // Full sine wave: 0 -> 1 -> 0 over t = 0 -> 1
                (t * std::f32::consts::PI).sin()
            }
        }
    }

    /// Apply bidirectional easing (for oscillating effects)
    ///
    /// Returns a value that oscillates between 0 and 1:
    /// - t=0.0 → 0.0
    /// - t=0.5 → 1.0
    /// - t=1.0 → 0.0
    #[must_use]
    pub fn apply_bidirectional(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Sine => (t * std::f32::consts::PI).sin(),
            _ => {
                // Convert linear-ish easing to bidirectional
                if t < 0.5 {
                    self.apply(t * 2.0)
                } else {
                    self.apply(2.0 - t * 2.0)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert!((EasingFn::Linear.apply(0.0) - 0.0).abs() < 0.001);
        assert!((EasingFn::Linear.apply(0.5) - 0.5).abs() < 0.001);
        assert!((EasingFn::Linear.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_ease_in() {
        assert!((EasingFn::EaseIn.apply(0.0) - 0.0).abs() < 0.001);
        assert!(EasingFn::EaseIn.apply(0.5) < 0.5); // Slow start
        assert!((EasingFn::EaseIn.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_ease_out() {
        assert!((EasingFn::EaseOut.apply(0.0) - 0.0).abs() < 0.001);
        assert!(EasingFn::EaseOut.apply(0.5) > 0.5); // Fast start
        assert!((EasingFn::EaseOut.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_sine_bidirectional() {
        assert!((EasingFn::Sine.apply(0.0) - 0.0).abs() < 0.001);
        assert!((EasingFn::Sine.apply(0.5) - 1.0).abs() < 0.001); // Peak at middle
        assert!((EasingFn::Sine.apply(1.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_clamp() {
        // Values outside 0-1 should be clamped
        assert!((EasingFn::Linear.apply(-0.5) - 0.0).abs() < 0.001);
        assert!((EasingFn::Linear.apply(1.5) - 1.0).abs() < 0.001);
    }
}
