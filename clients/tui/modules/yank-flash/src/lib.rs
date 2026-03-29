#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Yank flash highlight module (#657).
//!
//! Renders a brief background highlight on yanked text ranges.
//! Receives yank events from the server's `YankFlashBridge` via
//! `on_notification()` and renders via `inline_decorations()`.
//!
//! The flash auto-dismisses after `FLASH_DURATION` (200ms) via `tick()`.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use {
    reovim_arch::{
        Color,
        clock::{Clock, SystemClock},
    },
    reovim_client_driver::{
        ClientModule, ClientModuleError, InlineDecoration, ModuleContext, ProbeResult, Style,
        Version,
    },
};

/// Module kind identifier — matches the server bridge kind.
const KIND: &str = "yank-flash";

/// Flash highlight duration.
const FLASH_DURATION: Duration = Duration::from_millis(200);

/// Subtle blue-purple highlight background for the flash.
const FLASH_COLOR: Color = Color::Rgb {
    r: 80,
    g: 80,
    b: 120,
};

/// State for an active flash animation.
struct FlashState {
    /// Start line of the flash range (0-indexed).
    start_line: usize,
    /// End line of the flash range (0-indexed, inclusive).
    end_line: usize,
    /// Start column (for characterwise).
    start_col: usize,
    /// End column (for characterwise).
    end_col: usize,
    /// Whether the flash is linewise.
    is_linewise: bool,
    /// When the flash started.
    started_at: Instant,
}

/// Yank flash highlight module.
///
/// Listens for yank events and renders a timed background highlight
/// on the affected range using `inline_decorations()`.
pub struct YankFlashModule {
    /// Currently active flash, if any.
    active_flash: Option<FlashState>,
    /// Clock for timing.
    clock: Arc<dyn Clock>,
    /// Last seen sequence number (to detect new yanks).
    last_sequence: u64,
    /// Cached inline decorations per line.
    decorations_by_line: HashMap<usize, Vec<InlineDecoration>>,
}

impl YankFlashModule {
    /// Create a new module with system clock.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_flash: None,
            clock: Arc::new(SystemClock),
            last_sequence: 0,
            decorations_by_line: HashMap::new(),
        }
    }

    /// Create with custom clock (for deterministic testing).
    #[must_use]
    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        Self {
            active_flash: None,
            clock,
            last_sequence: 0,
            decorations_by_line: HashMap::new(),
        }
    }

    /// Build cached decorations from the current flash state.
    fn rebuild_decorations(&mut self) {
        self.decorations_by_line.clear();

        let Some(flash) = &self.active_flash else {
            return;
        };

        let style = Style {
            bg: Some(FLASH_COLOR),
            ..Style::default()
        };

        for line in flash.start_line..=flash.end_line {
            let decoration = if flash.is_linewise {
                // Linewise: full-width decoration
                InlineDecoration {
                    col_start: 0,
                    col_end: u16::MAX,
                    style: style.clone(),
                }
            } else if flash.start_line == flash.end_line {
                // Single-line characterwise
                #[allow(clippy::cast_possible_truncation)]
                InlineDecoration {
                    col_start: flash.start_col as u16,
                    col_end: flash.end_col as u16,
                    style: style.clone(),
                }
            } else if line == flash.start_line {
                // First line of multi-line characterwise
                #[allow(clippy::cast_possible_truncation)]
                InlineDecoration {
                    col_start: flash.start_col as u16,
                    col_end: u16::MAX,
                    style: style.clone(),
                }
            } else if line == flash.end_line {
                // Last line of multi-line characterwise
                #[allow(clippy::cast_possible_truncation)]
                InlineDecoration {
                    col_start: 0,
                    col_end: flash.end_col as u16,
                    style: style.clone(),
                }
            } else {
                // Middle lines of multi-line characterwise
                InlineDecoration {
                    col_start: 0,
                    col_end: u16::MAX,
                    style: style.clone(),
                }
            };

            self.decorations_by_line
                .entry(line)
                .or_default()
                .push(decoration);
        }
    }
}

impl Default for YankFlashModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for YankFlashModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Yank Flash"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        vec![KIND]
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_buffer_contrib(&self) -> bool {
        self.active_flash.is_some()
    }

    fn on_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        let Some(sequence) = json.get("sequence").and_then(serde_json::Value::as_u64) else {
            return;
        };

        // Ignore if same sequence (duplicate notification)
        if sequence <= self.last_sequence {
            return;
        }
        self.last_sequence = sequence;

        #[allow(clippy::cast_possible_truncation)]
        let start_line = json
            .get("startLine")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        #[allow(clippy::cast_possible_truncation)]
        let end_line = json
            .get("endLine")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        #[allow(clippy::cast_possible_truncation)]
        let start_col = json
            .get("startCol")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        #[allow(clippy::cast_possible_truncation)]
        let end_col = json
            .get("endCol")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let is_linewise = json
            .get("isLinewise")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        self.active_flash = Some(FlashState {
            start_line,
            end_line,
            start_col,
            end_col,
            is_linewise,
            started_at: self.clock.now(),
        });

        self.rebuild_decorations();
    }

    fn tick(&mut self) -> bool {
        let Some(flash) = &self.active_flash else {
            return false;
        };

        let elapsed = self.clock.now().duration_since(flash.started_at);
        if elapsed >= FLASH_DURATION {
            self.active_flash = None;
            self.decorations_by_line.clear();
            return true;
        }

        false
    }

    fn inline_decorations(&self, line: usize) -> &[InlineDecoration] {
        self.decorations_by_line
            .get(&line)
            .map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
