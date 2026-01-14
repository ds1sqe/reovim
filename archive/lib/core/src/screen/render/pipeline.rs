//! Rendering pipeline orchestration for screen windows
//!
//! This module contains the main rendering orchestration:
//! - `execute_pipeline()` - Orchestrates render stages (syntax, decorations, etc.)
//! - `render_windows()` - Main entry point that renders all windows to the frame buffer

use crate::{
    buffer::Buffer,
    command_line::CommandLine,
    decoration::DecorationStore,
    highlight::{ColorMode, HighlightStore, Theme},
    indent::IndentAnalyzer,
    modd::{ComponentId, ModeState},
    modifier::{ModifierContext, ModifierRegistry},
    visibility::BufferVisibilitySource,
};

use super::super::{Screen, ViewportScrollInfo, window::Window};

use reovim_sys::{
    cursor::{Hide, MoveTo, Show},
    queue,
};

use std::{collections::BTreeMap, io::Write};

impl Screen {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::screen) fn execute_pipeline(
        &self,
        window: &Window,
        text_buffer: &Buffer,
        _highlight_store: &HighlightStore,
        theme: &Theme,
        color_mode: ColorMode,
        _visibility_source: &dyn BufferVisibilitySource,
        _indent_analyzer: &IndentAnalyzer,
        _decoration_store: Option<&DecorationStore>,
        render_stages: &std::sync::Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
        mode: &crate::modd::ModeState,
    ) -> crate::render::RenderData {
        use crate::{component::RenderContext, render::RenderData};

        let pipeline_start = std::time::Instant::now();

        // Stage 1: Extract buffer content
        let mut data = RenderData::from_buffer(window, text_buffer, mode);
        data.buffer_id = window.buffer_id().unwrap_or(0);
        let from_buffer_time = pipeline_start.elapsed();

        // Create render context
        let ctx = RenderContext::new(self.size.width, self.size.height, theme, color_mode);

        // Execute registered render stages in order
        {
            let stages_guard = render_stages.read().unwrap();
            for stage in stages_guard.stages() {
                let stage_start = std::time::Instant::now();
                data = stage.transform(data, &ctx);
                tracing::trace!("[RTT] stage '{}' took {:?}", stage.name(), stage_start.elapsed());
            }
        }

        tracing::trace!(
            "[RTT] execute_pipeline: from_buffer={:?} stages={:?} total={:?}",
            from_buffer_time,
            pipeline_start.elapsed().saturating_sub(from_buffer_time),
            pipeline_start.elapsed()
        );

        data
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    pub(in crate::screen) fn render_windows(
        &mut self,
        buffers: &BTreeMap<usize, Buffer>,
        highlight_store: &HighlightStore,
        mode: &ModeState,
        cmd_line: &CommandLine,
        pending_keys: &str,
        last_command: &str,
        color_mode: ColorMode,
        theme: &Theme,
        visibility_source: &dyn BufferVisibilitySource,
        indent_analyzer: &IndentAnalyzer,
        modifier_registry: Option<&ModifierRegistry>,
        decoration_store: Option<&DecorationStore>,
        render_stages: &std::sync::Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
        plugin_state: &std::sync::Arc<crate::plugin::PluginStateRegistry>,
        display_registry: &crate::display::DisplayRegistry,
    ) -> std::result::Result<(), std::io::Error> {
        let rw_start = std::time::Instant::now();
        // Take frame renderer out (borrow checker workaround)
        let mut renderer = self
            .frame_renderer
            .take()
            .expect("frame renderer should always be initialized");

        // Set color mode for style conversion
        renderer.set_color_mode(color_mode);

        // Clear buffer for fresh frame
        renderer.clear();
        let buffer = renderer.buffer_mut();

        // Track cursor position
        let mut cursor_pos: Option<(u16, u16)> = None;
        let mut current_buffer: Option<&Buffer> = None;

        // Render tab line if multiple tabs exist
        self.render_tab_line_to_buffer(buffer, color_mode, theme);

        // Phase 2: Render editor windows
        // Phase 3 will add overlay collection and unified z-order rendering

        // Query panel offsets from plugin state and update layout manager if changed
        let left_offset = plugin_state.left_panel_width();
        let right_offset = plugin_state.right_panel_width();
        let offsets_changed =
            left_offset != self.layout.left_offset() || right_offset != self.layout.right_offset();

        if offsets_changed {
            self.layout.set_left_offset(left_offset);
            self.layout.set_right_offset(right_offset);
            self.update_window_layouts();
        }

        // Clear pending scroll events from previous frame
        self.pending_viewport_scrolls.clear();

        // Update scroll positions BEFORE cloning windows
        for win in self.window_store.iter_mut() {
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                let effective_cursor_y = if win.is_active {
                    buf.cur.y
                } else {
                    win.viewport.cursor.y
                };
                let scrolled = win.update_scroll(effective_cursor_y);
                if scrolled {
                    let (top_line, bottom_line) = win.viewport_bounds();
                    self.pending_viewport_scrolls.push(ViewportScrollInfo {
                        window_id: win.id.raw(),
                        buffer_id,
                        top_line,
                        bottom_line,
                    });
                }
            }
        }

        // Collect all windows (editor + plugin windows)
        let mut windows_to_render = self.window_store.windows_vec();

        // Extract active window info for EditorContext
        let (
            active_anchor_x,
            active_anchor_y,
            active_gutter_width,
            active_scroll_y,
            active_height,
            cursor_col,
            cursor_row,
            active_buffer_content,
        ) = self
            .window_store
            .iter()
            .find(|w| w.is_active)
            .and_then(|win: &super::super::window::Window| {
                let buffer_id = win.buffer_id()?;
                let buf = buffers.get(&buffer_id)?;
                let line_num_width = win.line_number_width(buf.contents.len());
                // Use max width for layout (assume signs present in auto mode)
                let sign_width = win.config.sign_column_mode.effective_width(true);
                let gutter_width = sign_width + line_num_width;
                let scroll_y = win.buffer_anchor().map_or(0, |a| a.y);
                let height = win.bounds.height;
                // Get buffer content for context providers (e.g., sticky headers)
                let snapshot = crate::buffer::BufferSnapshot::from_buffer(buf);
                let content = snapshot.content();
                Some((
                    win.bounds.x,
                    win.bounds.y,
                    gutter_width,
                    scroll_y,
                    height,
                    buf.cur.x,
                    buf.cur.y,
                    Some(content),
                ))
            })
            .unwrap_or((0, 0, 0, 0, 0, 0, 0, None));

        // Build editor context for window providers
        let editor_ctx = crate::plugin::EditorContext::new(
            self.size.width,
            self.size.height,
            mode.edit_mode.clone(),
            mode.sub_mode.clone(),
            mode.interactor_id,
            self.active_buffer_id().unwrap_or(0),
            buffers.len(),
            color_mode,
        )
        .with_left_offset(left_offset)
        .with_pending_keys(pending_keys)
        .with_active_window(
            active_anchor_x,
            active_anchor_y,
            active_gutter_width,
            active_scroll_y,
            active_height,
        )
        .with_cursor(cursor_col, cursor_row)
        .with_buffer_content(active_buffer_content);

        // Sort all windows by z-order
        windows_to_render.sort_by_key(|w| w.z_order);

        // Render all windows
        for win in &mut windows_to_render {
            // Handle PluginBuffer windows differently
            if let crate::content::WindowContentSource::PluginBuffer { provider, .. } = &win.source
            {
                // For plugin buffers, generate virtual content via provider
                use crate::content::BufferContext;
                let buffer_ctx = BufferContext {
                    buffer_id: 0,
                    width: win.bounds.width,
                    height: win.bounds.height,
                    state: plugin_state,
                };
                let lines = provider.get_lines(&buffer_ctx);

                // Render plugin buffer lines directly to frame buffer
                for (line_idx, line) in lines.iter().enumerate() {
                    let y = win.bounds.y + line_idx as u16;
                    if y >= self.size.height {
                        break;
                    }
                    let mut x = win.bounds.x;
                    for ch in line.chars() {
                        if x >= win.bounds.x + win.bounds.width {
                            break;
                        }
                        buffer.put_char(x, y, ch, &theme.base.default);
                        x += 1;
                    }
                }
                continue; // Skip normal window rendering
            }

            // Normal FileBuffer windows
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                current_buffer = Some(buf);

                // Check if this window exists in the store (for mutable access)
                // Plugin windows won't have a match, so we skip modifier/scroll updates for them
                let win_id = win.id;
                let is_editor_window = self.window_store.get(win_id).is_some();

                // Evaluate modifiers for this window (only for editor windows)
                if is_editor_window && let Some(registry) = modifier_registry {
                    let filetype = buf
                        .file_path
                        .as_ref()
                        .map(|p| crate::filetype::filetype_id(p));
                    let mod_ctx = ModifierContext::new(
                        ComponentId::EDITOR,
                        &mode.edit_mode,
                        &mode.sub_mode,
                        win.id.raw(),
                        buffer_id,
                    )
                    .with_filetype(filetype)
                    .with_active(win.is_active)
                    .with_modified(buf.modified)
                    .with_floating(win.is_floating);

                    let style_state = registry.evaluate(&mod_ctx);

                    // Apply window decorations from modifiers (need mutable access)
                    if let Some(win_mut) = self.window_store.get_mut(win_id) {
                        if let Some(show_ln) = style_state.style.decorations.line_numbers {
                            win_mut.set_number(show_ln);
                        }
                        if let Some(relative) = style_state.style.decorations.relative_numbers {
                            win_mut.set_relative_number(relative);
                        }
                        if let Some(scrollbar) = style_state.style.decorations.scrollbar {
                            win_mut.config.scrollbar_enabled = scrollbar;
                        }
                    }
                    // Note: scroll position already updated before window cloning
                }

                // Execute render pipeline with the window from windows_to_render
                let pipeline_start = std::time::Instant::now();
                let render_data = self.execute_pipeline(
                    win,
                    buf,
                    highlight_store,
                    theme,
                    color_mode,
                    visibility_source,
                    indent_analyzer,
                    decoration_store,
                    render_stages,
                    mode,
                );
                let pipeline_time = pipeline_start.elapsed();

                // Adjust scroll if virtual lines would push cursor off-screen
                let current_scroll = win.buffer_anchor().map_or(0, |a| a.y);
                if let Some(new_scroll) = Self::calculate_scroll_adjustment_for_virtual_lines(
                    &render_data,
                    current_scroll,
                    win.bounds.height,
                    buf.cur.y,
                ) {
                    // Update both the cloned window and the original
                    if let Some(mut anchor) = win.buffer_anchor() {
                        anchor.y = new_scroll;
                        win.set_buffer_anchor(anchor);
                    }
                    if is_editor_window
                        && let Some(store_win) = self.window_store.get_mut(win_id)
                        && let Some(mut anchor) = store_win.buffer_anchor()
                    {
                        anchor.y = new_scroll;
                        store_win.set_buffer_anchor(anchor);
                    }
                    tracing::trace!(
                        "[VLINE] adjusted scroll: {} -> {} for virtual lines",
                        current_scroll,
                        new_scroll
                    );
                }

                // Render pipeline data to frame buffer
                let fb_start = std::time::Instant::now();
                self.render_data_to_framebuffer(&render_data, win, buffer, theme, buf);
                tracing::trace!(
                    "[RTT] window render: pipeline={:?} framebuffer={:?}",
                    pipeline_time,
                    fb_start.elapsed()
                );

                // Calculate cursor position only for the ACTIVE window (and if editor is focused)
                if win.is_active {
                    use crate::render::VirtualLinePosition;

                    let line_num_width = win.line_number_width(buf.contents.len());
                    // Use max width for cursor position (assume signs present in auto mode)
                    let sign_width = win.config.sign_column_mode.effective_width(true);

                    // Apply column mapping for decorated lines (e.g., table expansion)
                    let visual_cursor_col = render_data
                        .cursor_col_mapping()
                        .and_then(|mapping| mapping.get(buf.cur.x as usize).copied())
                        .unwrap_or(buf.cur.x);
                    let cursor_x = win.bounds.x + sign_width + line_num_width + visual_cursor_col;

                    // Calculate virtual line offset for cursor position
                    // Virtual lines rendered BEFORE buffer lines shift the cursor down
                    let scroll_offset = win.buffer_anchor().map_or(0, |a| a.y);
                    let cursor_buffer_line = buf.cur.y;

                    // Count virtual lines that appear visually before the cursor line
                    // - Before: renders before line N, count if N <= cursor
                    // - After: renders after line N, count if N < cursor (before cursor line)
                    let virtual_line_offset: u16 = render_data
                        .virtual_lines
                        .keys()
                        .filter(|(line_idx, pos)| {
                            let line = *line_idx as u16;
                            line >= scroll_offset
                                && match pos {
                                    VirtualLinePosition::Before => line <= cursor_buffer_line,
                                    VirtualLinePosition::After => line < cursor_buffer_line,
                                }
                        })
                        .count() as u16;

                    let cursor_y = win.bounds.y
                        + cursor_buffer_line.saturating_sub(scroll_offset)
                        + virtual_line_offset;
                    cursor_pos = Some((cursor_x, cursor_y));
                }
            }
        }

        // Render plugin windows (new unified system)
        // These are sorted by z_order and rendered on top of editor windows
        self.render_plugin_windows(buffer, plugin_state, &editor_ctx, theme);

        // Render window separators
        self.render_window_separators_to_buffer(buffer, theme);

        // Render status line or command line
        if mode.is_command() {
            self.render_command_line_to_buffer(buffer, cmd_line, theme);
        } else {
            self.render_status_line_to_buffer(
                buffer,
                mode,
                current_buffer,
                pending_keys,
                last_command,
                theme,
                color_mode,
                plugin_state,
                display_registry,
            );
        }

        // Settings menu overlay is now rendered by the settings-menu plugin

        // Put renderer back and flush
        let pre_flush = rw_start.elapsed();
        self.frame_renderer = Some(renderer);
        let renderer = self.frame_renderer.as_mut().unwrap();
        queue!(self.out_stream, Hide)?;
        renderer.flush(&mut self.out_stream)?;
        let post_renderer_flush = rw_start.elapsed();

        // Position cursor
        if let Some((x, y)) = cursor_pos {
            queue!(self.out_stream, MoveTo(x, y))?;
        }

        queue!(self.out_stream, Show)?;
        let result = self.out_stream.flush();
        tracing::trace!(
            "[RTT] render_windows: pre_flush={:?} renderer.flush={:?} stream.flush={:?} total={:?}",
            pre_flush,
            post_renderer_flush.saturating_sub(pre_flush),
            rw_start.elapsed().saturating_sub(post_renderer_flush),
            rw_start.elapsed()
        );
        result
    }
}
