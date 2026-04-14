//! Text-domain implementations of `dyn Position` and `dyn Cursor`.
//!
//! # Type Discrimination
//!
//! - `inner_id=0`: [`TextPosition`] — a `(line, col)` coordinate
//! - `inner_id=1`: [`TextCursor`] — a position + optional selection
//!
//! `flags` carry per-instance state within a type:
//! - bit 0: has selection (only meaningful for `inner_id=1`)
//!
//! # Content Encoding (little-endian)
//!
//! - `TextPosition`: `[line: u64, col: u64]` = 16 bytes
//! - `TextCursor` without selection: `[line: u64, col: u64]` = 16 bytes
//! - `TextCursor` with selection:
//!   `[line: u64, col: u64, sel_start_line: u64, sel_start_col: u64, sel_mode: u8]`
//!   = 33 bytes
//!
//! `u64` for line/col is a conscious oversizing choice — `u32` would suffice for
//! text editors but `u64` avoids narrowing if future domains use the same encoding.

use reovim_subsys_coordination::{
    Cursor, CursorCodec, CursorHeader, Position, PositionCodec, PositionHeader,
};

use crate::{CursorPosition, api::SelectionMode};

/// Inner ID for text positions.
pub const TEXT_POSITION_INNER_ID: u16 = 0;
/// Inner ID for text cursors.
pub const TEXT_CURSOR_INNER_ID: u16 = 1;
/// Flag bit 0: cursor has an active selection.
const FLAG_HAS_SELECTION: u16 = 1;

// ============================================================================
// TextPosition
// ============================================================================

/// Text-domain position — `(line, col)` encoded as `dyn Position`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPosition {
    header: PositionHeader,
    line: u64,
    col: u64,
    content: Vec<u8>,
}

impl TextPosition {
    /// Create a new text position.
    #[must_use]
    pub fn new(domain_id: u32, line: u64, col: u64) -> Self {
        let header = PositionHeader::new(domain_id, TEXT_POSITION_INNER_ID, 0);
        let content = Self::encode_content(line, col);
        Self {
            header,
            line,
            col,
            content,
        }
    }

    /// Line number (0-indexed).
    #[must_use]
    pub const fn line(&self) -> u64 {
        self.line
    }

    /// Column number (0-indexed).
    #[must_use]
    pub const fn col(&self) -> u64 {
        self.col
    }

    fn encode_content(line: u64, col: u64) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&line.to_le_bytes());
        buf.extend_from_slice(&col.to_le_bytes());
        buf
    }

    fn decode_content(bytes: &[u8]) -> Option<(u64, u64)> {
        if bytes.len() < 16 {
            return None;
        }
        let line = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let col = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        Some((line, col))
    }
}

impl Position for TextPosition {
    fn header(&self) -> &PositionHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.content
    }

    fn display(&self) -> String {
        format!("{}:{}", self.line, self.col)
    }

    fn clone_box(&self) -> Box<dyn Position> {
        Box::new(self.clone())
    }
}

impl From<(u32, CursorPosition)> for TextPosition {
    fn from((domain_id, cp): (u32, CursorPosition)) -> Self {
        #[allow(clippy::cast_possible_truncation)] // usize→u64 widening on 64-bit; never truncates
        Self::new(domain_id, cp.line as u64, cp.column as u64)
    }
}

// ============================================================================
// SelectionData
// ============================================================================

/// Selection data stored inside a `TextCursor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionData {
    /// Selection start line.
    pub start_line: u64,
    /// Selection start column.
    pub start_col: u64,
    /// Selection mode.
    pub mode: SelectionMode,
}

impl SelectionData {
    const fn mode_byte(&self) -> u8 {
        match self.mode {
            SelectionMode::Character => 0,
            SelectionMode::Line => 1,
            SelectionMode::Block => 2,
        }
    }

    const fn mode_from_byte(b: u8) -> Option<SelectionMode> {
        match b {
            0 => Some(SelectionMode::Character),
            1 => Some(SelectionMode::Line),
            2 => Some(SelectionMode::Block),
            _ => None,
        }
    }
}

// ============================================================================
// TextCursor
// ============================================================================

/// Text-domain cursor — position + optional selection, encoded as `dyn Cursor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextCursor {
    header: CursorHeader,
    line: u64,
    col: u64,
    selection: Option<SelectionData>,
    content: Vec<u8>,
}

impl TextCursor {
    /// Create a cursor without selection.
    #[must_use]
    pub fn new(domain_id: u32, line: u64, col: u64) -> Self {
        let header = CursorHeader::new(domain_id, TEXT_CURSOR_INNER_ID, 0);
        let content = Self::encode_content(line, col, None);
        Self {
            header,
            line,
            col,
            selection: None,
            content,
        }
    }

    /// Create a cursor with selection.
    #[must_use]
    pub fn with_selection(domain_id: u32, line: u64, col: u64, selection: SelectionData) -> Self {
        let header = CursorHeader::new(domain_id, TEXT_CURSOR_INNER_ID, FLAG_HAS_SELECTION);
        let content = Self::encode_content(line, col, Some(&selection));
        Self {
            header,
            line,
            col,
            selection: Some(selection),
            content,
        }
    }

