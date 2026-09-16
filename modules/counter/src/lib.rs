#![no_std]
#![forbid(unsafe_code)]
//! A non-byte Resource service, staged through ordinary module activation.

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec};
use core::{
    cell::Cell,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
    caller::Caller,
    declaration::{Declaration, Operation},
    driver::{DriverOps, Lifecycle, Progress},
    module::{ModuleContext, ModuleImplementation},
    provider::{ProviderInterface, Registrar, ResourceOps, ResourceProvider},
};

struct State {
    phase: Lifecycle,
    exports: Registrar,
}

/// Supplies an inert module whose first driven turn stages `value`.
#[must_use]
pub fn implementation() -> ModuleImplementation<impl Sized, Caller> {
    ModuleImplementation {
        construct,
        driver: DriverOps {
            status: |state| state.phase,
            advance,
            close: |state| {
                state.phase = Lifecycle::Closed;
                Ok(())
            },
        },
    }
}

#[allow(
    clippy::result_large_err,
    clippy::unnecessary_wraps,
    reason = "the module constructor ABI returns unchanged context on rejection"
)]
fn construct(
    context: ModuleContext<Caller>,
) -> Result<State, (ModuleContext<Caller>, Error)> {
    Ok(State { phase: Lifecycle::Constructed, exports: context.exports })
}

fn advance(state: &mut State, _: &mut Context<'_>) -> Result<Progress, Error> {
    if state.phase == Lifecycle::Closed {
        return Ok(Progress::Stopped);
    }
    if state.phase == Lifecycle::Ready {
        return Ok(Progress::Idle);
    }
    state.phase = Lifecycle::Starting;
    let provider = provider()?;
    if let Err((error, mut provider)) = state.exports.stage(b"value", provider)
    {
        provider.stop()?;
        state.phase = Lifecycle::Failed(error);
        return Err(error);
    }
    state.phase = Lifecycle::Ready;
    Ok(Progress::Idle)
}

fn declaration() -> Result<Declaration, Error> {
    let mut bytes = vec![0xc1; 16];
    // Two ordinary methods: exec add(u64)->u64 with EXEC/WRITE activity,
    // and ioctl 1 read(unit)->u64 with READ access/activity. No byte I/O.
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&[
        1, 0, 0, 0, 0, 0, 4, 0, 0, 0, 2, 3, 0, b'a', b'd', b'd', 4, 4, 0,
    ]);
    bytes.extend_from_slice(&[
        2, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 4, 0, b'r', b'e', b'a', b'd', 0, 4, 0,
    ]);
    Declaration::decode(&bytes)
}

fn provider() -> Result<ResourceProvider, Error> {
    ResourceProvider::new(
        Cell::new(0_u64),
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration()?,
            begin: Some(begin),
            create: None,
        }]),
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
}

fn begin(
    state: &mut Cell<u64>,
    _: usize,
    operation: Operation,
    input: &Value,
) -> Result<Pending, Error> {
    let (next, activity) = match operation {
        Operation::Exec => {
            let amount = u64::from_le_bytes(
                input.data.as_ref().try_into().map_err(|_| Error::INVALID)?,
            );
            (state.get().checked_add(amount).ok_or(Error::PRESSURE)?, 2)
        }
        Operation::Ioctl(1) if input.data.is_empty() => (state.get(), 1),
        Operation::Ioctl(_) => return Err(Error::UNSUPPORTED),
    };
    let value = Value::copy(&next.to_le_bytes(), &[])?;
    state.set(next);
    Ok(Pending::new(
        Some(Completion { outcome: Outcome::Ok(value), activity, owner: None }),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |value| value.take().ok_or(Error::CLOSED),
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::task::Waker;

    fn result(pending: &mut Pending) -> u64 {
        assert_eq!(
            pending.poll(&mut Context::from_waker(Waker::noop())),
            Ok(Poll::Ready(()))
        );
        let completion = pending.take().unwrap();
        let Outcome::Ok(value) = &completion.outcome else {
            panic!("no value")
        };
        let result =
            u64::from_le_bytes(value.data.as_ref().try_into().unwrap());
        drop(completion);
        pending.finish().unwrap();
        result
    }

    #[test]
    fn invalid_add_and_overflow_preserve_the_counter() {
        let mut provider = provider().unwrap();
        assert!(provider.io().is_none());
        let maximum = Value::copy(&u64::MAX.to_le_bytes(), &[]).unwrap();
        let mut pending = provider.begin(0, Operation::Exec, &maximum).unwrap();
        assert_eq!(result(&mut pending), u64::MAX);
        let one = Value::copy(&1_u64.to_le_bytes(), &[]).unwrap();
        assert!(matches!(
            provider.begin(0, Operation::Exec, &one),
            Err(Error::PRESSURE)
        ));
        assert!(matches!(
            provider.begin(0, Operation::Exec, &Value::unit()),
            Err(Error::INVALID)
        ));
        let mut read =
            provider.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap();
        assert_eq!(result(&mut read), u64::MAX);
        provider.stop().unwrap();
    }
}
