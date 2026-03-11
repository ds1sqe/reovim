//! Category-to-behavior mappings for markdown rendering.
//!
//! This is where markdown rendering policy lives. The render engine
//! calls `classify_markdown_token()` via `TuiExtension::classify_token()`
//! to decide how to render each token category.

use {reovim_driver_display::render_backend::RenderBehavior, std::borrow::Cow};

/// Classify a markdown token category into a render behavior.
///
/// Returns `Some(behavior)` for markdown categories, `None` for
/// categories that should fall through to the default `Highlight`.
#[must_use]
pub fn classify_markdown_token(category: &str) -> Option<RenderBehavior> {
    match category {
        // Heading markers → Nerd Font icons
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

        // List bullets → depth-dependent glyphs
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

        // Horizontal rule → fill viewport width
        "markup.horizontal_rule" => Some(RenderBehavior::FullWidthLine { ch: '─' }),

        // Code span delimiters → hide (zero-width conceal)
        "markup.raw.delimiter" => Some(RenderBehavior::Hide),

        // Code block → background
        "markup.raw.block" => Some(RenderBehavior::Background),

        // Not a markdown category — fall through
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_heading_levels() {
        for (level, cat) in [
            (1, "markup.heading.1"),
            (2, "markup.heading.2"),
            (3, "markup.heading.3"),
            (4, "markup.heading.4"),
            (5, "markup.heading.5"),
            (6, "markup.heading.6"),
        ] {
            let result = classify_markdown_token(cat);
            assert!(
                matches!(result, Some(RenderBehavior::Conceal { .. })),
                "heading level {level} should be Conceal"
            );
        }
    }

    #[test]
    fn test_classify_heading_replacements_unique() {
        let mut replacements = Vec::new();
        for level in 1..=6 {
            let cat = format!("markup.heading.{level}");
            if let Some(RenderBehavior::Conceal { replacement }) = classify_markdown_token(&cat) {
                replacements.push(replacement.into_owned());
            }
        }
        assert_eq!(replacements.len(), 6);
        // All should be unique
        let unique: std::collections::HashSet<_> = replacements.iter().collect();
        assert_eq!(unique.len(), 6);
    }

    #[test]
    fn test_classify_list_bullets() {
        assert!(matches!(
            classify_markdown_token("markup.list.bullet.0"),
            Some(RenderBehavior::Conceal { .. })
        ));
        assert!(matches!(
            classify_markdown_token("markup.list.bullet.1"),
            Some(RenderBehavior::Conceal { .. })
        ));
        assert!(matches!(
            classify_markdown_token("markup.list.bullet.2"),
            Some(RenderBehavior::Conceal { .. })
        ));
        // Wildcard depth
        assert!(matches!(
            classify_markdown_token("markup.list.bullet.99"),
            Some(RenderBehavior::Conceal { .. })
        ));
    }

    #[test]
    fn test_classify_checkbox() {
        assert!(matches!(
            classify_markdown_token("markup.list.checkbox"),
            Some(RenderBehavior::Conceal { .. })
        ));
        assert!(matches!(
            classify_markdown_token("markup.list.checkbox.checked"),
            Some(RenderBehavior::Conceal { .. })
        ));
    }

    #[test]
    fn test_classify_blockquote_marker() {
        assert!(matches!(
            classify_markdown_token("markup.quote.marker"),
            Some(RenderBehavior::Conceal { .. })
        ));
    }

    #[test]
    fn test_classify_horizontal_rule() {
        assert!(matches!(
            classify_markdown_token("markup.horizontal_rule"),
            Some(RenderBehavior::FullWidthLine { ch: '─' })
        ));
    }

    #[test]
    fn test_classify_code_delimiter_hide() {
        assert!(matches!(
            classify_markdown_token("markup.raw.delimiter"),
            Some(RenderBehavior::Hide)
        ));
    }

    #[test]
    fn test_classify_code_block_background() {
        assert!(matches!(
            classify_markdown_token("markup.raw.block"),
            Some(RenderBehavior::Background)
        ));
    }

    #[test]
    fn test_classify_unknown_none() {
        assert!(classify_markdown_token("keyword.function").is_none());
        assert!(classify_markdown_token("string").is_none());
        assert!(classify_markdown_token("comment").is_none());
    }
}
