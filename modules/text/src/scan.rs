//! Bounded source reads; an incomplete UTF-8 prefix is carried, not replaced.
#![allow(clippy::future_not_send)]

use crate::api::Unit;
use alloc::vec::Vec;
use reovim_kernel_interface::{
    Connection, Error, RequestError, buffer::RetainedState,
};
use reovim_module_source::api;

pub struct Scan<'a> {
    kernel: &'a Connection,
    source: &'a RetainedState,
    length: u64,
    bytes: Vec<u8>,
    index: usize,
    pub offset: u64,
}

impl<'a> Scan<'a> {
    pub const fn new(
        kernel: &'a Connection,
        source: &'a RetainedState,
        length: u64,
    ) -> Self {
        Self { kernel, source, length, bytes: Vec::new(), index: 0, offset: 0 }
    }

    pub async fn next(&mut self) -> Result<Option<Unit>, RequestError> {
        if self.offset == self.length {
            return Ok(None);
        }
        if self.bytes.len() - self.index < 4 {
            self.fill().await?;
        }
        let (unit, consumed) = unit(&self.bytes[self.index..])?;
        self.index += consumed;
        self.offset += consumed as u64;
        Ok(Some(unit))
    }

    async fn fill(&mut self) -> Result<(), RequestError> {
        let mut data = self.bytes[self.index..].to_vec();
        let start = self.offset + data.len() as u64;
        let end = start.saturating_add(4096).min(self.length);
        if start < end {
            let window =
                api::read(self.kernel, self.source, start..end).await?;
            data.extend_from_slice(window.bytes());
        }
        self.bytes = data;
        self.index = 0;
        Ok(())
    }
}

fn unit(bytes: &[u8]) -> Result<(Unit, usize), Error> {
    let first = *bytes.first().ok_or(Error::PROVIDER)?;
    let width = match first {
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 1,
    };
    let character = bytes
        .get(..width)
        .and_then(|data| core::str::from_utf8(data).ok())
        .and_then(|text| text.chars().next());
    Ok(character.map_or((Unit::Opaque(first), 1), |c| (Unit::Scalar(c), width)))
}
