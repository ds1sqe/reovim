//! Category-to-behavior mappings for markdown rendering.
//!
//! This is where markdown rendering policy lives. The viewport renderer
//! calls `classify_markdown_token()` via `ClientModule::classify_token()`
//! to decide how to render each token category.

use {reovim_client_driver::RenderBehavior, std::borrow::Cow};

/// Classify a markdown token category into a render behavior.
///
/// Returns `Some(behavior)` for markdown categories, `None` for
/// categories that should fall through to the default `Highlight`.
#[must_use]
pub fn classify_markdown_token(category: &str) -> Option<RenderBehavior> {
    match category {
        // Heading markers -> Nerd Font icons
        "markup.heading.1" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{f0965} "),
        }),
        "markup.heading.2" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{f096c} "),
        }),
        "markup.heading.3" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{f096d} "),
        }),
        "markup.heading.4" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{f096e} "),
        }),
        "markup.heading.5" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{f096f} "),
        }),
        "markup.heading.6" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{f0970} "),
        }),

        // List bullets -> depth-dependent glyphs
        "markup.list.bullet.0" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{2022} "),
        }),
        "markup.list.bullet.1" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{25E6} "),
        }),
        "markup.list.bullet.2" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{25AA} "),
        }),
        c if c.starts_with("markup.list.bullet.") => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{25AB} "),
        }),

        // Checkboxes
        "markup.list.checkbox" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{2610} "),
        }),
        "markup.list.checkbox.checked" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{2713} "),
        }),

        // Blockquote marker
        "markup.quote.marker" => Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("\u{2502} "),
        }),

        // Horizontal rule -> fill viewport width
        "markup.horizontal_rule" => Some(RenderBehavior::FullWidthLine {
            ch: '\u{2500}',
            style: reovim_client_driver::Style::default(),
        }),

        // Code span delimiters -> hide (zero-width conceal)
        "markup.raw.delimiter" => Some(RenderBehavior::Hide),

        // Code block -> background
        "markup.raw.block" => Some(RenderBehavior::Background(
            reovim_arch::Color::default(),
        )),

        // Not a markdown category -- fall through
        _ => None,
    }
}

#[cfg(test)]
#[path = "behaviors_tests.rs"]
mod tests;
