//! Line numbers annotation module.
//!
//! Renders line numbers in the gutter. Supports absolute, relative, and
//! hybrid display modes. Extracted from the hardcoded `render_line_number()`
//! in `viewport.rs` as part of the gutter framework (M5, #636).

use reovim_client_driver::{
    AnnotationContext, ClientModule, ClientModuleError, ColumnWidth, GutterCell, LineNumberMode,
    ModuleContext, OptionValue, PlatformCapabilities, ProbeResult, Style, Version,
};

/// Line numbers annotation module.
///
/// Renders right-aligned line numbers in the gutter with dynamic width
/// based on the buffer's total line count.
pub struct LineNumbersModule {
    mode: LineNumberMode,
}

impl LineNumbersModule {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: LineNumberMode::None,
        }
    }

    /// Set the line number display mode.
    pub const fn set_mode(&mut self, mode: LineNumberMode) {
        self.mode = mode;
    }
}

impl Default for LineNumbersModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for LineNumbersModule {
    fn id(&self) -> &'static str {
        "line-numbers"
    }

    fn name(&self) -> &'static str {
        "Line Numbers"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_annotations(&self) -> bool {
        self.mode != LineNumberMode::None
    }

    fn annotation_priority(&self) -> u16 {
        100 // Leftmost in gutter
    }

    fn annotation_column_width(
        &self,
        ctx: &AnnotationContext,
        _caps: &dyn PlatformCapabilities,
    ) -> ColumnWidth {
        if self.mode == LineNumberMode::None {
            return ColumnWidth::Fixed(0);
        }
        // Width = digits needed for total_lines + 1 padding
        let digits = digit_count(ctx.total_lines);
        ColumnWidth::Dynamic(digits + 1)
    }

    fn annotate(&self, line: usize, ctx: &AnnotationContext) -> Option<GutterCell> {
        if self.mode == LineNumberMode::None {
            return None;
        }

        let line_num = line + 1; // 1-indexed display
        let is_cursor_line = line == ctx.cursor_line;

        let (display_num, _) = match self.mode {
            LineNumberMode::Absolute | LineNumberMode::Hybrid => (line_num, is_cursor_line),
            LineNumberMode::Relative => {
                let rel = if is_cursor_line {
                    line_num
                } else {
                    line.abs_diff(ctx.cursor_line)
                };
                (rel, is_cursor_line)
            }
            LineNumberMode::None => return None,
        };

        let style = if is_cursor_line {
            Style::new().fg(reovim_arch::Color::Yellow)
        } else {
            Style::new().fg(reovim_arch::Color::DarkGrey)
        };

        Some(GutterCell {
            text: display_num.to_string(),
            style,
        })
    }

    fn on_option_changed(&mut self, name: &str, value: &OptionValue) {
        match (name, value) {
            ("number", OptionValue::Bool(true))
                if self.mode == LineNumberMode::None || self.mode == LineNumberMode::Relative =>
            {
                self.mode = LineNumberMode::Absolute;
            }
            ("number", OptionValue::Bool(false))
                if self.mode == LineNumberMode::Absolute || self.mode == LineNumberMode::Hybrid =>
            {
                self.mode = LineNumberMode::None;
            }
            ("relativenumber", OptionValue::Bool(true)) => {
                if self.mode == LineNumberMode::Absolute {
                    self.mode = LineNumberMode::Hybrid;
                } else if self.mode == LineNumberMode::None {
                    self.mode = LineNumberMode::Relative;
                }
            }
            ("relativenumber", OptionValue::Bool(false)) => {
                if self.mode == LineNumberMode::Hybrid {
                    self.mode = LineNumberMode::Absolute;
                } else if self.mode == LineNumberMode::Relative {
                    self.mode = LineNumberMode::None;
                }
            }
            _ => {}
        }
    }
}

/// Count the decimal digits needed to display a number.
#[allow(clippy::cast_possible_truncation)]
const fn digit_count(n: usize) -> u16 {
    if n == 0 {
        return 1;
    }
    let mut count: u16 = 0;
    let mut val = n;
    while val > 0 {
        count += 1;
        val /= 10;
    }
    count
}

#[cfg(test)]
mod tests;
