use alloc::{boxed::Box, sync::Arc};
use core::{
    future::poll_fn,
    sync::atomic::{AtomicBool, Ordering},
    task::{Poll, Waker},
};
use reovim_kernel_interface::{Error, Message, provider::ProviderReply};
use std::sync::Mutex;

#[derive(Default)]
struct Completion {
    result: Option<Result<Message, Error>>,
    wake: Option<Waker>,
}

pub fn start(
    busy: &Arc<AtomicBool>,
    job: impl FnOnce() -> Result<Message, Error> + Send + 'static,
) -> ProviderReply {
    if busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Box::pin(async { Err(Error::BUSY) });
    }
    let completion = Arc::new(Mutex::new(Completion::default()));
    let output = completion.clone();
    let running = busy.clone();
    let spawned = std::thread::Builder::new()
        .name("resource-io".into())
        .spawn(move || complete(&output, &running, job()));
    if spawned.is_err() {
        busy.store(false, Ordering::Release);
        return Box::pin(async { Err(Error::PRESSURE) });
    }
    Box::pin(poll_fn(move |context| {
        let mut completion = completion
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(result) = completion.result.take() {
            return Poll::Ready(result);
        }
        completion.wake = Some(context.waker().clone());
        Poll::Pending
    }))
}
fn complete(
    completion: &Mutex<Completion>,
    busy: &AtomicBool,
    result: Result<Message, Error>,
) {
    let wake = {
        let mut completion = completion
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        completion.result = Some(result);
        busy.store(false, Ordering::Release);
        completion.wake.take()
    };
    if let Some(wake) = wake {
        wake.wake();
    }
}
