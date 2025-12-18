//! Trigger detection for autocompletion

use std::time::{Duration, Instant};

/// Configuration for completion triggering
#[derive(Debug, Clone)]
pub struct TriggerConfig {
    /// Minimum characters before auto-triggering
    pub min_chars: usize,
    /// Delay after typing before triggering (milliseconds)
    pub debounce_ms: u64,
    /// Characters that trigger completion immediately
    pub trigger_chars: Vec<char>,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            min_chars: 1,
            debounce_ms: 50, // Fast for "fastest-reaction" goal
            trigger_chars: vec!['.', ':'],
        }
    }
}

impl TriggerConfig {
    /// Create a new trigger config
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min_chars: 1,
            debounce_ms: 50,
            trigger_chars: Vec::new(),
        }
    }

    /// Set minimum characters
    #[must_use]
    pub const fn with_min_chars(mut self, min: usize) -> Self {
        self.min_chars = min;
        self
    }

    /// Set debounce duration
    #[must_use]
    pub const fn with_debounce_ms(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Set trigger characters
    #[must_use]
    pub fn with_trigger_chars(mut self, chars: Vec<char>) -> Self {
        self.trigger_chars = chars;
        self
    }
}

/// Result of trigger detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerResult {
    /// No trigger condition met
    NoTrigger,
    /// Trigger immediately (e.g., trigger character typed)
    TriggerNow,
    /// Wait for debounce period
    TriggerAfterDebounce,
}

/// Manages trigger detection with debouncing
pub struct TriggerDetector {
    config: TriggerConfig,
    last_input_time: Option<Instant>,
    pending_trigger: bool,
}

impl TriggerDetector {
    /// Create a new trigger detector
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Option::None is const but not recognized
    pub fn new(config: TriggerConfig) -> Self {
        Self {
            config,
            last_input_time: None,
            pending_trigger: false,
        }
    }

    /// Called when a character is typed in insert mode
    ///
    /// Returns the trigger result indicating whether completion should be triggered
    pub fn on_char_typed(&mut self, ch: char, prefix_len: usize) -> TriggerResult {
        self.last_input_time = Some(Instant::now());

        // Immediate trigger on trigger characters
        if self.config.trigger_chars.contains(&ch) {
            self.pending_trigger = false;
            return TriggerResult::TriggerNow;
        }

        // Check minimum character threshold
        if prefix_len >= self.config.min_chars {
            self.pending_trigger = true;
            return TriggerResult::TriggerAfterDebounce;
        }

        TriggerResult::NoTrigger
    }

    /// Check if debounce period has elapsed
    #[must_use]
    pub fn should_trigger_now(&self) -> bool {
        if !self.pending_trigger {
            return false;
        }

        self.last_input_time.is_some_and(|last_time| {
            last_time.elapsed() >= Duration::from_millis(self.config.debounce_ms)
        })
    }

    /// Get the debounce duration
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Duration::from_millis not const-stable
    pub fn debounce_duration(&self) -> Duration {
        Duration::from_millis(self.config.debounce_ms)
    }

    /// Clear pending trigger (called after triggering or on dismiss)
    pub const fn clear(&mut self) {
        self.pending_trigger = false;
    }

    /// Check if a trigger is pending
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.pending_trigger
    }

    /// Get the config
    #[must_use]
    pub const fn config(&self) -> &TriggerConfig {
        &self.config
    }
}

impl Default for TriggerDetector {
    fn default() -> Self {
        Self::new(TriggerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::thread};

    #[test]
    fn test_trigger_on_trigger_char() {
        let mut detector = TriggerDetector::default();

        let result = detector.on_char_typed('.', 0);
        assert_eq!(result, TriggerResult::TriggerNow);
    }

    #[test]
    fn test_trigger_after_min_chars() {
        let config = TriggerConfig::default().with_min_chars(2);
        let mut detector = TriggerDetector::new(config);

        // 1 char - not enough
        let result = detector.on_char_typed('a', 1);
        assert_eq!(result, TriggerResult::NoTrigger);

        // 2 chars - triggers after debounce
        let result = detector.on_char_typed('b', 2);
        assert_eq!(result, TriggerResult::TriggerAfterDebounce);
    }

    #[test]
    fn test_debounce_timing() {
        let config = TriggerConfig::default().with_debounce_ms(10);
        let mut detector = TriggerDetector::new(config);

        let result = detector.on_char_typed('a', 1);
        assert_eq!(result, TriggerResult::TriggerAfterDebounce);

        // Should not trigger immediately
        assert!(!detector.should_trigger_now());

        // Wait for debounce
        thread::sleep(Duration::from_millis(15));

        // Now should trigger
        assert!(detector.should_trigger_now());
    }

    #[test]
    fn test_clear_pending() {
        let mut detector = TriggerDetector::default();

        detector.on_char_typed('a', 1);
        assert!(detector.is_pending());

        detector.clear();
        assert!(!detector.is_pending());
        assert!(!detector.should_trigger_now());
    }
}
