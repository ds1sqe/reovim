//! Virtual frame buffer strategy - full diff comparison

use {
    super::{RenderCommand, RenderStrategy},
    crate::{
        frame::{DirtyRegions, FrameBuffer},
        highlight::ColorMode,
    },
};

/// Strategy 1: Full virtual frame buffer with diff
///
/// Compares every cell between current and previous frame,
/// only emitting commands for changed cells. Most robust but
/// requires O(cells) comparison.
pub struct VirtualBufferStrategy {
    color_mode: ColorMode,
}

impl VirtualBufferStrategy {
    /// Create a new virtual buffer strategy
    #[must_use]
    pub const fn new(color_mode: ColorMode) -> Self {
        Self { color_mode }
    }

    /// Emit commands for the entire frame (used on first render)
    fn emit_full_frame(&self, frame: &FrameBuffer) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        let mut last_style: Option<String> = None;

        for y in 0..frame.height() {
            commands.push(RenderCommand::MoveTo(0, y));
            let mut line_content = String::new();

            if let Some(row) = frame.row(y) {
                for cell in row {
                    let style_str = cell.style.to_ansi_start(self.color_mode);

                    if last_style.as_ref() != Some(&style_str) {
                        // Flush pending content
                        if !line_content.is_empty() {
                            commands.push(RenderCommand::Print(std::mem::take(&mut line_content)));
                        }

                        // Set new style
                        if style_str.is_empty() {
                            if last_style.is_some() {
                                commands.push(RenderCommand::ResetStyle);
                            }
                        } else {
                            commands.push(RenderCommand::SetStyle(style_str.clone()));
                        }
                        last_style = Some(style_str);
                    }

                    line_content.push(cell.char);
                }
            }

            // Flush remaining content
            if !line_content.is_empty() {
                commands.push(RenderCommand::Print(line_content));
            }
        }

        // Reset at end
        if last_style.is_some() {
            commands.push(RenderCommand::ResetStyle);
        }

        commands
    }
}

impl Default for VirtualBufferStrategy {
    fn default() -> Self {
        Self::new(ColorMode::TrueColor)
    }
}

impl RenderStrategy for VirtualBufferStrategy {
    fn name(&self) -> &'static str {
        "virtual_buffer"
    }

    fn requires_previous_frame(&self) -> bool {
        true
    }

    fn compute_commands(
        &self,
        current: &FrameBuffer,
        previous: Option<&FrameBuffer>,
        _dirty_regions: Option<&DirtyRegions>,
    ) -> Vec<RenderCommand> {
        let Some(prev) = previous else {
            // No previous frame - emit everything
            return self.emit_full_frame(current);
        };

        let mut commands = Vec::new();
        let mut pending_chars = String::new();
        let mut pending_start_x: u16 = 0;
        let mut pending_y: u16 = 0;
        let mut pending_style: Option<String> = None;

        // Track if we need to reset style after flush
        let mut last_emitted_style: Option<String> = None;

        // Flush pending characters to commands
        let flush_pending = |commands: &mut Vec<RenderCommand>,
                             pending: &mut String,
                             start_x: u16,
                             y: u16,
                             style: &Option<String>,
                             last_emitted: &mut Option<String>| {
            if !pending.is_empty() {
                commands.push(RenderCommand::MoveTo(start_x, y));

                // Handle style transitions
                if let Some(s) = style {
                    if s.is_empty() {
                        // Transitioning to unstyled - need reset if previous had style
                        if last_emitted.as_ref().is_some_and(|prev| !prev.is_empty()) {
                            commands.push(RenderCommand::ResetStyle);
                        }
                    } else {
                        // Has style - emit it
                        commands.push(RenderCommand::SetStyle(s.clone()));
                    }
                    *last_emitted = Some(s.clone());
                }

                commands.push(RenderCommand::Print(std::mem::take(pending)));
            }
        };

        for y in 0..current.height() {
            for x in 0..current.width() {
                let Some(curr_cell) = current.get(x, y) else {
                    continue;
                };
                let prev_cell = prev.get(x, y);

                // Check if cell differs
                let differs = prev_cell.is_none_or(|p| curr_cell.differs_from(p));

                if differs {
                    let style_str = curr_cell.style.to_ansi_start(self.color_mode);

                    // Check if we can batch with pending (same row, consecutive, same style)
                    let can_batch = pending_y == y
                        && u16::try_from(pending_start_x as usize + pending_chars.len())
                            .is_ok_and(|expected| expected == x)
                        && pending_style.as_ref() == Some(&style_str);

                    if can_batch {
                        pending_chars.push(curr_cell.char);
                    } else {
                        // Flush previous batch
                        flush_pending(
                            &mut commands,
                            &mut pending_chars,
                            pending_start_x,
                            pending_y,
                            &pending_style,
                            &mut last_emitted_style,
                        );

                        // Start new batch
                        pending_chars.push(curr_cell.char);
                        pending_start_x = x;
                        pending_y = y;
                        pending_style = Some(style_str);
                    }
                }
            }

            // Flush at end of each row
            flush_pending(
                &mut commands,
                &mut pending_chars,
                pending_start_x,
                pending_y,
                &pending_style,
                &mut last_emitted_style,
            );
        }

        // Final reset if any style was emitted
        if last_emitted_style.as_ref().is_some_and(|s| !s.is_empty()) {
            commands.push(RenderCommand::ResetStyle);
        }

        commands
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::frame::Cell};

    #[test]
    fn test_strategy_name() {
        let strategy = VirtualBufferStrategy::default();
        assert_eq!(strategy.name(), "virtual_buffer");
        assert!(strategy.requires_previous_frame());
    }

    #[test]
    fn test_full_frame_on_first_render() {
        let strategy = VirtualBufferStrategy::default();
        let mut frame = FrameBuffer::new(10, 2);
        frame.set(0, 0, Cell::from_char('H'));
        frame.set(1, 0, Cell::from_char('i'));

        let commands = strategy.compute_commands(&frame, None, None);
        // Should have at least MoveTo and Print commands
        assert!(!commands.is_empty());
    }

    #[test]
    fn test_diff_only_changed_cells() {
        let strategy = VirtualBufferStrategy::default();
        let mut prev = FrameBuffer::new(10, 2);
        let mut curr = FrameBuffer::new(10, 2);

        // Set same content in both
        prev.set(0, 0, Cell::from_char('A'));
        curr.set(0, 0, Cell::from_char('A'));

        // Change one cell
        curr.set(1, 0, Cell::from_char('B'));

        let commands = strategy.compute_commands(&curr, Some(&prev), None);
        // Should only have commands for the changed cell
        assert!(commands.len() < 10);
    }
}
