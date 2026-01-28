use crate::{component::RenderState, content::WindowContentSource, highlight::ColorMode};

use super::Runtime;

impl Runtime {
    /// Main render method
    pub(crate) fn render(&mut self) {
        let render_start = std::time::Instant::now();

        // PHASE 1: Instant render (reads from cache, never waits)
        // Get visibility source from plugin state (fold plugin provides this)
        let visibility_source = self.plugin_state.visibility_source();
        let state = RenderState {
            buffers: &self.buffers,
            highlight_store: &self.highlight_store,
            mode: &self.mode_state,
            command_line: &self.command_line,
            pending_keys: &self.pending_keys,
            last_command: &self.last_command,
            color_mode: self.color_mode,
            theme: &self.theme,
            plugin_state: &self.plugin_state,
            visibility_source: visibility_source.as_ref(),
            indent_analyzer: &self.indent_analyzer,
            modifier_registry: Some(&self.modifier_registry),
            decoration_store: Some(&self.decoration_store),
            render_stages: &self.render_stages,
            display_registry: &self.display_registry,
        };
        let pre_render = render_start.elapsed();
        self.screen
            .render_with_state(&state)
            .expect("failed to render");
        let post_render = render_start.elapsed();
        self.screen.flush().expect("failed to flush");
        let flush_time = render_start.elapsed();

        // PHASE 2: Cache update (blocking but after user sees response)
        // This ensures next render has fresh data
        self.update_visible_highlights();
        let saturator_time = render_start.elapsed().saturating_sub(flush_time);

        tracing::trace!(
            "[RTT] render: state_build={:?} screen_render={:?} flush={:?} saturator={:?} total={:?}",
            pre_render,
            post_render.saturating_sub(pre_render),
            flush_time.saturating_sub(post_render),
            saturator_time,
            render_start.elapsed()
        );
    }

    /// Mark that a render is needed (doesn't render immediately)
    ///
    /// Use this instead of `render()` to enable render coalescing.
    /// Call `flush_render()` at the end of event processing to perform
    /// the actual render if needed.
    pub(crate) const fn request_render(&mut self) {
        self.render_pending = true;
    }

    /// Flush pending render if needed
    ///
    /// Should be called once at the end of each event loop iteration.
    /// Only renders if `request_render()` was called since the last flush.
    pub(crate) fn flush_render(&mut self) {
        if self.render_pending {
            self.render();
            self.render_pending = false;
        }
    }

    /// SATURATOR: Request highlight/decoration updates for visible buffers
    ///
    /// This is NON-BLOCKING - sends requests to background saturator tasks.
    /// Render reads from cache immediately (may show stale data on first render).
    /// Saturator sends `RenderSignal` when cache is updated.
    #[allow(clippy::cast_possible_truncation)]
    fn update_visible_highlights(&mut self) {
        const HIGHLIGHT_PADDING: u16 = 10;

        // Collect (buffer_id, viewport_start, viewport_end) for all visible windows
        let viewports: Vec<(usize, u16, u16)> = self
            .screen
            .windows()
            .iter()
            .filter_map(|win| {
                if let WindowContentSource::FileBuffer { .. } = &win.source {
                    let buffer_id = win.buffer_id()?;
                    let line_count = self.buffers.get(&buffer_id)?.contents.len() as u16;
                    let scroll_y = win.viewport.scroll.y;
                    let viewport_start = scroll_y.saturating_sub(HIGHLIGHT_PADDING);
                    let viewport_end =
                        (scroll_y + win.bounds.height + HIGHLIGHT_PADDING).min(line_count);
                    Some((buffer_id, viewport_start, viewport_end))
                } else {
                    None
                }
            })
            .collect();

        // Request saturator updates (non-blocking) or fall back to sync for buffers without saturator
        for (buffer_id, viewport_start, viewport_end) in viewports {
            if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                if buffer.has_saturator() {
                    // Non-blocking: send request to background task
                    buffer.request_saturator_update(viewport_start, viewport_end);
                } else {
                    // Fallback: sync update for buffers without saturator
                    buffer.update_highlights(viewport_start, viewport_end);
                    buffer.update_decorations(viewport_start, viewport_end);
                }
            }
        }
    }

    /// Set color mode (for :set colormode command)
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
    }
}
