#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Illuminate (word reference highlight) TUI client module (#664).
//!
//! Renders background highlights on all occurrences of the word under cursor.
//! Receives highlight ranges from the server's `IlluminateBridge` via
//! `on_notification()` and renders via `inline_decorations()`.

use std::collections::HashMap;

use {
    reovim_arch::Color,
    reovim_client_driver::{
        ClientModule, ClientModuleError, InlineDecoration, ModuleContext, ProbeResult, Style,
        Version,
    },
};

/// Module kind identifier — matches the server bridge kind.
const KIND: &str = "illuminate";

/// Subtle underline-style highlight for word references.
const HIGHLIGHT_COLOR: Color = Color::Rgb {
    r: 60,
    g: 60,
    b: 90,
};

/// TUI illuminate module.
///
/// Parses word highlight ranges from the server bridge and renders them
/// as background highlights via `inline_decorations()`.
pub struct IlluminateModule {
    /// Whether highlights are currently active.
    active: bool,
    /// Last seen sequence number for deduplication.
    last_sequence: u64,
    /// Cached inline decorations per line.
    decorations_by_line: HashMap<usize, Vec<InlineDecoration>>,
}

impl IlluminateModule {
    /// Create a new illuminate module.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            last_sequence: 0,
            decorations_by_line: HashMap::new(),
        }
    }

    /// Rebuild decorations from parsed ranges.
    fn rebuild_decorations(&mut self, ranges: &[RangeInfo]) {
        self.decorations_by_line.clear();

        let style = Style {
            bg: Some(HIGHLIGHT_COLOR),
            ..Style::default()
        };

        for range in ranges {
            #[allow(clippy::cast_possible_truncation)]
            let decoration = InlineDecoration {
                col_start: range.start_col as u16,
                col_end: range.end_col as u16,
                style: style.clone(),
            };

            self.decorations_by_line
                .entry(range.start_line)
                .or_default()
                .push(decoration);
        }
    }
}

impl Default for IlluminateModule {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed highlight range from server JSON.
struct RangeInfo {
    start_line: usize,
    start_col: usize,
    end_col: usize,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for IlluminateModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Illuminate"
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
        self.active
    }

    fn on_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        let Some(sequence) = json.get("sequence").and_then(serde_json::Value::as_u64) else {
            return;
        };

        // Ignore duplicate notifications
        if sequence <= self.last_sequence {
            return;
        }
        self.last_sequence = sequence;

        let active = json
            .get("active")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if !active {
            self.active = false;
            self.decorations_by_line.clear();
            return;
        }

        // Parse ranges array
        let Some(ranges_arr) = json.get("ranges").and_then(serde_json::Value::as_array) else {
            self.active = false;
            self.decorations_by_line.clear();
            return;
        };

        #[allow(clippy::cast_possible_truncation)]
        let ranges: Vec<RangeInfo> = ranges_arr
            .iter()
            .filter_map(|r| {
                Some(RangeInfo {
                    start_line: r.get("startLine")?.as_u64()? as usize,
                    start_col: r.get("startCol")?.as_u64()? as usize,
                    end_col: r.get("endCol")?.as_u64()? as usize,
                })
            })
            .collect();

        self.active = !ranges.is_empty();
        self.rebuild_decorations(&ranges);
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
