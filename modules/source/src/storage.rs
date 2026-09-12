use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::ops::Range;
use reovim_kernel_interface::Error;

#[derive(Clone)]
struct Piece {
    bytes: Rc<[u8]>,
    range: Range<usize>,
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
            pieces.push(Piece { range: 0..bytes.len(), bytes: bytes.into() });
        }
        Ok(Self { pieces, length })
    }

    pub const fn len(&self) -> u64 {
        self.length
    }

    pub fn cost(&self) -> Result<u64, Error> {
        self.pieces.iter().try_fold(
            (self.pieces.capacity() * core::mem::size_of::<Piece>()) as u64,
            |cost, piece| {
                cost.checked_add(piece.bytes.len() as u64)
                    .ok_or(Error::PRESSURE)
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
        visit(&self.pieces, range, |piece, selected| {
            output.extend_from_slice(&piece.bytes[selected]);
        });
        Ok(())
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
            pieces.push(Piece { bytes: bytes.into(), range: 0..bytes.len() });
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

fn visit(
    pieces: &[Piece],
    range: Range<u64>,
    mut use_piece: impl FnMut(&Piece, Range<usize>),
) {
    let mut offset = 0;
    for piece in pieces {
        let end = offset + (piece.range.end - piece.range.start) as u64;
        let start = range.start.max(offset);
        let stop = range.end.min(end);
        if start < stop {
            // Both offsets were clipped to this resident allocation.
            let first = usize::try_from(start - offset).expect("piece offset");
            let last = usize::try_from(stop - offset).expect("piece offset");
            let selected = piece.range.start + first..piece.range.start + last;
            use_piece(piece, selected);
        }
        offset = end;
        if offset >= range.end {
            break;
        }
    }
}
