use alloc::boxed::Box;
use core::task::{RawWaker, RawWakerVTable, Waker};
use reovim_kernel_interface::abi;

static TABLE: RawWakerVTable =
    RawWakerVTable::new(clone, wake, wake_by_ref, drop);

// ABI promises thread-safe callbacks. Each RawWaker owns one foreign reference.
pub(super) unsafe fn import(value: abi::Wake) -> Waker {
    unsafe {
        (value.retain)(value.context);
    }
    let pointer = Box::into_raw(Box::new(value));
    unsafe { Waker::from_raw(RawWaker::new(pointer.cast(), &TABLE)) }
}
unsafe fn clone(pointer: *const ()) -> RawWaker {
    let value = unsafe { *pointer.cast::<abi::Wake>() };
    unsafe {
        (value.retain)(value.context);
    }
    RawWaker::new(Box::into_raw(Box::new(value)).cast(), &TABLE)
}
unsafe fn wake_by_ref(pointer: *const ()) {
    let value = unsafe { &*pointer.cast::<abi::Wake>() };
    unsafe {
        (value.wake)(value.context);
    }
}
unsafe fn wake(pointer: *const ()) {
    unsafe {
        wake_by_ref(pointer);
        drop(pointer);
    }
}
unsafe fn drop(pointer: *const ()) {
    let value =
        unsafe { Box::from_raw(pointer.cast_mut().cast::<abi::Wake>()) };
    unsafe {
        (value.release)(value.context);
    }
}
