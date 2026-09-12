#![allow(clippy::future_not_send)] // State and Connection use one executor.
use super::backing::Backing;
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::ops::Range;
use reovim_kernel_interface::Error;

#[derive(Clone)]
enum Data {
    Resident(Rc<[u8]>),
    Backing(Rc<Backing>),
}

#[derive(Clone)]
struct Piece {
    bytes: Data,
    range: Range<u64>,
}

pub struct Source {
    pieces: Vec<Piece>,
    length: u64,
}

impl Source {
    pub fn new(bytes: Box<[u8]>) -> Result<Self, Error> {
        let length = bytes.len() as u64;
        let mut pieces = Vec::new();
        if !bytes.is_empty() {
            pieces.try_reserve_exact(1).map_err(|_| Error::PRESSURE)?;
            pieces.push(Piece {
                range: 0..length,
                bytes: Data::Resident(bytes.into()),
            });
        }
        Ok(Self { pieces, length })
    }

    pub const fn len(&self) -> u64 {
        self.length
    }

    pub fn backed(backing: Backing) -> Self {
        let length = backing.length;
        Self {
            pieces: alloc::vec![Piece {
                range: 0..length,
                bytes: Data::Backing(Rc::new(backing)),
            }],
            length,
        }
    }

    pub async fn read_async(
        &self,
        range: Range<u64>,
        output: &mut Vec<u8>,
    ) -> Result<(), Error> {
        self.validate(&range)?;
        let mut pieces = self.pieces.iter();
        let mut offset = 0;
        while offset < range.end {
            let piece = pieces.next().ok_or(Error::PROVIDER)?;
            let end = offset + (piece.range.end - piece.range.start);
            append_selected(piece, offset, &range, output).await?;
            offset = end;
        }
        Ok(())
    }

    pub fn cost(&self) -> Result<u64, Error> {
        self.pieces.iter().try_fold(
            (self.pieces.capacity() * core::mem::size_of::<Piece>()) as u64,
            |cost, piece| {
                let bytes = match &piece.bytes {
                    Data::Resident(bytes) => bytes.len() as u64,
                    Data::Backing(_) => core::mem::size_of::<Backing>() as u64,
                };
                cost.checked_add(bytes).ok_or(Error::PRESSURE)
            },
        )
    }

    pub fn read(
        &self,
        range: Range<u64>,
        output: &mut Vec<u8>,
    ) -> Result<(), Error> {
        self.validate(&range)?;
        let size = usize::try_from(range.end - range.start)
            .map_err(|_| Error::PRESSURE)?;
        output.try_reserve(size).map_err(|_| Error::PRESSURE)?;
        let mut ready = true;
        visit(&self.pieces, range, |piece, selected| match &piece.bytes {
            Data::Resident(bytes) => output.extend_from_slice(
                &bytes[usize::try_from(selected.start).expect("resident range")
                    ..usize::try_from(selected.end).expect("resident range")],
            ),
            Data::Backing(_) => ready = false,
        });
        if ready { Ok(()) } else { Err(Error::NOT_READY) }
    }

    pub fn splice(
        &self,
        range: Range<u64>,
        bytes: &[u8],
    ) -> Result<Self, Error> {
        self.validate(&range)?;
        let length = self
            .length
            .checked_sub(range.end - range.start)
            .and_then(|length| length.checked_add(bytes.len() as u64))
            .ok_or(Error::PRESSURE)?;
        let mut pieces = Vec::new();
        let capacity =
            self.pieces.len().checked_add(2).ok_or(Error::PRESSURE)?;
        pieces.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
        self.copy_pieces(0..range.start, &mut pieces);
        if !bytes.is_empty() {
            pieces.push(Piece {
                bytes: Data::Resident(bytes.into()),
                range: 0..bytes.len() as u64,
            });
        }
        self.copy_pieces(range.end..self.length, &mut pieces);
        Ok(Self { pieces, length })
    }

    const fn validate(&self, range: &Range<u64>) -> Result<(), Error> {
        if range.start > range.end {
            return Err(Error::INVALID);
        }
        if range.end > self.length {
            return Err(Error::INVALID);
        }
        Ok(())
    }

    fn copy_pieces(&self, range: Range<u64>, output: &mut Vec<Piece>) {
        visit(&self.pieces, range, |piece, range| {
            output.push(Piece { bytes: piece.bytes.clone(), range });
        });
    }
}

fn visit<'a>(
    pieces: &'a [Piece],
    range: Range<u64>,
    mut use_piece: impl FnMut(&'a Piece, Range<u64>),
) {
    let mut offset = 0;
    for piece in pieces {
        let end = offset + (piece.range.end - piece.range.start);
        let start = range.start.max(offset);
        let stop = range.end.min(end);
        if start < stop {
            let first = start - offset;
            let last = stop - offset;
            let selected = piece.range.start + first..piece.range.start + last;
            use_piece(piece, selected);
        }
        offset = end;
        if offset >= range.end {
            break;
        }
    }
}

async fn append(
    piece: &Piece,
    range: Range<u64>,
    output: &mut Vec<u8>,
) -> Result<(), Error> {
    match &piece.bytes {
        Data::Resident(bytes) => {
            let start =
                usize::try_from(range.start).map_err(|_| Error::PROVIDER)?;
            let end =
                usize::try_from(range.end).map_err(|_| Error::PROVIDER)?;
            output.try_reserve(end - start).map_err(|_| Error::PRESSURE)?;
            output.extend_from_slice(&bytes[start..end]);
        }
        Data::Backing(backing) => backing.read(range, output).await?,
    }
    Ok(())
}

async fn append_selected(
    piece: &Piece,
    offset: u64,
    range: &Range<u64>,
    output: &mut Vec<u8>,
) -> Result<(), Error> {
    let start = range.start.max(offset);
    let end = range.end.min(offset + (piece.range.end - piece.range.start));
    if start >= end {
        return Ok(());
    }
    let selected = piece.range.start + (start - offset)
        ..piece.range.start + (end - offset);
    append(piece, selected, output).await
}
