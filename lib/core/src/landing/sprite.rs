//! ASCII sprite animation controller

/// Animation playback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationMode {
    /// Play once then hold last frame
    Once,
    /// Loop forever (0->n->0->n...)
    #[default]
    Loop,
    /// Play forward then backward (0->n->0)
    PingPong,
}

/// Direction for ping-pong animation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Backward,
}

/// Animated ASCII sprite with frame cycling
#[derive(Debug, Clone)]
pub struct AsciiSprite {
    /// Array of frames (each frame is array of lines)
    frames: &'static [&'static [&'static str]],
    /// Milliseconds per frame
    frame_duration_ms: u32,
    /// Pause after complete cycle (milliseconds)
    pause_duration_ms: u32,
    /// Current frame index
    current_frame: usize,
    /// Accumulated time since last frame change (in ms, stored as u32)
    accumulated_ms: u32,
    /// Animation mode
    mode: AnimationMode,
    /// Direction for ping-pong
    direction: Direction,
    /// Whether currently in pause state
    paused: bool,
}

impl AsciiSprite {
    /// Create a new sprite with the given frames and timing
    #[must_use]
    pub const fn new(
        frames: &'static [&'static [&'static str]],
        frame_duration_ms: u32,
        pause_duration_ms: u32,
        mode: AnimationMode,
    ) -> Self {
        Self {
            frames,
            frame_duration_ms,
            pause_duration_ms,
            current_frame: 0,
            accumulated_ms: 0,
            mode,
            direction: Direction::Forward,
            paused: false,
        }
    }

    /// Advance animation by `delta_ms`, returns true if frame changed
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn tick(&mut self, delta_ms: f32) -> bool {
        if self.frames.is_empty() {
            return false;
        }

        // Convert delta to u32 (truncate fractional part)
        self.accumulated_ms = self.accumulated_ms.saturating_add(delta_ms as u32);

        // Check if in pause state
        if self.paused {
            if self.accumulated_ms >= self.pause_duration_ms {
                self.accumulated_ms = 0;
                self.paused = false;
            }
            return false;
        }

        // Check if time to advance frame
        if self.accumulated_ms < self.frame_duration_ms {
            return false;
        }

        self.accumulated_ms = 0;
        let old_frame = self.current_frame;
        let frame_count = self.frames.len();

        match self.mode {
            AnimationMode::Once => {
                if self.current_frame < frame_count - 1 {
                    self.current_frame += 1;
                }
            }
            AnimationMode::Loop => {
                self.current_frame = (self.current_frame + 1) % frame_count;
                // Enter pause state when cycle completes
                if self.current_frame == 0 && self.pause_duration_ms > 0 {
                    self.paused = true;
                }
            }
            AnimationMode::PingPong => {
                match self.direction {
                    Direction::Forward => {
                        if self.current_frame + 1 >= frame_count {
                            // Reached end, reverse direction
                            self.direction = Direction::Backward;
                            self.current_frame = frame_count.saturating_sub(2);
                        } else {
                            self.current_frame += 1;
                        }
                    }
                    Direction::Backward => {
                        if self.current_frame == 0 {
                            // Reached start, reverse direction
                            self.direction = Direction::Forward;
                            if self.pause_duration_ms > 0 {
                                self.paused = true;
                            } else {
                                self.current_frame = 1.min(frame_count - 1);
                            }
                        } else {
                            self.current_frame -= 1;
                        }
                    }
                }
            }
        }

        self.current_frame != old_frame
    }

    /// Get current frame content
    #[must_use]
    pub fn current_frame(&self) -> &'static [&'static str] {
        self.frames
            .get(self.current_frame)
            .copied()
            .unwrap_or(&[])
    }

    /// Get current frame index
    #[must_use]
    pub const fn frame_index(&self) -> usize {
        self.current_frame
    }

    /// Get total frame count
    #[must_use]
    pub const fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Reset animation to first frame
    pub const fn reset(&mut self) {
        self.current_frame = 0;
        self.accumulated_ms = 0;
        self.direction = Direction::Forward;
        self.paused = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_FRAMES: &[&[&str]] = &[&["frame0"], &["frame1"], &["frame2"]];

    #[test]
    fn test_loop_mode() {
        let mut sprite = AsciiSprite::new(TEST_FRAMES, 100, 0, AnimationMode::Loop);

        assert_eq!(sprite.frame_index(), 0);

        // Advance past frame duration
        assert!(sprite.tick(150.0));
        assert_eq!(sprite.frame_index(), 1);

        assert!(sprite.tick(100.0));
        assert_eq!(sprite.frame_index(), 2);

        // Should wrap to 0
        assert!(sprite.tick(100.0));
        assert_eq!(sprite.frame_index(), 0);
    }

    #[test]
    fn test_ping_pong_mode() {
        let mut sprite = AsciiSprite::new(TEST_FRAMES, 100, 0, AnimationMode::PingPong);

        assert_eq!(sprite.frame_index(), 0);

        sprite.tick(100.0);
        assert_eq!(sprite.frame_index(), 1);

        sprite.tick(100.0);
        assert_eq!(sprite.frame_index(), 2);

        // Should reverse direction
        sprite.tick(100.0);
        assert_eq!(sprite.frame_index(), 1);

        sprite.tick(100.0);
        assert_eq!(sprite.frame_index(), 0);
    }

    #[test]
    fn test_once_mode() {
        let mut sprite = AsciiSprite::new(TEST_FRAMES, 100, 0, AnimationMode::Once);

        sprite.tick(100.0);
        sprite.tick(100.0);
        assert_eq!(sprite.frame_index(), 2);

        // Should stay at last frame
        sprite.tick(100.0);
        assert_eq!(sprite.frame_index(), 2);
    }

    #[test]
    fn test_pause_after_cycle() {
        let mut sprite = AsciiSprite::new(TEST_FRAMES, 100, 200, AnimationMode::Loop);

        // Go through all frames
        sprite.tick(100.0); // -> 1
        sprite.tick(100.0); // -> 2
        sprite.tick(100.0); // -> 0, enter pause

        assert_eq!(sprite.frame_index(), 0);

        // During pause, frame shouldn't change
        assert!(!sprite.tick(100.0));
        assert_eq!(sprite.frame_index(), 0);

        // After pause duration, animation resumes
        assert!(!sprite.tick(100.0)); // Still in pause (200ms total needed)
        assert!(sprite.tick(100.0)); // Pause done, advances
        assert_eq!(sprite.frame_index(), 1);
    }
}
