//! Bounded terminal-cell projection; no terminal escapes or Source mutation.

use crate::context::Window;
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt::Write;
use reovim_module_text::api::Unit;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidWindow,
    TooNarrow,
    ColumnContext,
}

impl core::fmt::Display for Error {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(output, "viewport: {self:?}")
    }
}
impl core::error::Error for Error {}

pub struct Frame {
    pub lines: Vec<String>,
    pub cursor: Option<(u16, u16)>,
    pub top: usize,
}

struct Layout {
    lines: Vec<String>,
    x: usize,
    columns: usize,
    cursor: Option<(usize, usize)>,
    logical_column: Option<usize>,
}

impl Layout {
    fn hard_break(&mut self, selected: bool) {
        if selected {
            self.cursor = Some((self.x, self.lines.len() - 1));
        }
        self.newline();
        self.logical_column = Some(0);
    }
    fn newline(&mut self) {
        self.lines.push(String::new());
        self.x = 0;
    }

    fn text(&mut self, value: &str, selected: bool) -> Result<(), Error> {
        let width = value.width();
        if width > self.columns {
            return Err(Error::TooNarrow);
        }
        if self.x + width > self.columns {
            self.newline();
        }
        if selected {
            self.cursor = Some((self.x, self.lines.len() - 1));
        }
        self.lines.last_mut().unwrap().push_str(value);
        self.x += width;
        self.logical_column =
            self.logical_column.map(|column| (column + width) % 4);
        Ok(())
    }
}

/// Projects one 1024-unit Text window, reserving its last column for the caret.
/// Cursor coordinates count cells; input positions still count Source units.
/// # Errors
/// Rejects invalid dimensions/windows or a glyph wider than the terminal.
pub fn project(
    window: &Window,
    cursor: usize,
    columns: u16,
    rows: u16,
    top: usize,
) -> Result<Frame, Error> {
    if columns == 0
        || rows == 0
        || window.units.len() > 1024
        || cursor > window.units.len()
        || window.range.end.checked_sub(window.range.start)
            != Some(window.units.len() as u64)
    {
        return Err(Error::InvalidWindow);
    }
    let mut layout = Layout {
        lines: vec![String::new()],
        x: 0,
        columns: usize::from(columns - 1),
        cursor: None,
        logical_column: window.column(),
    };
    project_units(&mut layout, &window.units, cursor)?;
    if cursor == window.units.len() {
        layout.cursor = Some((layout.x, layout.lines.len() - 1));
    }
    let top = visible_top(&layout, usize::from(rows), top);
    let cursor = layout
        .cursor
        .and_then(|(x, y)| {
            Some((
                u16::try_from(x).ok()?,
                u16::try_from(y.checked_sub(top)?).ok()?,
            ))
        })
        .filter(|(_, y)| *y < rows);
    Ok(Frame {
        lines: layout
            .lines
            .into_iter()
            .skip(top)
            .take(usize::from(rows))
            .collect(),
        cursor,
        top,
    })
}

fn visible_top(layout: &Layout, rows: usize, requested: usize) -> usize {
    let top = requested.min(layout.lines.len().saturating_sub(rows));
    match layout.cursor {
        Some((_, y)) if y < top => y,
        Some((_, y)) if y >= top + rows => y + 1 - rows,
        _ => top,
    }
}

fn project_units(
    layout: &mut Layout,
    units: &[Unit],
    cursor: usize,
) -> Result<(), Error> {
    let mut offset = 0;
    while offset < units.len() {
        if let Unit::Opaque(byte) = units[offset] {
            layout.text(&format!("⟦{byte:02x}⟧"), cursor == offset)?;
            offset += 1;
            continue;
        }
        let mut text = String::new();
        let start = offset;
        while let Some(Unit::Scalar(ch)) = units.get(offset) {
            text.push(*ch);
            offset += 1;
        }
        project_text(layout, &text, start, cursor)?;
    }
    Ok(())
}

fn project_text(
    layout: &mut Layout,
    text: &str,
    mut offset: usize,
    cursor: usize,
) -> Result<(), Error> {
    for cluster in text.graphemes(true) {
        let selected = offset == cursor;
        offset += cluster.chars().count();
        if matches!(cluster, "\r\n" | "\r" | "\n") {
            layout.hard_break(selected);
            continue;
        }
        if cluster == "\t" {
            tab(layout, selected)?;
            continue;
        }
        let displayed = display(cluster);
        layout.text(&displayed, selected)?;
    }
    Ok(())
}

fn tab(layout: &mut Layout, selected: bool) -> Result<(), Error> {
    let width = 4 - layout.logical_column.ok_or(Error::ColumnContext)?;
    for index in 0..width {
        layout.text(" ", selected && index == 0)?;
    }
    Ok(())
}

pub(crate) fn column_after(
    units: &[Unit],
    column: Option<usize>,
) -> Option<usize> {
    let width = match units {
        [Unit::Scalar('\r' | '\n' | '\t')]
        | [Unit::Scalar('\r'), Unit::Scalar('\n')] => return Some(0),
        [Unit::Opaque(_)] => 4,
        [Unit::Scalar(ch)] if !ch.is_control() => {
            ch.width().unwrap_or(0).max(1)
        }
        _ => {
            let text: String = units
                .iter()
                .filter_map(|unit| match unit {
                    Unit::Scalar(ch) => Some(*ch),
                    Unit::Opaque(_) => None,
                })
                .collect();
            display(&text).width()
        }
    };
    column.map(|column| (column + width) % 4)
}

fn display(cluster: &str) -> String {
    if cluster.chars().any(char::is_control) {
        return controls(cluster);
    }
    if cluster.width() == 0 {
        return format!("◌{cluster}");
    }
    cluster.into()
}

fn controls(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        write!(result, "⟦{:x}⟧", u32::from(ch)).unwrap();
    }
    result
}

/// Snaps a planned scalar cursor forward to a local grapheme boundary.
#[must_use]
pub fn cursor_boundary(units: &[Unit], position: usize) -> usize {
    boundaries(units)
        .into_iter()
        .find(|end| *end >= position)
        .unwrap_or(units.len())
}

pub(crate) fn step(units: &[Unit], position: usize, forward: bool) -> usize {
    let values = boundaries(units);
    if forward {
        return values
            .into_iter()
            .find(|end| *end > position)
            .unwrap_or(units.len());
    }
    values.into_iter().rev().find(|start| *start < position).unwrap_or(0)
}

fn boundaries(units: &[Unit]) -> Vec<usize> {
    let mut result = vec![0];
    let mut offset = 0;
    while offset < units.len() {
        if matches!(units[offset], Unit::Opaque(_)) {
            offset += 1;
            result.push(offset);
            continue;
        }
        let mut text = String::new();
        let mut end = offset;
        while let Some(Unit::Scalar(ch)) = units.get(end) {
            text.push(*ch);
            end += 1;
        }
        for cluster in text.graphemes(true) {
            offset += cluster.chars().count();
            result.push(offset);
        }
    }
    result
}

/// Clips one safe status line at whole grapheme boundaries.
#[must_use]
pub fn status(text: &str, columns: u16) -> String {
    let safe = text.escape_debug().to_string();
    let mut result = String::new();
    let mut width = 0;
    for cluster in safe.graphemes(true) {
        width += cluster.width();
        if width > usize::from(columns) {
            break;
        }
        result.push_str(cluster);
    }
    result
}
