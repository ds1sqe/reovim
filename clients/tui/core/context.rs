//! Snapshot-bound grapheme context over bounded Text reads.

use alloc::{string::String, vec, vec::Vec};
use reovim_module_text::api::{Observation, Unit, Window as SourceWindow};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

/// An immutable Text window whose grapheme edges have been established.
pub struct Window {
    source: SourceWindow,
    column: Option<usize>,
}

impl Window {
    /// A complete document already supplies all grapheme context.
    /// # Errors
    /// Rejects partial or inconsistent source windows.
    pub fn complete(
        source: SourceWindow,
    ) -> Result<Self, Error<core::convert::Infallible>> {
        if source.range.start != 0
            || !source.eof
            || source.range.end != source.units.len() as u64
        {
            return Err(Error::InvalidChunk);
        }
        Ok(Self { source, column: Some(0) })
    }

    /// Display-column phase at the window origin for four-cell tab stops.
    /// None means an earlier oversized grapheme prevents exact tab placement.
    #[must_use]
    pub const fn column(&self) -> Option<usize> {
        self.column
    }
}

impl core::ops::Deref for Window {
    type Target = SourceWindow;
    fn deref(&self) -> &SourceWindow {
        &self.source
    }
}

#[derive(Debug)]
pub enum Error<E> {
    Read(E),
    InvalidChunk,
    Position,
    /// A selected grapheme exceeds the 1024-unit view capacity.
    GraphemeTooLarge,
}

impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Read(error) => write!(out, "Text context read: {error}"),
            Self::InvalidChunk => out.write_str("invalid Text context chunk"),
            Self::Position => {
                out.write_str("Text context position unavailable")
            }
            Self::GraphemeTooLarge => {
                out.write_str("grapheme exceeds view capacity")
            }
        }
    }
}
impl<E: core::error::Error + 'static> core::error::Error for Error<E> {}

struct Chunk {
    window: SourceWindow,
    text: String,
    bytes: usize,
    offsets: Vec<usize>,
}

impl Chunk {
    fn new<E>(window: SourceWindow, bytes: usize) -> Result<Self, Error<E>> {
        let mut text = String::new();
        let mut offsets = vec![0];
        for unit in &window.units {
            // One opaque source byte breaks graphemes on both sides.
            text.push(match unit {
                Unit::Scalar(ch) => *ch,
                Unit::Opaque(_) => '\0',
            });
            offsets.push(text.len());
        }
        bytes.checked_add(text.len()).ok_or(Error::Position)?;
        Ok(Self { window, text, bytes, offsets })
    }

    fn position<E>(&self, bytes: usize) -> Result<u64, Error<E>> {
        let offset =
            bytes.checked_sub(self.bytes).ok_or(Error::InvalidChunk)?;
        let index = self
            .offsets
            .binary_search(&offset)
            .map_err(|_| Error::InvalidChunk)?;
        self.window.range.start.checked_add(index as u64).ok_or(Error::Position)
    }
}

struct Scan {
    cursor: GraphemeCursor,
    chunk: Chunk,
    context: Option<(u64, usize)>,
    byte_len: usize,
}

enum Next {
    Boundary(u64, usize),
    End,
    BeyondView,
}

fn next<E>(
    scan: &mut Scan,
    observed: &Observation,
    read: &mut impl FnMut(u64, u64) -> Result<SourceWindow, E>,
    wanted: u64,
    start: u64,
) -> Result<Next, Error<E>> {
    loop {
        match scan.cursor.next_boundary(&scan.chunk.text, scan.chunk.bytes) {
            Ok(Some(bytes)) => {
                return Ok(Next::Boundary(scan.chunk.position(bytes)?, bytes));
            }
            Ok(None) => return Ok(Next::End),
            Err(GraphemeIncomplete::NextChunk)
                if scan.chunk.window.range.end > wanted
                    && scan.chunk.window.range.end - start > 1024 =>
            {
                return Ok(Next::BeyondView);
            }
            Err(GraphemeIncomplete::NextChunk) => {
                scan.advance(observed, read)?;
            }
            Err(GraphemeIncomplete::PreContext(end)) => {
                scan.preceding(end, observed, read)?;
            }
            Err(_) => return Err(Error::InvalidChunk),
        }
    }
}

