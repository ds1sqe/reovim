#![cfg(all(feature = "std", target_os = "linux"))]
#![allow(unsafe_code)]

use reovim_kernel_interface::{abi, provider::Permissions};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    ffi::{CString, c_char, c_int, c_void},
    mem::MaybeUninit,
    path::Path,
    process::Command,
    rc::Rc,
    task::{Context, Waker},
};

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}
struct Library(*mut c_void);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            dlclose(self.0);
        }
    }
}
#[test]
fn independent_c_caller_uses_the_kernel_table() {
    let mut runtime =
        Runtime::new(83, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let resource = runtime
        .register(
            "counter",
            reovim_kernel_interface_public_consumer::counter(Rc::clone(&value)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let connection = reovim_sdk::export(
        runtime.connect(&[(resource, Permissions::ALL)]).unwrap(),
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir()
        .join(format!("reovim-c-caller-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("caller.so");
    let output = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC"])
        .arg("-I")
        .arg(root.join("host/reference/include"))
        .arg(root.join("apps/server/tests/native/caller.c"))
        .arg("-o")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    let handle = unsafe { dlopen(path.as_ptr(), 2) };
    assert!(!handle.is_null());
    let library = Library(handle);
    let start = unsafe { dlsym(handle, c"reovim_client_start".as_ptr()) };
    let finish = unsafe { dlsym(handle, c"reovim_client_finish".as_ptr()) };
    assert!(!start.is_null() && !finish.is_null());
    // The independently compiled fixture declares these signatures in C.
    let start: unsafe extern "C" fn(
        abi::Connection,
        *mut abi::Pending,
    ) -> c_int = unsafe { std::mem::transmute(start) };
    let finish: unsafe extern "C" fn(abi::Pending, *mut u64) -> c_int =
        unsafe { std::mem::transmute(finish) };
    let mut pending = MaybeUninit::uninit();
    assert_eq!(unsafe { start(connection, pending.as_mut_ptr()) }, 0);
    assert_eq!(value.get(), 0);
    let mut context = Context::from_waker(Waker::noop());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    let mut actual = 0;
    assert_eq!(unsafe { finish(pending.assume_init(), &raw mut actual) }, 0);
    assert_eq!(actual, 11);
    assert_eq!(value.get(), 11);
    unsafe {
        ((*connection.table).release)(connection.context);
    }
    runtime.unregister(resource).unwrap();
    runtime.close().unwrap();
    drop(library);
    std::fs::remove_dir_all(directory).unwrap();
}
