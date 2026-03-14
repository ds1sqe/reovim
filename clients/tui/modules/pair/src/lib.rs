//! Bracket pair highlighting module.
//!
//! Receives bracket state from the server's pair bridge and renders:
//! - Rainbow bracket coloring (6-color depth cycling)
//! - Matched-pair highlighting (bold + underline)
//! - Unmatched bracket warning (red + underline)
//!
//! Migrated from `PairExtension` (`TuiExtension`) to native
//! `ClientModule` as part of M6 (#637).

use std::collections::HashMap;

use reovim_client_driver::{
    Attributes, ClientModule, ClientModuleError, InlineDecoration, ModuleContext, ProbeResult,
    Style, Version,
};

use {reovim_arch::Color, serde::Deserialize};

/// Rainbow color palette (6 colors, cycling via `depth % 6`).
const RAINBOW_COLORS: [Color; 6] = [
    Color::Rgb {
        r: 255,
        g: 215,
        b: 0,
    }, // Gold
    Color::Rgb {
        r: 218,
        g: 112,
        b: 214,
    }, // Orchid
    Color::Rgb {
        r: 0,
        g: 191,
        b: 255,
    }, // Deep sky blue
    Color::Rgb {
        r: 50,
        g: 205,
        b: 50,
    }, // Lime green
    Color::Rgb {
        r: 255,
        g: 127,
        b: 80,
    }, // Coral
    Color::Rgb {
        r: 147,
        g: 112,
        b: 219,
    }, // Medium purple
];

/// Color for unmatched brackets.
const UNMATCHED_COLOR: Color = Color::Rgb {
    r: 255,
    g: 80,
    b: 80,
};

/// Number of rainbow colors.
const RAINBOW_COUNT: usize = RAINBOW_COLORS.len();

#[derive(Debug, Deserialize)]
struct BracketEntry {
    line: u32,
    col: u32,
    depth: u64,
    #[allow(dead_code)]
    char: String,
    unmatched: bool,
}

#[derive(Debug, Deserialize)]
struct MatchedEntry {
    open: PosEntry,
    close: PosEntry,
}

#[derive(Debug, Deserialize)]
struct PosEntry {
    line: u32,
    col: u32,
}

#[derive(Deserialize)]
struct PairNotification {
    #[serde(default)]
    active: bool,
    #[serde(default = "default_true")]
    rainbow: bool,
    #[serde(default = "default_true")]
    matchpair: bool,
    #[serde(default)]
    brackets: Vec<BracketEntry>,
    #[serde(default)]
    matched: Option<MatchedEntry>,
}

const fn default_true() -> bool {
    true
}

/// Bracket pair highlighting module.
///
/// Renders rainbow-colored brackets and matched-pair highlighting
/// via inline decorations. Style-only overlays — no text replacement.
///
/// Kind: `"pair"` (matches server's pair bridge).
pub struct PairModule {
    active: bool,
    rainbow_enabled: bool,
    matchpair_enabled: bool,
    brackets: Vec<BracketEntry>,
    matched: Option<MatchedEntry>,
    /// Cached inline decorations grouped by line.
    decorations_by_line: HashMap<usize, Vec<InlineDecoration>>,
}

impl PairModule {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            rainbow_enabled: true,
            matchpair_enabled: true,
            brackets: Vec::new(),
            matched: None,
            decorations_by_line: HashMap::new(),
        }
    }

    /// Rebuild the cached inline decorations from current state.
    #[allow(clippy::cast_possible_truncation)]
    fn rebuild_decorations(&mut self) {
        self.decorations_by_line.clear();

        if !self.active {
            return;
        }

        // Rainbow brackets
        if self.rainbow_enabled {
            for bracket in &self.brackets {
                let style = if bracket.unmatched {
                    let mut attrs = Attributes::new();
                    attrs.set(Attributes::UNDERLINE);
                    Style {
                        fg: Some(UNMATCHED_COLOR),
                        attributes: attrs,
                        ..Style::default()
                    }
                } else {
                    let color_idx = (bracket.depth as usize) % RAINBOW_COUNT;
                    Style {
                        fg: Some(RAINBOW_COLORS[color_idx]),
                        ..Style::default()
                    }
                };

                self.decorations_by_line
                    .entry(bracket.line as usize)
                    .or_default()
                    .push(InlineDecoration {
                        col_start: bracket.col as u16,
                        col_end: bracket.col as u16 + 1,
                        style,
                    });
            }
        }

        // Matched pair highlight
        if self.matchpair_enabled
            && let Some(ref matched) = self.matched
        {
            let mut attrs = Attributes::new();
            attrs.set(Attributes::BOLD | Attributes::UNDERLINE);
            let style = Style {
                attributes: attrs,
                ..Style::default()
            };

            self.decorations_by_line
                .entry(matched.open.line as usize)
                .or_default()
                .push(InlineDecoration {
                    col_start: matched.open.col as u16,
                    col_end: matched.open.col as u16 + 1,
                    style: style.clone(),
                });

            self.decorations_by_line
                .entry(matched.close.line as usize)
                .or_default()
                .push(InlineDecoration {
                    col_start: matched.close.col as u16,
                    col_end: matched.close.col as u16 + 1,
                    style,
                });
        }
    }
}

impl Default for PairModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for PairModule {
    fn id(&self) -> &'static str {
        "pair"
    }

    fn kind(&self) -> &'static str {
        "pair"
    }

    fn name(&self) -> &'static str {
        "Bracket Pair"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
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
        let Ok(notification) = serde_json::from_str::<PairNotification>(data) else {
            return;
        };

        self.active = notification.active;
        self.rainbow_enabled = notification.rainbow;
        self.matchpair_enabled = notification.matchpair;
        self.brackets = notification.brackets;
        self.matched = notification.matched;

        self.rebuild_decorations();
    }

    fn inline_decorations(&self, line: usize) -> &[InlineDecoration] {
        self.decorations_by_line
            .get(&line)
            .map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests;
