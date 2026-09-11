use alloc::sync::Arc;
use core::{ffi::c_void, task::Waker};
use reovim_kernel_interface::abi;

pub(super) struct Export(pub abi::Wake);
impl Export {
    pub(super) fn new(waker: Waker) -> Self {
        Self(abi::Wake {
            context: Arc::into_raw(Arc::new(waker)).cast(),
            retain,
            release,
            wake,
        })
    }
}
impl Drop for Export {
    fn drop(&mut self) {
        unsafe {
            release(self.0.context);
        }
    }
}
unsafe extern "C" fn retain(context: *const c_void) {
    unsafe {
        Arc::increment_strong_count(context.cast::<Waker>());
    }
}
unsafe extern "C" fn release(context: *const c_void) {
    unsafe {
        Arc::decrement_strong_count(context.cast::<Waker>());
    }
}
unsafe extern "C" fn wake(context: *const c_void) {
    unsafe { &*context.cast::<Waker>() }.wake_by_ref();
}
