#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TUI extension for bracket pair highlighting.
//!
//! Receives bracket state from the server's pair bridge and renders:
//! - Rainbow bracket coloring (6-color depth cycling)
//! - Matched-pair highlighting (bold + underline)
//! - Unmatched bracket warning (red + underline)

use {
    reovim_driver_display::{
        Attributes, Color, Style,
        render_backend::{RenderBackend, TuiExtension, ViewportContext},
    },
    serde::Deserialize,
};

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

/// TUI extension for bracket pair highlighting.
pub struct PairExtension {
    active: bool,
    rainbow_enabled: bool,
    matchpair_enabled: bool,
    brackets: Vec<BracketEntry>,
    matched: Option<MatchedEntry>,
}

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

impl PairExtension {
    /// Create a new pair extension.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            rainbow_enabled: true,
            matchpair_enabled: true,
            brackets: Vec::new(),
            matched: None,
        }
    }
}

impl Default for PairExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for PairExtension {
    fn kind(&self) -> &'static str {
        reovim_extension_kinds::PAIR
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(notification) = serde_json::from_str::<PairNotification>(data) else {
            return;
        };

        self.active = notification.active;
        self.rainbow_enabled = notification.rainbow;
        self.matchpair_enabled = notification.matchpair;
        self.brackets = notification.brackets;
        self.matched = notification.matched;
    }

    fn render(&self, _backend: &mut dyn RenderBackend) {
        // Pair rendering requires viewport context; see render_with_viewport.
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_with_viewport(&self, backend: &mut dyn RenderBackend, viewport: &ViewportContext) {
        if !self.active {
            return;
        }

        let (width, height) = backend.size();
        let scroll_top = viewport.scroll_top as u32;
        let content_x = viewport.content_x;

        // Render rainbow brackets
        if self.rainbow_enabled {
            for bracket in &self.brackets {
                // Skip brackets outside viewport
                if bracket.line < scroll_top || bracket.line >= scroll_top + u32::from(height) {
                    continue;
                }

                let screen_y = (bracket.line - scroll_top) as u16;
                let screen_x = bracket.col as u16 + content_x;

                if screen_x >= width {
                    continue;
                }

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

                backend.apply_style(screen_x, screen_y, &style);
            }
        }

        // Render matched pair highlight
        if self.matchpair_enabled
            && let Some(ref matched) = self.matched
        {
            let style = {
                let mut attrs = Attributes::new();
                attrs.set(Attributes::BOLD | Attributes::UNDERLINE);
                Style {
                    attributes: attrs,
                    ..Style::default()
                }
            };

            // Open bracket
            if matched.open.line >= scroll_top && matched.open.line < scroll_top + u32::from(height)
            {
                let y = (matched.open.line - scroll_top) as u16;
                let x = matched.open.col as u16 + content_x;
                if x < width {
                    backend.apply_style(x, y, &style);
                }
            }

            // Close bracket
            if matched.close.line >= scroll_top
                && matched.close.line < scroll_top + u32::from(height)
            {
                let y = (matched.close.line - scroll_top) as u16;
                let x = matched.close.col as u16 + content_x;
                if x < width {
                    backend.apply_style(x, y, &style);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
