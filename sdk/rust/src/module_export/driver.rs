use super::{Instance, abi, c_void};
use core::task::{Context, Waker};
use reovim_kernel_interface::{
    Error,
    driver::{Lifecycle, Progress},
};

pub(super) static OPS: abi::ModuleDriver = abi::ModuleDriver {
    bytes: size_of::<abi::ModuleDriver>(),
    status: Some(status),
    advance: Some(advance),
    close: Some(close),
};

unsafe fn instance<'a>(
    pointer: *mut c_void,
) -> Result<&'a mut Instance, Error> {
    if pointer.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: Export transfers a unique Box; callbacks are serialized and
    // native destroy has not consumed its context. No reference escapes a call.
    Ok(unsafe { &mut *pointer.cast::<Instance>() })
}

unsafe extern "C" fn status(
    pointer: *mut c_void,
    output: *mut abi::ModuleStatus,
) -> u32 {
    if output.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The host supplies an owned, serialized instance context.
    let Ok(instance) = (unsafe { instance(pointer) }) else {
        return Error::INVALID.code.0;
    };
    let observed = instance.module.status();
    let phase = if instance.closed {
        Lifecycle::Closed
    } else if observed == Lifecycle::Closed {
        Lifecycle::Closing
    } else {
        observed
    };
    let (phase, failure) = match phase {
        Lifecycle::Constructed => (0, abi::Error::default()),
        Lifecycle::Starting => (1, abi::Error::default()),
        Lifecycle::Ready => (2, abi::Error::default()),
        Lifecycle::Closing => (3, abi::Error::default()),
        Lifecycle::Closed => (4, abi::Error::default()),
        Lifecycle::Failed(error) => {
            (5, crate::caller_export::native_error(error))
        }
    };
    // SAFETY: The host supplies writable status storage; passive observation
    // transfers no ownership and cannot complete cleanup by itself.
    unsafe { output.write(abi::ModuleStatus { phase, failure }) };
    0
}

unsafe extern "C" fn advance(
    pointer: *mut c_void,
    signal: abi::Wake,
    progress: *mut u32,
    error: *mut abi::Error,
) -> u32 {
    if progress.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The host retains the instance and serializes its driver calls.
    // A supplied native wake lends valid thread-safe retain/release callbacks.
    let result = unsafe { instance(pointer) }
        .and_then(|instance| unsafe { drive(instance, signal) });
    match result {
        Ok(value) => {
            let value = match value {
                Progress::More => 0,
                Progress::Idle => 1,
                Progress::Stopped => 2,
            };
            // SAFETY: Successful advance writes one scalar to host storage.
            unsafe { progress.write(value) };
            0
        }
        // SAFETY: The host supplies writable error storage for this failure.
        Err(cause) => unsafe { failure(error, cause) },
    }
}

unsafe fn drive(
    instance: &mut Instance,
    signal: abi::Wake,
) -> Result<Progress, Error> {
    if instance.closed {
        return Ok(Progress::Stopped);
    }
    let progress = if instance.module.status() == Lifecycle::Closed {
        Progress::Idle
    } else {
        let waker = if crate::caller_export::wake::absent(signal) {
            Waker::noop().clone()
        } else {
            // SAFETY: The host lends a live wake; import validates its slots
            // and retains a reference before exposing it to the Rust driver.
            unsafe { crate::caller_export::wake::import(signal)? }
        };
        instance.module.advance(&mut Context::from_waker(&waker))?
    };
    if instance.module.status() == Lifecycle::Closed {
        return match drain(instance) {
            Ok(()) => Ok(Progress::Stopped),
            Err(error) if error == Error::BUSY => Ok(Progress::Idle),
            Err(error) => Err(error),
        };
    }
    Ok(progress)
}

fn drain(instance: &mut Instance) -> Result<(), Error> {
    instance.exports = None;
    instance.imports.close()?;
    instance.closed = true;
    Ok(())
}

unsafe extern "C" fn close(
    pointer: *mut c_void,
    error: *mut abi::Error,
) -> u32 {
    if error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The host owns this serialized instance until native destruction.
    let result = unsafe { instance(pointer) }.and_then(|instance| {
        instance.exports = None;
        instance.module.close()?;
        drain(instance)
    });
    match result {
        Ok(()) => 0,
        // SAFETY: The host supplies writable error storage; failed close keeps
        // instance ownership and never authorizes native destruction.
        Err(cause) => unsafe { failure(error, cause) },
    }
}

pub(super) unsafe fn failure(output: *mut abi::Error, error: Error) -> u32 {
    let error = crate::caller_export::native_error(error);
    // SAFETY: The caller checked writable error storage before dispatch.
    unsafe { output.write(error) };
    error.code
}
