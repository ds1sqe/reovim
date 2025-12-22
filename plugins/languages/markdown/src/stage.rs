//! Markdown render stage for decorations (conceals, backgrounds)

use reovim_core::{
    component::RenderContext,
    decoration::DecorationStore,
    render::{Decoration, DecorationKind, RenderData, RenderStage},
};

use std::sync::{Arc, RwLock};

/// Markdown render stage - populates decorations (conceals, backgrounds)
///
/// This stage reads decorations from DecorationStore and populates
/// the decorations field in RenderData for each line.
pub struct MarkdownRenderStage {
    decoration_store: Arc<RwLock<DecorationStore>>,
}

impl MarkdownRenderStage {
    /// Create new markdown render stage
    pub fn new(decoration_store: Arc<RwLock<DecorationStore>>) -> Self {
        Self { decoration_store }
    }
}

impl RenderStage for MarkdownRenderStage {
    fn transform(&self, mut input: RenderData, _ctx: &RenderContext<'_>) -> RenderData {
        let buffer_id = input.buffer_id;

        // Lock decoration store for reading
        let store = match self.decoration_store.read() {
            Ok(store) => store,
            Err(_) => return input, // Poisoned lock - skip decorations
        };

        // Populate decorations for each line
        for (line_idx, line_decorations) in input.decorations.iter_mut().enumerate() {
            let line_num = line_idx as u32;

            // Get decorations from store
            let store_decorations = store.get_line_decorations(buffer_id, line_num);

            // Convert from decoration::Decoration to render::Decoration
            *line_decorations = store_decorations
                .into_iter()
                .filter_map(|deco| {
                    // Only process decorations that apply to this line
                    match deco {
                        reovim_core::decoration::Decoration::Conceal {
                            span,
                            replacement,
                            style,
                        } if span.start_line == line_num => {
                            let kind = if replacement.is_empty() {
                                DecorationKind::Conceal {
                                    replacement: None,
                                }
                            } else {
                                DecorationKind::Conceal {
                                    replacement: Some(replacement.clone()),
                                }
                            };

                            Some(Decoration {
                                start_col: span.start_col as usize,
                                end_col: span.end_col as usize,
                                kind,
                            })
                        }
                        reovim_core::decoration::Decoration::Hide { span }
                            if span.start_line == line_num =>
                        {
                            Some(Decoration {
                                start_col: span.start_col as usize,
                                end_col: span.end_col as usize,
                                kind: DecorationKind::Conceal {
                                    replacement: None,
                                },
                            })
                        }
                        reovim_core::decoration::Decoration::InlineStyle { span, style }
                            if span.start_line == line_num =>
                        {
                            // Inline styles can be represented as background decorations
                            Some(Decoration {
                                start_col: span.start_col as usize,
                                end_col: span.end_col as usize,
                                kind: DecorationKind::Background {
                                    style: style.clone(),
                                },
                            })
                        }
                        _ => None,
                    }
                })
                .collect();
        }

        input
    }

    fn name(&self) -> &'static str {
        "markdown"
    }
}
