//! Cell-level delta strategy - hybrid approach

use {
    super::{RenderCommand, RenderStrategy, VirtualBufferStrategy},
    crate::{
        frame::{DirtyRegions, FrameBuffer},
        highlight::ColorMode,
    },
};

/// Strategy 3: Cell-level delta with dirty hints
///
/// Hybrid approach that combines frame diff with dirty region hints.
/// Uses dirty regions as optimization hints but also compares cells
/// for correctness. Best balance of performance and accuracy.
pub struct CellDeltaStrategy {
    color_mode: ColorMode,
}

impl CellDeltaStrategy {
    /// Create a new cell delta strategy
    #[must_use]
    pub const fn new(color_mode: ColorMode) -> Self {
        Self { color_mode }
    }
}

impl Default for CellDeltaStrategy {
    fn default() -> Self {
        Self::new(ColorMode::TrueColor)
    }
}

impl RenderStrategy for CellDeltaStrategy {
    fn name(&self) -> &'static str {
        "cell_delta"
    }

    fn requires_previous_frame(&self) -> bool {
        true
    }

    fn compute_commands(
        &self,
        current: &FrameBuffer,
        previous: Option<&FrameBuffer>,
        dirty_regions: Option<&DirtyRegions>,
    ) -> Vec<RenderCommand> {
        let Some(prev) = previous else {
            // No previous - use full frame strategy
            return VirtualBufferStrategy::new(self.color_mode)
                .compute_commands(current, None, None);
        };

        let mut commands = Vec::new();
        let mut pending_chars = String::new();
        let mut pending_start_x: u16 = 0;
        let mut pending_y: u16 = 0;
        let mut pending_style: Option<String> = None;

        // Flush pending characters to commands
        let flush_pending = |commands: &mut Vec<RenderCommand>,
                             pending: &mut String,
                             start_x: u16,
                             y: u16,
                             style: &Option<String>| {
            if !pending.is_empty() {
                commands.push(RenderCommand::MoveTo(start_x, y));
                if let Some(s) = style
                    && !s.is_empty()
                {
                    commands.push(RenderCommand::SetStyle(s.clone()));
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

                // Check if in dirty region (optimization hint) or cell actually differs
                let in_dirty = dirty_regions.is_some_and(|r| r.contains(x, y));
                let cell_differs = prev_cell.is_none_or(|p| curr_cell.differs_from(p));

                // Update if either hint says dirty OR actual comparison shows difference
                if in_dirty || cell_differs {
                    let style_str = curr_cell.style.to_ansi_start(self.color_mode);

                    // Check if we can batch
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
            );
        }

        // Final reset
        if pending_style.is_some() {
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
        let strategy = CellDeltaStrategy::default();
        assert_eq!(strategy.name(), "cell_delta");
        assert!(strategy.requires_previous_frame());
    }

    #[test]
    fn test_uses_both_dirty_hints_and_diff() {
        let strategy = CellDeltaStrategy::default();
        let mut prev = FrameBuffer::new(10, 5);
        let mut curr = FrameBuffer::new(10, 5);

        // Cell changed but NOT in dirty region
        prev.set(0, 0, Cell::from_char('A'));
        curr.set(0, 0, Cell::from_char('B'));

        // Cell unchanged but in dirty region
        prev.set(5, 2, Cell::from_char('X'));
        curr.set(5, 2, Cell::from_char('X'));

        let mut regions = DirtyRegions::new();
        regions.mark_rect(5, 2, 1, 1);

        let commands = strategy.compute_commands(&curr, Some(&prev), Some(&regions));
        // Should include both the actually-changed cell AND the dirty-marked cell
        assert!(!commands.is_empty());
    }

    #[test]
    fn test_no_commands_when_identical() {
        let strategy = CellDeltaStrategy::default();
        let frame1 = FrameBuffer::new(10, 5);
        let frame2 = FrameBuffer::new(10, 5);

        let commands = strategy.compute_commands(&frame2, Some(&frame1), None);
        // Should be empty or just a reset
        assert!(commands.len() <= 1);
    }
}
