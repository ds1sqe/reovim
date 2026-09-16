#![no_std]
#![forbid(unsafe_code)]
//! Immutable memory creation through ordinary module activation and factories.

extern crate alloc;

mod memory;

use alloc::{boxed::Box, rc::Rc, vec};
use core::task::{Context, Poll};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
    caller::Caller,
    declaration::{Declaration, Operation},
    driver::{DriverOps, Lifecycle, Progress},
    module::{ModuleContext, ModuleImplementation},
    provider::{
        Creation, ProviderInterface, Registrar, ResourceOps, ResourceProvider,
    },
};

struct State {
    phase: Lifecycle,
    exports: Registrar,
}

/// Supplies an inert module whose first driven turn stages `create`.
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
    if let Err((error, mut provider)) = state.exports.stage(b"create", provider)
    {
        provider.stop()?;
        state.phase = Lifecycle::Failed(error);
        return Err(error);
    }
    state.phase = Lifecycle::Ready;
    Ok(Progress::Idle)
}

fn declaration() -> Result<Declaration, Error> {
    let mut bytes = vec![0xc2; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    // exec create: factory dispatch, EXEC access, WRITE activity.
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 2, 4, 0, 0, 0, 2, 6, 0]);
    bytes.extend_from_slice(b"create");
    // Bytes bounded to 4096, Unit result/progress.
    bytes.push(7);
    bytes.extend_from_slice(&4096_u32.to_le_bytes());
    // Primary Ref with no API restriction.
    bytes.extend_from_slice(&[0, 0, 12]);
    bytes.extend_from_slice(&[0; 16]);
    // Created access ceiling READ.
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    Declaration::decode(&bytes)
}

fn provider() -> Result<ResourceProvider, Error> {
    ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration()?,
            begin: None,
            create: Some(create),
        }]),
        Rc::new(()),
    )
    .map_err(|(error, ())| error)
}

fn create(
    (): &mut (),
    _: usize,
    operation: Operation,
    input: &Value,
    creation: &Creation,
) -> Result<Pending, Error> {
    if operation != Operation::Exec || input.data.len() < 4 {
        return Err(Error::INVALID);
    }
    let count = u32::from_le_bytes(input.data[..4].try_into().unwrap());
    if count > 4096 || usize::try_from(count).ok() != Some(input.data.len() - 4)
    {
        return Err(Error::INVALID);
    }
    let memory = memory::provider(&input.data[4..])?;
    if let Err((error, mut memory)) = creation.resource(memory) {
        memory.stop()?;
        return Err(error);
    }
    Ok(Pending::new(
        Some(Completion {
            outcome: Outcome::Ok(Value::unit()),
            activity: 2,
            owner: None,
        }),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |completion| completion.take().ok_or(Error::CLOSED),
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    ))
}
