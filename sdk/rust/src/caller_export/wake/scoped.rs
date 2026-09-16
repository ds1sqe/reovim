use alloc::{sync::Arc, task::Wake};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
    task::Waker,
};
use reovim_kernel_interface::{Error, abi};

const CLOSED: usize = 1 << (usize::BITS - 1);

struct Relay {
    state: AtomicUsize,
    native: UnsafeCell<Option<Waker>>,
}

// SAFETY: Only shared Waker calls access native while a counted read is held.
// The serialized close writer first fences new reads and waits for all counted
// readers to finish before taking the Waker. Waker itself is Send + Sync.
unsafe impl Sync for Relay {}

impl Relay {
    fn enter(&self) -> bool {
        let mut old = self.state.load(Ordering::Relaxed);
        while old < CLOSED - 1 {
            match self.state.compare_exchange_weak(
                old,
                old + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(current) => old = current,
            }
        }
        false
    }
}

impl Wake for Relay {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if !self.enter() {
            return;
        }
        // SAFETY: This counted reader precedes the close fence. The native
        // Waker remains immutable until this and every earlier reader exit.
        if let Some(waker) = unsafe { &*self.native.get() } {
            waker.wake_by_ref();
        }
        self.state.fetch_sub(1, Ordering::Release);
    }
}

pub struct Scoped {
    pub raw: abi::Wake,
    relay: Arc<Relay>,
}
impl Scoped {
    pub unsafe fn new(raw: abi::Wake) -> Result<Self, Error> {
        // SAFETY: The caller lends live thread-safe native callbacks; import
        // retains their reference before constructing the disconnectable relay.
        let native = unsafe { super::import(raw)? };
        Ok(Self {
            raw,
            relay: Arc::new(Relay {
                state: AtomicUsize::new(0),
                native: UnsafeCell::new(Some(native)),
            }),
        })
    }
    pub fn waker(&self) -> Waker {
        Waker::from(self.relay.clone())
    }
    pub fn close(&mut self) -> Result<(), Error> {
        let old = self.relay.state.fetch_or(CLOSED, Ordering::AcqRel);
        if old & !CLOSED != 0 {
            return Err(Error::BUSY);
        }
        // SAFETY: New reads are fenced and all previously counted reads ended.
        // Scoped owns the sole serialized writer; retained relay Wakers now
        // observe Closed and cannot touch this field or call foreign code.
        let native = unsafe { &mut *self.relay.native.get() }.take();
        drop(native);
        Ok(())
    }
}