impl Scan {
    fn column<E>(
        &self,
        observed: &Observation,
        start: u64,
        end: u64,
        phase: Option<usize>,
        read: &mut impl FnMut(u64, u64) -> Result<SourceWindow, E>,
    ) -> Result<Option<usize>, Error<E>> {
        if end - start > 1024 {
            return Ok(None);
        }
        if start >= self.chunk.window.range.start
            && end <= self.chunk.window.range.end
        {
            let first = usize::try_from(start - self.chunk.window.range.start)
                .map_err(|_| Error::Position)?;
            let last = usize::try_from(end - self.chunk.window.range.start)
                .map_err(|_| Error::Position)?;
            return Ok(crate::viewport::column_after(
                &self.chunk.window.units[first..last],
                phase,
            ));
        }
        let value = fetch(observed, start, end - start, read)?;
        if value.range.end != end {
            return Err(Error::InvalidChunk);
        }
        Ok(crate::viewport::column_after(&value.units, phase))
    }
    fn advance<E>(
        &mut self,
        observed: &Observation,
        read: &mut impl FnMut(u64, u64) -> Result<SourceWindow, E>,
    ) -> Result<(), Error<E>> {
        if self.chunk.window.eof {
            return Err(Error::InvalidChunk);
        }
        let bytes = self.chunk.bytes + self.chunk.text.len();
        let window = fetch(observed, self.chunk.window.range.end, 1024, read)?;
        self.chunk = Chunk::new(window, bytes)?;
        self.context = None;
        check_length(&self.chunk, self.byte_len)
    }

    fn preceding<E>(
        &mut self,
        end: usize,
        observed: &Observation,
        read: &mut impl FnMut(u64, u64) -> Result<SourceWindow, E>,
    ) -> Result<(), Error<E>> {
        let scalar_end = if end == self.chunk.bytes {
            self.chunk.window.range.start
        } else {
            self.context
                .filter(|(_, bytes)| *bytes == end)
                .ok_or(Error::InvalidChunk)?
                .0
        };
        let start = scalar_end.saturating_sub(1024);
        if start == scalar_end {
            return Err(Error::InvalidChunk);
        }
        let window = fetch(observed, start, scalar_end - start, read)?;
        if window.range.end != scalar_end {
            return Err(Error::InvalidChunk);
        }
        let mut chunk = Chunk::new(window, 0)?;
        chunk.bytes =
            end.checked_sub(chunk.text.len()).ok_or(Error::InvalidChunk)?;
        self.cursor.provide_context(&chunk.text, chunk.bytes);
        self.context = Some((start, chunk.bytes));
        Ok(())
    }
}

/// Finds complete grapheme edges around a requested scalar position.
///
/// Every read uses the same retained observation. Prefix scanning retains only
/// bounded chunks; it does not copy the document. Returned units remain exact.
/// # Errors
/// Propagates read failures and rejects foreign chunks, missing positions or a
/// selected grapheme too large for the view. No failure becomes empty content.
pub fn read<E>(
    observed: &Observation,
    wanted: u64,
    mut read: impl FnMut(u64, u64) -> Result<SourceWindow, E>,
) -> Result<Window, Error<E>> {
    let byte_len =
        usize::try_from(observed.byte_len()).map_err(|_| Error::Position)?;
    let chunk = Chunk::new(fetch(observed, 0, 1024, &mut read)?, 0)?;
    check_length(&chunk, byte_len)?;
    let mut scan = Scan {
        cursor: GraphemeCursor::new(0, byte_len, true),
        chunk,
        context: None,
        byte_len,
    };
    let mut start = 0;
    let mut end = 0;
    let mut end_bytes = 0;
    let mut column = Some(0);
    while let Next::Boundary(position, bytes) =
        next(&mut scan, observed, &mut read, wanted, start)?
    {
        if position <= wanted {
            column =
                scan.column(observed, start, position, column, &mut read)?;
            start = position;
            end = position;
            end_bytes = bytes;
            continue;
        }
        if position - start > 1024 {
            break;
        }
        end = position;
        end_bytes = bytes;
        if end - start == 1024 {
            break;
        }
    }
    if wanted > scan.chunk.window.range.end && scan.chunk.window.eof {
        return Err(Error::Position);
    }
    if start == end && end_bytes != byte_len {
        return Err(Error::GraphemeTooLarge);
    }
    let result = fetch(observed, start, end - start, &mut read)?;
    if result.range.end != end || result.eof != (end_bytes == byte_len) {
        return Err(Error::InvalidChunk);
    }
    Ok(Window { source: result, column })
}

fn fetch<E>(
    observed: &Observation,
    start: u64,
    count: u64,
    read: &mut impl FnMut(u64, u64) -> Result<SourceWindow, E>,
) -> Result<SourceWindow, Error<E>> {
    let value = read(start, count).map_err(Error::Read)?;
    if value.snapshot != observed.snapshot()
        || value.observed != observed.observed()
        || value.range.start != start
        || value.units.len() as u64 > count
        || value.range.end.checked_sub(start) != Some(value.units.len() as u64)
        || (count != 0 && value.units.is_empty() && !value.eof)
    {
        return Err(Error::InvalidChunk);
    }
    Ok(value)
}

const fn check_length<E>(
    chunk: &Chunk,
    expected: usize,
) -> Result<(), Error<E>> {
    let end = chunk.bytes + chunk.text.len();
    if end > expected || (chunk.window.eof && end != expected) {
        return Err(Error::InvalidChunk);
    }
    Ok(())
}
