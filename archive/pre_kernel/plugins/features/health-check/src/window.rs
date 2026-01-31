use {
    crate::manager::HealthCheckManager,
    reovim_core::{
        frame::FrameBuffer,
        highlight::Theme,
        plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
        screen::border::{BorderConfig, BorderStyle, WindowAdjacency, render_border_to_buffer},
    },
    std::sync::Arc,
};

/// Plugin window for health check modal
pub struct HealthCheckPluginWindow;

impl PluginWindow for HealthCheckPluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        state.with::<HealthCheckManager, _, _>(|manager| {
            if !manager.is_visible() {
                return None;
            }

            // Calculate centered bounds: 70% width, 80% height
            let term_cols = ctx.screen_width;
            let term_rows = ctx.screen_height;
            let width = (term_cols * 70 / 100).max(40);
            let height = (term_rows * 80 / 100).max(20);
            let x = (term_cols.saturating_sub(width)) / 2;
            let y = (term_rows.saturating_sub(height)) / 2;

            Some(WindowConfig {
                bounds: Rect::new(x, y, width, height),
                z_order: 700, // Modal layer
                visible: true,
            })
        })?
    }

    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        let Some(manager) = state.with::<HealthCheckManager, _, _>(Clone::clone) else {
            tracing::debug!("HealthCheck render: no manager state available");
            return;
        };

        let normal_style = &theme.popup.normal;
        let selected_style = &theme.popup.selected;

        // Render border with title
        let border_config = BorderConfig::new(BorderStyle::Rounded).with_title("Health Check");
        let adjacency = WindowAdjacency::default();
        render_border_to_buffer(
            buffer,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            &border_config,
            &theme.popup.border,
            &adjacency,
            true, // floating window
        );

        // Content area (inside border)
        let content_height = bounds.height.saturating_sub(3) as usize; // Reserve 1 for footer
        let content_y_start = bounds.y + 1;

        // Render results
        if let Some(results) = manager.last_results() {
            let mut current_row = 0;
            let mut display_row = 0;
            let mut current_category = None;

            for result in results {
                // Render category header if this is a new category
                if current_category != Some(&result.category) {
                    current_category = Some(&result.category);

                    // Check if this row is visible and selected
                    if display_row >= manager.scroll_offset()
                        && display_row < manager.scroll_offset() + content_height
                    {
                        let y = content_y_start + current_row as u16;
                        let is_selected = display_row == manager.selected_index();
                        let style = if is_selected {
                            selected_style
                        } else {
                            normal_style
                        };

                        let header_text = format!("[{}]", result.category);

                        // Draw category header
                        for (i, ch) in header_text.chars().enumerate() {
                            let x = bounds.x + 1 + i as u16;
                            if x < bounds.x + bounds.width - 1 {
                                buffer.put_char(x, y, ch, style);
                            }
                        }

                        // Fill remaining space
                        for x in
                            (bounds.x + 1 + header_text.len() as u16)..(bounds.x + bounds.width - 1)
                        {
                            buffer.put_char(x, y, ' ', style);
                        }

                        current_row += 1;
                    }
                    display_row += 1;
                }

                // Render health check result
                if display_row >= manager.scroll_offset()
                    && display_row < manager.scroll_offset() + content_height
                {
                    let y = content_y_start + current_row as u16;
                    let is_selected = display_row == manager.selected_index();
                    let style = if is_selected {
                        selected_style
                    } else {
                        normal_style
                    };

                    // Format: " ✓ Check Name: message"
                    let icon = result.result.status.icon();
                    let msg = result.result.messages.first().map_or("", String::as_str);
                    let check_text = format!(" {} {}: {}", icon, result.name, msg);

                    // Draw check result
                    for (i, ch) in check_text.chars().enumerate() {
                        let x = bounds.x + 1 + i as u16;
                        if x < bounds.x + bounds.width - 1 {
                            buffer.put_char(x, y, ch, style);
                        }
                    }

                    // Fill remaining space
                    for x in (bounds.x + 1 + check_text.len() as u16)..(bounds.x + bounds.width - 1)
                    {
                        buffer.put_char(x, y, ' ', style);
                    }

                    current_row += 1;

                    // Render expanded details if this row is expanded
                    if manager.is_expanded(display_row) {
                        // Render additional messages (skip first, already shown)
                        for msg in result.result.messages.iter().skip(1) {
                            if current_row >= content_height {
                                break;
                            }
                            let detail_y = content_y_start + current_row as u16;
                            let detail_text = format!("     {msg}");

                            for (i, ch) in detail_text.chars().enumerate() {
                                let x = bounds.x + 1 + i as u16;
                                if x < bounds.x + bounds.width - 1 {
                                    buffer.put_char(x, detail_y, ch, normal_style);
                                }
                            }

                            // Fill remaining space
                            for x in (bounds.x + 1 + detail_text.len() as u16)
                                ..(bounds.x + bounds.width - 1)
                            {
                                buffer.put_char(x, detail_y, ' ', normal_style);
                            }

                            current_row += 1;
                        }

                        // Render details field if present
                        if let Some(details) = &result.result.details {
                            for line in details.lines() {
                                if current_row >= content_height {
                                    break;
                                }
                                let detail_y = content_y_start + current_row as u16;
                                let detail_text = format!("     {line}");

                                for (i, ch) in detail_text.chars().enumerate() {
                                    let x = bounds.x + 1 + i as u16;
                                    if x < bounds.x + bounds.width - 1 {
                                        buffer.put_char(x, detail_y, ch, normal_style);
                                    }
                                }

                                // Fill remaining space
                                for x in (bounds.x + 1 + detail_text.len() as u16)
                                    ..(bounds.x + bounds.width - 1)
                                {
                                    buffer.put_char(x, detail_y, ' ', normal_style);
                                }

                                current_row += 1;
                            }
                        }
                    }
                }
                display_row += 1;
            }

            // Fill empty rows
            while current_row < content_height {
                let y = content_y_start + current_row as u16;
                for x in (bounds.x + 1)..(bounds.x + bounds.width - 1) {
                    buffer.put_char(x, y, ' ', normal_style);
                }
                current_row += 1;
            }
        } else {
            // No results yet - show message
            let y = content_y_start;
            let msg = "Running health checks...";
            for (i, ch) in msg.chars().enumerate() {
                let x = bounds.x + 1 + i as u16;
                if x < bounds.x + bounds.width - 1 {
                    buffer.put_char(x, y, ch, normal_style);
                }
            }

            // Fill remaining space
            for x in (bounds.x + 1 + msg.len() as u16)..(bounds.x + bounds.width - 1) {
                buffer.put_char(x, y, ' ', normal_style);
            }

            // Fill remaining rows
            for row in 1..content_height {
                let y = content_y_start + row as u16;
                for x in (bounds.x + 1)..(bounds.x + bounds.width - 1) {
                    buffer.put_char(x, y, ' ', normal_style);
                }
            }
        }

        // Render footer with help text
        let footer_y = bounds.y + bounds.height - 2;
        let help_text = "[j/k] Navigate [Space/Enter] Toggle [r] Re-run [q/Esc] Close";
        for (i, ch) in help_text.chars().enumerate() {
            let x = bounds.x + 1 + i as u16;
            if x < bounds.x + bounds.width - 1 {
                buffer.put_char(x, footer_y, ch, &theme.popup.border);
            }
        }

        // Fill remaining footer space
        for x in (bounds.x + 1 + help_text.len() as u16)..(bounds.x + bounds.width - 1) {
            buffer.put_char(x, footer_y, ' ', &theme.popup.border);
        }
    }
}
