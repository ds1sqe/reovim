//! Byte and packet transfer through the selected Resource implementation.

use crate::{Error, Pending};

/// Independently supported byte directions and their admission limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IoInfo {
    /// Sequential=1, positional=2; zero means no byte reads.
    pub read_modes: u32,
    /// Independent sequential/positional write support.
    pub write_modes: u32,
    /// Required offset and transfer alignment, at least one byte.
    pub alignment: u64,
    /// Maximum bytes in one admitted transfer; nonzero.
    pub transfer_limit: u64,
}

/// A sequential or explicitly positioned read, e.g. offset Some(8), maximum 4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Read {
    /// None selects the implementation's sequential interface.
    pub offset: Option<u64>,
    /// Maximum returned bytes; zero establishes no EOF.
    pub maximum: u64,
}

/// Borrowed write bytes; pending implementations capture them before returning.
pub struct Write<'a> {
    /// None selects sequential writing; Some selects an explicit byte offset.
    pub offset: Option<u64>,
    /// Bytes whose accepted prefix is returned as canonical U64.
    pub data: &'a [u8],
}

/// Read entry paired with its private provider state.
pub type ReadFn<State> = fn(&mut State, Read) -> Result<Pending, Error>;
/// Write entry whose borrowed input lasts only through the callback.
pub type WriteFn<State> = fn(&mut State, Write<'_>) -> Result<Pending, Error>;

/// Typed transfer callbacks; absent byte directions have zero mode masks.
pub struct IoOps<State> {
    /// Validated support and limits, copied before provider execution.
    pub info: IoInfo,
    /// Optional packet receive; e.g. a queue with no byte stream interface.
    pub receive: Option<super::ReceiveOps<State>>,
    /// Optional packet send; its Type owns all protocol-specific metadata.
    pub send: Option<super::SendOps<State>>,
    /// Returns canonical {data:Bytes, eof:Bool} through pending custody.
    pub read: Option<ReadFn<State>>,
    /// Returns canonical U64 accepted bytes through ordinary pending custody.
    pub write: Option<WriteFn<State>>,
}

impl<State> IoOps<State> {
    pub(super) const fn validate(&self) -> Result<(), Error> {
        if self.info.alignment == 0
            || self.info.transfer_limit == 0
            || (self.info.read_modes | self.info.write_modes) & !3 != 0
            || (self.info.read_modes != 0) != self.read.is_some()
            || (self.info.write_modes != 0) != self.write.is_some()
        {
            return Err(Error::INVALID);
        }
        Ok(())
    }
}

impl IoInfo {
    /// Validates the selected read mode, alignment and transfer bound.
    /// # Errors
    /// Unsupported directions reject before callbacks; malformed alignment
    /// and oversized requests return Invalid and Pressure respectively.
    pub const fn read(&self, request: Read) -> Result<(), Error> {
        self.check(self.read_modes, request.offset, request.maximum)
    }

    /// Validates the selected write mode and captured byte extent.
    /// # Errors
    /// Unsupported, Invalid and Pressure leave provider state untouched.
    pub fn write(&self, request: &Write<'_>) -> Result<(), Error> {
        let size =
            u64::try_from(request.data.len()).map_err(|_| Error::PRESSURE)?;
        self.check(self.write_modes, request.offset, size)
    }

    const fn check(
        &self,
        modes: u32,
        offset: Option<u64>,
        size: u64,
    ) -> Result<(), Error> {
        let mode = if offset.is_some() { 2 } else { 1 };
        if modes & mode == 0 {
            return Err(Error::UNSUPPORTED);
        }
        if self.alignment == 0 {
            return Err(Error::INVALID);
        }
        let offset = match offset {
            Some(offset) => offset,
            None => 0,
        };
        if !offset.is_multiple_of(self.alignment)
            || !size.is_multiple_of(self.alignment)
            || offset.checked_add(size).is_none()
        {
            return Err(Error::INVALID);
        }
        if size > self.transfer_limit {
            return Err(Error::PRESSURE);
        }
        Ok(())
    }
}
