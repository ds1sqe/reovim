use alloc::sync::Arc;
use core::{ffi::c_void, task::Waker};
use reovim_kernel_interface::abi;

pub struct Export(pub abi::Wake);

impl Export {
    pub(crate) fn new(waker: Waker) -> Self {
        Self(abi::Wake {
            context: Arc::into_raw(Arc::new(waker)).cast_mut().cast(),
            wake: Some(wake),
            retain: Some(retain),
            release: Some(release),
        })
    }
}

impl Drop for Export {
    fn drop(&mut self) {
        // SAFETY: new transferred exactly one Arc reference into context.
        // Export releases only that reference; native retain owns any others.
        unsafe { release(self.0.context) };
    }
}

unsafe extern "C" fn retain(context: *mut c_void) {
    // SAFETY: context came from Arc::into_raw in Export::new. The borrowed
    // callback or a prior retain keeps its strong count nonzero during retain.
    unsafe { Arc::increment_strong_count(context.cast::<Waker>()) };
}

unsafe extern "C" fn release(context: *mut c_void) {
    // SAFETY: Each call consumes one reference supplied by new or retain,
    // exactly once, using the original Arc allocation and Waker type.
    unsafe { Arc::decrement_strong_count(context.cast::<Waker>()) };
}

unsafe extern "C" fn wake(context: *mut c_void) {
    // SAFETY: The callback borrow or retained Arc keeps this Waker initialized
    // and alive. Only a shared reference is formed; Waker supports thread use.
    unsafe { &*context.cast::<Waker>() }.wake_by_ref();
}
