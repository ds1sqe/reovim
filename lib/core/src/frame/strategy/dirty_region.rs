//! Dirty region strategy - only render marked regions

use {
    super::{RenderCommand, RenderStrategy, VirtualBufferStrategy},
    crate::{
        frame::{DirtyRegions, FrameBuffer},
        highlight::ColorMode,
    },
};

/// Strategy 2: Dirty region tracking
///
/// Only re-renders regions that were explicitly marked dirty.
/// More efficient than full diff when change regions are known,
/// but requires careful tracking by the renderer.
pub struct DirtyRegionStrategy {
    color_mode: ColorMode,
}

impl DirtyRegionStrategy {
    /// Create a new dirty region strategy
    #[must_use]
    pub const fn new(color_mode: ColorMode) -> Self {
        Self { color_mode }
    }
}

impl Default for DirtyRegionStrategy {
    fn default() -> Self {
        Self::new(ColorMode::TrueColor)
    }
}

impl RenderStrategy for DirtyRegionStrategy {
    fn name(&self) -> &'static str {
        "dirty_region"
    }

    fn requires_previous_frame(&self) -> bool {
        false // Uses dirty hints instead of frame comparison
    }

    fn compute_commands(
        &self,
        current: &FrameBuffer,
        _previous: Option<&FrameBuffer>,
        dirty_regions: Option<&DirtyRegions>,
    ) -> Vec<RenderCommand> {
        let Some(regions) = dirty_regions else {
            // No dirty info - fall back to full frame render
            return VirtualBufferStrategy::new(self.color_mode)
                .compute_commands(current, None, None);
        };

        if regions.is_empty() {
            return Vec::new(); // Nothing dirty
        }

        let mut commands = Vec::new();
        let mut last_style: Option<String> = None;

        // Render each dirty region
        for region in regions.iter() {
            let end_y = (region.y + region.height).min(current.height());
            let end_x = (region.x + region.width).min(current.width());

            for y in region.y..end_y {
                commands.push(RenderCommand::MoveTo(region.x, y));
                let mut line_content = String::new();

                for x in region.x..end_x {
                    let Some(cell) = current.get(x, y) else {
                        continue;
                    };

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

                // Flush remaining content
                if !line_content.is_empty() {
                    commands.push(RenderCommand::Print(line_content));
                }
            }
        }

        // Reset at end
        if last_style.is_some() {
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
        let strategy = DirtyRegionStrategy::default();
        assert_eq!(strategy.name(), "dirty_region");
        assert!(!strategy.requires_previous_frame());
    }

    #[test]
    fn test_empty_regions_no_commands() {
        let strategy = DirtyRegionStrategy::default();
        let frame = FrameBuffer::new(10, 5);
        let regions = DirtyRegions::new();

        let commands = strategy.compute_commands(&frame, None, Some(&regions));
        assert!(commands.is_empty());
    }

    #[test]
    fn test_renders_dirty_region_only() {
        let strategy = DirtyRegionStrategy::default();
        let mut frame = FrameBuffer::new(20, 10);
        frame.set(5, 5, Cell::from_char('X'));

        let mut regions = DirtyRegions::new();
        regions.mark_rect(5, 5, 3, 3);

        let commands = strategy.compute_commands(&frame, None, Some(&regions));
        // Should have commands, but fewer than full frame
        assert!(!commands.is_empty());
        assert!(commands.len() < 50);
    }
}
