//! Private immutable byte storage behind the module factory.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::task::Poll;
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
    provider::{IoInfo, IoOps, Read, ResourceOps, ResourceProvider},
};

/// Copies an immutable byte source with positional READ support.
/// # Errors
/// Allocation pressure leaves no registered Resource or provider obligations.
pub fn provider(data: &[u8]) -> Result<ResourceProvider, Error> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(data.len()).map_err(|_| Error::PRESSURE)?;
    bytes.extend_from_slice(data);
    ResourceProvider::new(
        bytes.into_boxed_slice(),
        ResourceOps {
            open: None,
            namespace: None,
            io: Some(IoOps {
                receive: None,
                send: None,
                info: IoInfo {
                    read_modes: 2,
                    write_modes: 0,
                    alignment: 1,
                    transfer_limit: u64::from(u32::MAX),
                },
                read: Some(|data, request| read_memory(data, request)),
                write: None,
            }),
            stop: |_| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
}

fn read_memory(data: &[u8], request: Read) -> Result<Pending, Error> {
    let offset = request.offset.ok_or(Error::UNSUPPORTED)?;
    let start = usize::try_from(offset).unwrap_or(usize::MAX).min(data.len());
    let count = usize::try_from(request.maximum)
        .unwrap_or(usize::MAX)
        .min(data.len() - start);
    let size = count.checked_add(5).ok_or(Error::PRESSURE)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    let length = u32::try_from(count).map_err(|_| Error::PRESSURE)?;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&data[start..start + count]);
    bytes.push(u8::from(request.maximum != 0 && start + count == data.len()));
    let value =
        Value { data: bytes.into_boxed_slice(), references: Box::new([]) };
    let completion = Completion {
        outcome: Outcome::Ok(value),
        activity: u8::from(request.maximum != 0),
        owner: None,
    };
    Ok(Pending::new(
        Some(completion),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |value| value.take().ok_or(Error::CLOSED),
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    ))
}
