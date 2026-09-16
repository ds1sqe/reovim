use alloc::{sync::Arc, task::Wake};
use core::{ffi::c_void, task::Waker};
use reovim_kernel_interface::{Error, abi};

struct Native(abi::Wake);

// SAFETY: Only import constructs Native, under the ABI's thread-safe wake,
// retain and release promise. Its immutable record owns one foreign reference.
unsafe impl Send for Native {}
// SAFETY: Shared wake calls use that same thread-safe foreign context; final
// release runs only after the last Arc owner, on whichever thread drops it.
unsafe impl Sync for Native {}

impl Wake for Native {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // SAFETY: import checked wake and retained the context; this Arc keeps
        // it alive, and the ABI permits concurrent callback invocation.
        unsafe { self.0.wake.unwrap()(self.0.context) };
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        // SAFETY: The final Arc consumes the single reference acquired by
        // import. The callback and its code remain live through release.
        unsafe { self.0.release.unwrap()(self.0.context) };
    }
}

pub unsafe fn import(value: abi::Wake) -> Result<Waker, Error> {
    let retain = value.retain.ok_or(Error::INVALID)?;
    if value.wake.is_none() || value.release.is_none() {
        return Err(Error::INVALID);
    }
    // SAFETY: Native ABI callers lend a live Wake with thread-safe callbacks.
    // This retained reference backs every cloned Arc/Waker until final drop.
    unsafe { retain(value.context) };
    Ok(Waker::from(Arc::new(Native(value))))
}

pub fn same(left: abi::Wake, right: abi::Wake) -> bool {
    left.context == right.context
        && callback(left.wake, right.wake)
        && callback(left.retain, right.retain)
        && callback(left.release, right.release)
}

fn callback(
    left: Option<unsafe extern "C" fn(*mut c_void)>,
    right: Option<unsafe extern "C" fn(*mut c_void)>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => core::ptr::fn_addr_eq(left, right),
        _ => false,
    }
}

pub const fn absent(value: abi::Wake) -> bool {
    value.context.is_null()
        && value.wake.is_none()
        && value.retain.is_none()
        && value.release.is_none()
}

mod scoped;
pub use scoped::Scoped;
