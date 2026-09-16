//! Linux admission of independently built modules through the common entry.

use alloc::rc::Rc;
use core::ffi::{c_char, c_int, c_void};
use reovim_kernel_interface::{Error, abi};
use reovim_sdk::NativeModule;
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

struct Library(*mut c_void);

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: This owns one successful dlopen reference. SDK module,
        // instance, provider and result leases retain it until their final use.
        unsafe { dlclose(self.0) };
    }
}

/// Admits passive metadata from the selected artifact without constructing it.
///
/// Composition supplies granted imports and activates the returned module using
/// the SDK's `module_implementation` table. No backend kind enters this loader.
///
/// # Errors
/// Reports invalid paths, loader failure, missing entry or rejected metadata.
/// # Safety
/// Loading executes library initializers and entry code. The selected artifact
/// must obey the current native ABI, including readable immutable descriptors,
/// serialized callbacks, ownership rules and no unwinding across the boundary.
/// Native modules are trusted code; grants are not a memory sandbox.
pub unsafe fn load(path: &Path) -> Result<NativeModule, Error> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| Error::INVALID)?;
    // SAFETY: path is a live NUL-terminated native path. The caller trusts its
    // initializers. RTLD_NOW resolves imports now; symbols remain local.
    let handle = unsafe { dlopen(path.as_ptr(), 2) };
    if handle.is_null() {
        return Err(Error::UNAVAILABLE);
    }
    let library = Rc::new(Library(handle));
    // SAFETY: library owns a live handle; the entry name is NUL-terminated.
    let symbol = unsafe { dlsym(handle, c"rv_module_entry".as_ptr()) };
    if symbol.is_null() {
        return Err(Error::UNSUPPORTED);
    }
    // SAFETY: Linux dlsym supplies function addresses convertible to function
    // pointers. The caller guarantees the selected entry's exact ABI signature.
    let entry: unsafe extern "C" fn() -> *const abi::Module =
        unsafe { core::mem::transmute(symbol) };
    // SAFETY: The library lease retains this entry and its descriptor storage;
    // the passive entry obeys its declared return and ownership contract.
    let descriptor = unsafe { entry() };
    // SAFETY: The artifact guarantees readable tables and conforming callbacks.
    // Admission validates metadata and retains library through all later uses.
    unsafe { NativeModule::import(descriptor, library) }
}
