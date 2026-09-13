#![allow(clippy::future_not_send)] // Connections use their owning executor.
use alloc::vec::Vec;
use core::ops::Range;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, ResourceId,
};

pub struct Backing {
    connection: Connection,
    resource: ResourceId,
    pub length: u64,
}
impl Backing {
    pub async fn open(
        connection: Connection,
        resource: ResourceId,
    ) -> Result<Self, Error> {
        let connection = connection.read_view(resource)?;
        let resource =
            connection.root().await.map_err(|error| failure(&error))?;
        let input = Message::new(Contract::new("resource.snapshot", 1), &[]);
        let value = connection
            .read(resource, &input)
            .await
            .map_err(|error| failure(&error))?;
        if value.contract != input.contract || !value.resources.is_empty() {
            return Err(Error::PROVIDER);
        }
        let bytes =
            value.data.as_ref().try_into().map_err(|_| Error::PROVIDER)?;
        Ok(Self { connection, resource, length: u64::from_le_bytes(bytes) })
    }
    pub async fn read(
        &self,
        range: Range<u64>,
        output: &mut Vec<u8>,
    ) -> Result<(), Error> {
        let mut start = range.start;
        while start < range.end {
            let end = start.saturating_add(32768).min(range.end);
            let input = Message::new(
                Contract::new("resource.snapshot", 1),
                &super::api::encode_range(&(start..end)),
            );
            let value = self
                .connection
                .read(self.resource, &input)
                .await
                .map_err(|error| failure(&error))?;
            validate(&value, &input, end - start)?;
            output
                .try_reserve(value.data.len())
                .map_err(|_| Error::PRESSURE)?;
            output.extend_from_slice(&value.data);
            start = end;
        }
        Ok(())
    }
}

fn validate(value: &Message, input: &Message, size: u64) -> Result<(), Error> {
    if value.contract != input.contract
        || !value.resources.is_empty()
        || value.data.len() as u64 != size
    {
        return Err(Error::PROVIDER);
    }
    Ok(())
}
const fn failure(error: &reovim_kernel_interface::RequestError) -> Error {
    use reovim_kernel_interface::RequestError;
    match error {
        RequestError::Rejected(error) | RequestError::Failed(error) => {
            Error::new(error.code.0)
        }
        RequestError::Unknown { .. } => Error::PROVIDER,
    }
}
