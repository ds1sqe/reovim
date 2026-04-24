use {super::*, std::mem};

#[test]
fn debug_observer_vtable_layout() {
    assert_eq!(mem::offset_of!(DebugObserverVTable, abi_version), 0);
    let ptr_size = mem::size_of::<usize>();
    assert!(mem::size_of::<DebugObserverVTable>() >= 2 * ptr_size);
    assert_eq!(mem::align_of::<DebugObserverVTable>(), mem::align_of::<usize>());
}

#[test]
fn client_debug_vtable_layout() {
    assert_eq!(mem::offset_of!(ClientDebugVTable, abi_version), 0);

    let abi_end = mem::offset_of!(ClientDebugVTable, abi_version) + mem::size_of::<u32>();
    let api_off = mem::offset_of!(ClientDebugVTable, api_version);
    assert!(api_off >= abi_end);

    let size_off = mem::offset_of!(ClientDebugVTable, size_of_self);
    assert!(size_off > api_off);

    assert_eq!(mem::align_of::<ClientDebugVTable>(), mem::align_of::<usize>());
}

#[test]
fn abi_version_is_one() {
    assert_eq!(REOVIM_CLIENT_DEBUG_DRIVER_ABI_VERSION, 1);
}

#[test]
fn api_version_is_one_zero_zero() {
    assert_eq!(REOVIM_CLIENT_DEBUG_DRIVER_API_VERSION, Version::new(1, 0, 0));
}

#[test]
fn version_field_is_repr_c_usable() {
    const _V: Version = REOVIM_CLIENT_DEBUG_DRIVER_API_VERSION;
}
