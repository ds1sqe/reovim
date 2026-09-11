use alloc::{sync::Arc, task::Wake};
use core::{
    sync::atomic::{AtomicBool, Ordering},
    task::Waker,
};

pub struct Schedule {
    ready: Arc<AtomicBool>,
    owner: Option<Waker>,
}

impl Schedule {
    pub fn new() -> Self {
        Self { ready: Arc::new(AtomicBool::new(true)), owner: None }
    }

    pub fn runnable(&self, owner: &Waker) -> bool {
        self.ready.load(Ordering::Acquire)
            || self.owner.as_ref().is_none_or(|old| !old.will_wake(owner))
    }

    pub fn polling(&mut self, owner: &Waker) -> Waker {
        self.ready.store(false, Ordering::Release);
        self.owner = Some(owner.clone());
        Waker::from(Arc::new(Signal {
            ready: Arc::clone(&self.ready),
            owner: owner.clone(),
        }))
    }
}

struct Signal {
    ready: Arc<AtomicBool>,
    owner: Waker,
}

impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.ready.store(true, Ordering::Release);
        self.owner.wake_by_ref();
    }
}
