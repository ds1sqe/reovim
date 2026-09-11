use core::task::Context;
use reovim_kernel_interface::{
    Error,
    driver::{DriverOps, Lifecycle, Progress},
};

use crate::Runtime;

impl Runtime {
    /// Assembly supplies this table only to the Runtime's execution owner.
    pub const DRIVER: DriverOps<Self> = DriverOps { status, advance, close };
}

fn status(runtime: &Runtime) -> Lifecycle {
    let shared = runtime.shared.borrow();
    if shared.entries.is_empty() {
        Lifecycle::Closed
    } else if shared.closing {
        Lifecycle::Closing
    } else if shared.ready {
        Lifecycle::Ready
    } else {
        Lifecycle::Constructed
    }
}

fn advance(
    runtime: &mut Runtime,
    context: &mut Context<'_>,
) -> Result<Progress, Error> {
    match status(runtime) {
        Lifecycle::Constructed => {
            runtime.start()?;
            Ok(Progress::More)
        }
        Lifecycle::Closed => Ok(Progress::Stopped),
        Lifecycle::Closing => closing(runtime, context),
        _ => runtime.advance(context),
    }
}

fn closing(
    runtime: &mut Runtime,
    context: &mut Context<'_>,
) -> Result<Progress, Error> {
    let progress = runtime.advance(context)?;
    match runtime.close() {
        Ok(()) => Ok(Progress::Stopped),
        Err(error) if error == Error::BUSY => Ok(progress),
        Err(error) => Err(error),
    }
}

fn close(runtime: &mut Runtime) -> Result<(), Error> {
    runtime.close()
}
