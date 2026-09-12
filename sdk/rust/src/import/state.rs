use alloc::{boxed::Box, rc::Rc};
use core::{
    any::Any,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, abi,
    buffer::{State, StateReply},
};

type Adopt = Box<dyn FnOnce(abi::Reply) -> Result<State, Error>>;

struct Preparing {
    pending: Option<abi::StatePending>,
    adopt: Option<Adopt>,
    _owner: Rc<dyn Any>,
}

/// Adopts one native preparation, taking ready state before pending release.
/// # Safety
/// The table obeys `StatePending` and no-unwind rules. Owner pins code/context;
/// adopt consumes the Reply using that Model's matching release/drop callbacks.
pub unsafe fn import_state_reply(
    pending: abi::StatePending,
    owner: Rc<dyn Any>,
    adopt: impl FnOnce(abi::Reply) -> Result<State, Error> + 'static,
) -> StateReply {
    Box::pin(Preparing {
        pending: Some(pending),
        adopt: Some(Box::new(adopt)),
        _owner: owner,
    })
}

impl Preparing {
    fn release(&mut self) {
        if let Some(pending) = self.pending.take() {
            unsafe {
                (pending.release)(pending.context);
            }
        }
    }
}
impl Drop for Preparing {
    fn drop(&mut self) {
        self.release();
    }
}

impl Future for Preparing {
    type Output = Result<State, Error>;
    fn poll(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let Some(pending) = self.pending else {
            return Poll::Ready(Err(Error::CLOSED));
        };
        let wake = super::wake::Export::new(context.waker().clone());
        let output = unsafe { (pending.poll)(pending.context, wake.0) };
        if output.status == 0 && output.error == 0 {
            return Poll::Pending;
        }
        let result = result(&mut self.adopt, pending, output);
        self.release();
        Poll::Ready(result)
    }
}

fn result(
    adopt: &mut Option<Adopt>,
    pending: abi::StatePending,
    output: abi::StatePoll,
) -> Result<State, Error> {
    match output.status {
        1 if output.error == 0 => {
            let reply = unsafe { (pending.take)(pending.context) };
            adopt.take().expect("one state transfer")(reply)
        }
        2 if output.error != 0 => Err(Error::new(output.error)),
        _ => Err(Error::PROVIDER),
    }
}
