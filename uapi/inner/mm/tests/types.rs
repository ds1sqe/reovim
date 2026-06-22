use {
    core::{alloc::Layout, ptr::NonNull},
    reovim_uapi_mm::{AllocControl, AllocError},
};

fn refusing_alloc(_: Layout) -> Result<NonNull<u8>, AllocError> {
    Err(AllocError)
}

fn noop_dealloc(_: NonNull<u8>, _: Layout) {}

#[test]
fn alloc_error_is_unit_value() {
    assert_eq!(AllocError, AllocError);
}

#[test]
fn alloc_control_routes_allocation() {
    let control = AllocControl::new(refusing_alloc, noop_dealloc);
    assert_eq!(control.allocate(Layout::new::<u8>()), Err(AllocError));
}

#[test]
fn default_control_refuses_allocations() {
    assert_eq!(AllocControl::default().allocate(Layout::new::<u8>()), Err(AllocError));
}
