//! Snippet expander implementation.
//!
//! Implements [`SnippetExpander`] from `reovim-driver-session` so that the
//! completion module can trigger snippet expansion without depending on
//! `reovim-module-snippet` types directly.
//!
//! # Architecture (#542)
//!
//! This is the policy implementation for snippet expansion, providing the
//! HOW: parsing, variable resolution, tab-stop tracking, and mode transition.

use {
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, Selection, SessionRuntime,
        SnippetExpander, TransitionContext,
    },
    reovim_kernel::api::v1::BufferId,
    reovim_types_text::Position,
};

use crate::{
    engine::ActiveSnippet, ids, parser, state::SnippetSessionState, variables::VariableContext,
};

/// Expands snippet bodies from completion confirm.
pub struct SnippetExpanderImpl;

#[cfg_attr(coverage_nightly, coverage(off))]
impl SnippetExpander for SnippetExpanderImpl {
    fn expand(
        &self,
        runtime: &mut SessionRuntime<'_>,
        buffer_id: BufferId,
        insert_pos: Position,
        snippet_body: &str,
    ) {
        // Parse the snippet body.
        let Ok(body) = parser::parse(snippet_body) else {
            // Fallback: insert raw text if parsing fails.
            runtime.insert_text(buffer_id, insert_pos, snippet_body);
            return;
        };

        // Build variable context.
        let var_ctx = VariableContext {
            file_path: runtime.buffer_file_path(buffer_id),
            line_number: insert_pos.line,
            ..VariableContext::empty()
        };

        // Expand the snippet.
        let (expanded_text, active_snippet) = ActiveSnippet::expand(&body, insert_pos, &var_ctx);
        runtime.insert_text(buffer_id, insert_pos, &expanded_text);

        let has_tab_stops = !active_snippet.is_done();
        let first_stop_range = active_snippet.current().map(|ts| (ts.start, ts.end));

        // Store active snippet state.
        let state = runtime.ext_mut::<SnippetSessionState>();
        state.active = Some(active_snippet);

        // Enter snippet navigation mode if there are tab stops.
        if has_tab_stops {
            runtime.push_mode(ids::NAVIGATING_MODE, TransitionContext::new());
            if let Some((start, end)) = first_stop_range {
                if let Some(w) = runtime.windows_mut().active_mut() {
                    w.cursor.line = start.line;
                    w.cursor.column = start.column;
                    if start == end {
                        w.selection = None;
                    } else {
                        w.selection = Some(Selection::character(start, end));
                    }
                }
                runtime.record_cursor_move(buffer_id);
                if start != end {
                    runtime.record_selection_change(buffer_id);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "expander_tests.rs"]
mod tests;
