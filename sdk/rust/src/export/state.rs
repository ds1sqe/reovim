use alloc::boxed::Box;
use core::{
    ffi::c_void,
    ptr,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, abi,
    buffer::{State, StateReply},
};

struct Preparing {
    future: Option<StateReply>,
    result: Option<Result<State, Error>>,
}

/// Exports a prepared state with explicit one-time ownership transfer.
#[must_use]
pub fn export_state_reply(future: StateReply) -> abi::StatePending {
    abi::StatePending {
        context: Box::into_raw(Box::new(Preparing {
            future: Some(future),
            result: None,
        }))
        .cast(),
        poll,
        take,
        release,
    }
}

unsafe extern "C" fn poll(
    context: *mut c_void,
    wake: abi::Wake,
) -> abi::StatePoll {
    let value = unsafe { &mut *context.cast::<Preparing>() };
    if let Some(future) = value.future.as_mut() {
        let waker = unsafe { super::wake::import(wake) };
        if let Poll::Ready(result) =
            future.as_mut().poll(&mut Context::from_waker(&waker))
        {
            value.result = Some(result);
            value.future = None;
        }
    }
    match &value.result {
        Some(Ok(_)) => abi::StatePoll { status: 1, error: 0 },
        Some(Err(error)) => abi::StatePoll { status: 2, error: error.code.0 },
        None if value.future.is_some() => {
            abi::StatePoll { status: 0, error: 0 }
        }
        None => abi::StatePoll { status: 2, error: Error::CLOSED.code.0 },
    }
}

unsafe extern "C" fn take(context: *mut c_void) -> abi::Reply {
    let value = unsafe { &mut *context.cast::<Preparing>() };
    let mut reply = abi::Reply {
        error: 0,
        data: abi::Bytes { data: ptr::null(), len: 0 },
        state: ptr::null_mut(),
        state_bytes: 0,
    };
    match value.result.take() {
        Some(Ok(state)) => {
            reply.state_bytes = state.cost;
            reply.state = Box::into_raw(Box::new(state)).cast();
        }
        Some(Err(error)) => reply.error = error.code.0,
        None => {
            reply.error = if value.future.is_some() {
                Error::NOT_READY.code.0
            } else {
                Error::CLOSED.code.0
            }
        }
    }
    reply
}

unsafe extern "C" fn release(context: *mut c_void) {
    drop(unsafe { Box::from_raw(context.cast::<Preparing>()) });
}