    /// Cursor line (0-indexed).
    #[must_use]
    pub const fn line(&self) -> u64 {
        self.line
    }

    /// Cursor column (0-indexed).
    #[must_use]
    pub const fn col(&self) -> u64 {
        self.col
    }

    /// Selection data, if any.
    #[must_use]
    pub const fn selection(&self) -> Option<&SelectionData> {
        self.selection.as_ref()
    }

    /// Whether this cursor has an active selection.
    #[must_use]
    pub const fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    fn encode_content(line: u64, col: u64, selection: Option<&SelectionData>) -> Vec<u8> {
        let capacity = if selection.is_some() { 33 } else { 16 };
        let mut buf = Vec::with_capacity(capacity);
        buf.extend_from_slice(&line.to_le_bytes());
        buf.extend_from_slice(&col.to_le_bytes());
        if let Some(sel) = selection {
            buf.extend_from_slice(&sel.start_line.to_le_bytes());
            buf.extend_from_slice(&sel.start_col.to_le_bytes());
            buf.push(sel.mode_byte());
        }
        buf
    }

    #[allow(clippy::trivially_copy_pass_by_ref)] // matches CursorCodec::decode(&CursorHeader) signature
    fn decode_content(
        header: &CursorHeader,
        bytes: &[u8],
    ) -> Option<(u64, u64, Option<SelectionData>)> {
        if bytes.len() < 16 {
            return None;
        }
        let line = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let col = u64::from_le_bytes(bytes[8..16].try_into().ok()?);

        let has_selection = header.flags() & FLAG_HAS_SELECTION != 0;
        let selection = if has_selection {
            if bytes.len() < 33 {
                return None;
            }
            let start_line = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
            let start_col = u64::from_le_bytes(bytes[24..32].try_into().ok()?);
            let mode = SelectionData::mode_from_byte(bytes[32])?;
            Some(SelectionData {
                start_line,
                start_col,
                mode,
            })
        } else {
            None
        };

        Some((line, col, selection))
    }
}

impl Cursor for TextCursor {
    fn header(&self) -> &CursorHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.content
    }

    fn display(&self) -> String {
        self.selection.as_ref().map_or_else(
            || format!("{}:{}", self.line, self.col),
            |sel| {
                format!(
                    "{}:{} sel={}:{} mode={:?}",
                    self.line, self.col, sel.start_line, sel.start_col, sel.mode
                )
            },
        )
    }

    fn clone_box(&self) -> Box<dyn Cursor> {
        Box::new(self.clone())
    }
}

impl From<(u32, CursorPosition)> for TextCursor {
    fn from((domain_id, cp): (u32, CursorPosition)) -> Self {
        #[allow(clippy::cast_possible_truncation)] // usize→u64 widening on 64-bit; never truncates
        Self::new(domain_id, cp.line as u64, cp.column as u64)
    }
}

// ============================================================================
// Codecs
// ============================================================================

/// Codec for decoding wire bytes into `TextPosition`.
pub struct TextPositionCodec {
    domain_id: u32,
}

impl TextPositionCodec {
    /// Create a new text position codec for the given domain.
    #[must_use]
    pub const fn new(domain_id: u32) -> Self {
        Self { domain_id }
    }
}

impl PositionCodec for TextPositionCodec {
    fn domain_id(&self) -> u32 {
        self.domain_id
    }

    fn inner_id(&self) -> u16 {
        TEXT_POSITION_INNER_ID
    }

    fn decode(&self, header: &PositionHeader, content: &[u8]) -> Option<Box<dyn Position>> {
        let (line, col) = TextPosition::decode_content(content)?;
        let _ = header; // Header already validated by registry lookup
        Some(Box::new(TextPosition::new(self.domain_id, line, col)))
    }
}

/// Codec for decoding wire bytes into `TextCursor`.
pub struct TextCursorCodec {
    domain_id: u32,
}

impl TextCursorCodec {
    /// Create a new text cursor codec for the given domain.
    #[must_use]
    pub const fn new(domain_id: u32) -> Self {
        Self { domain_id }
    }
}

impl CursorCodec for TextCursorCodec {
    fn domain_id(&self) -> u32 {
        self.domain_id
    }

    fn inner_id(&self) -> u16 {
        TEXT_CURSOR_INNER_ID
    }

    fn decode(&self, header: &CursorHeader, content: &[u8]) -> Option<Box<dyn Cursor>> {
        let (line, col, selection) = TextCursor::decode_content(header, content)?;
        Some(selection.map_or_else(
            || Box::new(TextCursor::new(self.domain_id, line, col)) as Box<dyn Cursor>,
            |sel| Box::new(TextCursor::with_selection(self.domain_id, line, col, sel)),
        ))
    }
}

#[cfg(test)]
#[path = "text_cursor_tests.rs"]
mod tests;
