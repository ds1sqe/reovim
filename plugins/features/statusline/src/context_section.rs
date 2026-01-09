//! Document context breadcrumb section
//!
//! Provides a statusline section that displays the current scope hierarchy
//! as a breadcrumb trail (e.g., `> CLAUDE.md > Section > Subsection`).
//!
//! This section uses cached context from `CursorContextUpdated` events
//! instead of polling `get_context()` on every render.

use reovim_core::{context_provider::ContextHierarchy, plugin::SectionAlignment};

use std::sync::Arc;

use crate::{
    section::{SectionContent, SectionRenderContext, StatuslineSection},
    state::SharedStatuslineManager,
};

/// Maximum number of breadcrumb items to display before truncation
const MAX_ITEMS: usize = 4;

/// Maximum length of a single breadcrumb item before truncation
const MAX_ITEM_LEN: usize = 30;

/// Default separator between breadcrumb items
const DEFAULT_SEPARATOR: &str = " > ";

/// Create the document context breadcrumb section
///
/// This section uses cached context from `CursorContextUpdated` events
/// and renders it as a breadcrumb in the statusline.
///
/// Priority is set low (10) so it renders early on the left side.
#[must_use]
pub fn create_context_section() -> StatuslineSection {
    StatuslineSection::new(
        "document_context",
        10, // Low priority = render early on left
        SectionAlignment::Left,
        render_breadcrumb,
    )
}

/// Render the context breadcrumb
///
/// Uses cached context from `CursorContextUpdated` events and formats
/// it as a breadcrumb with smart truncation.
fn render_breadcrumb(ctx: &SectionRenderContext) -> SectionContent {
    // Check if we have an active buffer
    let Some(buffer_id) = ctx.active_buffer_id else {
        return SectionContent::hidden();
    };

    // Get cached cursor context from the statusline manager
    let result = ctx
        .plugin_state
        .with::<Arc<SharedStatuslineManager>, _, _>(|manager| {
            let cached = manager.get_cached_cursor_context()?;

            // Verify context is for current buffer
            if cached.buffer_id != buffer_id {
                return None;
            }

            // Get the hierarchy from cached context and format breadcrumb
            cached.context.as_ref().map(|hierarchy| {
                format_breadcrumb(hierarchy, DEFAULT_SEPARATOR, MAX_ITEMS, MAX_ITEM_LEN)
            })
        })
        .flatten();

    match result {
        Some(breadcrumb) if !breadcrumb.is_empty() => SectionContent::new(breadcrumb),
        _ => SectionContent::hidden(),
    }
}

/// Format a context hierarchy as a breadcrumb string
///
/// # Arguments
///
/// * `context` - The context hierarchy to format
/// * `separator` - String to use between items (e.g., " > ")
/// * `max_items` - Maximum number of items before truncation
/// * `max_item_len` - Maximum length of a single item before truncation
///
/// # Returns
///
/// A formatted breadcrumb string with leading/trailing spaces
fn format_breadcrumb(
    context: &ContextHierarchy,
    separator: &str,
    max_items: usize,
    max_item_len: usize,
) -> String {
    if context.items.is_empty() {
        return String::new();
    }

    // Truncate individual items
    let items: Vec<String> = context
        .items
        .iter()
        .map(|item| truncate(&item.text, max_item_len))
        .collect();

    // If too many items, show: first ... last(n-2)
    let display = if items.len() > max_items {
        let mut result = vec![items[0].clone()];
        result.push("...".to_string());
        let remaining = max_items.saturating_sub(2);
        result.extend(
            items[items.len().saturating_sub(remaining)..]
                .iter()
                .cloned(),
        );
        result
    } else {
        items
    };

    format!(" {}{} ", display.join(separator), separator)
}

/// Truncate a string to a maximum length, adding "..." if truncated
fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use reovim_core::context_provider::{ContextHierarchy, ContextItem};

    use super::*;

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("short", 30), "short");
    }

    #[test]
    fn test_truncate_long() {
        let long = "a".repeat(50);
        let result = truncate(&long, 30);
        assert_eq!(result.len(), 30);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_exact() {
        let text = "a".repeat(30);
        let result = truncate(&text, 30);
        assert_eq!(result, text);
    }

    #[test]
    fn test_format_breadcrumb_empty() {
        let ctx = ContextHierarchy::new(0, 0, 0);
        assert_eq!(format_breadcrumb(&ctx, " > ", 4, 30), "");
    }

    #[test]
    fn test_format_breadcrumb_single() {
        let ctx = ContextHierarchy::with_items(
            0,
            0,
            0,
            vec![ContextItem {
                text: "Main".to_string(),
                start_line: 0,
                end_line: 100,
                kind: "heading".to_string(),
                level: 0,
            }],
        );
        assert_eq!(format_breadcrumb(&ctx, " > ", 4, 30), " Main >  ");
    }

    #[test]
    fn test_format_breadcrumb_multiple() {
        let ctx = ContextHierarchy::with_items(
            0,
            0,
            0,
            vec![
                ContextItem {
                    text: "File".to_string(),
                    start_line: 0,
                    end_line: 100,
                    kind: "file".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "Class".to_string(),
                    start_line: 10,
                    end_line: 80,
                    kind: "class".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "Method".to_string(),
                    start_line: 20,
                    end_line: 40,
                    kind: "function".to_string(),
                    level: 2,
                },
            ],
        );
        assert_eq!(format_breadcrumb(&ctx, " > ", 4, 30), " File > Class > Method >  ");
    }

    #[test]
    fn test_format_breadcrumb_too_many() {
        let items = (0..10)
            .map(|i| ContextItem {
                text: format!("Item{i}"),
                start_line: i * 10,
                end_line: (i + 1) * 10,
                kind: "scope".to_string(),
                level: i as usize,
            })
            .collect();

        let ctx = ContextHierarchy::with_items(0, 0, 0, items);
        let result = format_breadcrumb(&ctx, " > ", 4, 30);

        // Should have: first + "..." + last 2 = 4 items
        assert!(result.contains("Item0"));
        assert!(result.contains("..."));
        assert!(result.contains("Item8"));
        assert!(result.contains("Item9"));
    }

    #[test]
    fn test_format_breadcrumb_custom_separator() {
        let ctx = ContextHierarchy::with_items(
            0,
            0,
            0,
            vec![
                ContextItem {
                    text: "A".to_string(),
                    start_line: 0,
                    end_line: 10,
                    kind: "scope".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "B".to_string(),
                    start_line: 5,
                    end_line: 8,
                    kind: "scope".to_string(),
                    level: 1,
                },
            ],
        );
        assert_eq!(format_breadcrumb(&ctx, " / ", 4, 30), " A / B /  ");
    }

    #[test]
    fn test_format_breadcrumb_truncate_items() {
        let ctx = ContextHierarchy::with_items(
            0,
            0,
            0,
            vec![ContextItem {
                text: "a".repeat(50),
                start_line: 0,
                end_line: 10,
                kind: "scope".to_string(),
                level: 0,
            }],
        );
        let result = format_breadcrumb(&ctx, " > ", 4, 30);
        assert!(result.len() < 50 + 10); // Much shorter than original + separator + spaces
        assert!(result.contains("..."));
    }
}
