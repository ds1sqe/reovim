#![cfg(target_os = "linux")]
#![allow(unsafe_code)]

use reovim_kernel_interface::{
    Error, Outcome, Pending, ResourceId, Value, abi,
    declaration::{Operation, ValueLimits},
};
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::PathBuf,
    process::Command,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

const LIMITS: ValueLimits =
    ValueLimits { bytes: 4096, elements: 256, references: 8 };
const fn grant(
    resource: ResourceId,
    access: u32,
) -> reovim_kernel_runtime::Grant {
    reovim_kernel_runtime::Grant {
        resource,
        access,
        create_access: 0,
        retain_loans: false,
    }
}

#[path = "native/memory.rs"]
mod memory;

#[path = "native/state.rs"]
mod state;

#[path = "native/state_export.rs"]
mod state_export;

#[path = "native/edit.rs"]
mod edit;

#[path = "native/edit_export.rs"]
mod edit_export;

static NEXT: AtomicU64 = AtomicU64::new(0);

#[path = "native/namespace.rs"]
mod namespace;

#[path = "native/module_export.rs"]
mod module_export;

#[path = "native/wake_scope.rs"]
mod wake_scope;

#[path = "native/provider_export.rs"]
mod provider_export;

#[path = "native/interfaces.rs"]
mod interfaces;

#[path = "native/pending_export.rs"]
mod pending_export;

#[path = "native/registrar.rs"]
mod registrar;

#[path = "native/runtime.rs"]
mod runtime;

#[path = "native/service.rs"]
mod service;

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

struct Library {
    handle: *mut c_void,
    directory: PathBuf,
}

impl Library {
    fn compile() -> Rc<Self> {
        Self::compile_source("operations.c")
    }

    fn compile_source(source: &str) -> Rc<Self> {
        let directory = std::env::temp_dir().join(format!(
            "reovim-declared-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let library = directory.join("operations.so");
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let status = Command::new("cc")
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            ])
            .arg("-I")
            .arg(manifest.join("../../host/reference/include"))
            .arg(manifest.join("tests/native").join(source))
            .arg("-o")
            .arg(&library)
            .status()
            .unwrap();
        assert!(status.success());
        let path =
            CString::new(library.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: path is a live NUL-terminated string naming our compiled
        // fixture. Library owns the returned handle until the final Rc drops.
        let handle = unsafe { dlopen(path.as_ptr(), 2) };
        assert!(!handle.is_null());
        Rc::new(Self { handle, directory })
    }

    unsafe fn symbol<T: Copy>(&self, name: &CStr) -> T {
        // SAFETY: self retains a live dlopen handle and name is NUL-terminated.
        let pointer = unsafe { dlsym(self.handle, name.as_ptr()) };
        assert!(!pointer.is_null());
        assert_eq!(size_of::<T>(), size_of::<*mut c_void>());
        // SAFETY: The caller selects the fixture's exact extern-C signature;
        // Linux dlsym function addresses use this checked pointer-size layout.
        unsafe { core::mem::transmute_copy(&pointer) }
    }

    fn create(&self) -> abi::Provider {
        // SAFETY: operations.c exports this exact constructor signature.
        let create: unsafe extern "C" fn(*mut abi::Provider) -> u32 =
            unsafe { self.symbol(c"fixture_create") };
        let mut output = core::mem::MaybeUninit::uninit();
        // SAFETY: The constructor receives writable aligned Provider storage;
        // the fixture fully initializes it on the asserted successful return.
        assert_eq!(unsafe { create(output.as_mut_ptr()) }, 0);
        // SAFETY: The successful constructor initialized every Provider field.
        unsafe { output.assume_init() }
    }

    fn count(&self, name: &CStr) -> u64 {
        // SAFETY: Tests pass only the fixture's no-argument u64 counters.
        let read: unsafe extern "C" fn() -> u64 = unsafe { self.symbol(name) };
        // SAFETY: The signature matches and self retains executable code.
        unsafe { read() }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: This is the final Rc lease. Tests finish pending/result and
        // provider ownership before releasing the library's sole dlopen handle.
        assert_eq!(unsafe { dlclose(self.handle) }, 0);
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

struct Signal(AtomicBool);
impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn ready(pending: &mut Pending) {
    let signal = Arc::new(Signal(AtomicBool::new(false)));
    let waker = Waker::from(signal.clone());
    assert_eq!(
        pending.poll(&mut Context::from_waker(&waker)),
        Ok(Poll::Ready(()))
    );
    assert!(signal.0.load(Ordering::Relaxed));
}

fn number(value: &Value) -> u64 {
    assert!(value.references.is_empty());
    u64::from_le_bytes(value.data.as_ref().try_into().unwrap())
}

#[test]
fn independent_c_provider_transfers_results_and_refines_one_effect() {
    let library = Library::compile();
    let raw = library.create();
    // SAFETY: The fixture supplies valid immutable ABI tables. This Rc retains
    // its library; successful import takes the newly constructed context.
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let exec = provider.interfaces()[0].methods()[0].clone();
    assert_eq!(exec.operation(), Operation::Exec);
    let mut request = provider
        .begin(
            0,
            Operation::Exec,
            &Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
        )
        .unwrap();
    assert_eq!(request.take().err(), Some(Error::NOT_READY));
    let mut read =
        provider.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap();
    assert_eq!(provider.stop(), Err(Error::BUSY));
    assert!(matches!(
        provider.begin(0, Operation::Ioctl(1), &Value::unit()),
        Err(Error::CLOSED)
    ));
    ready(&mut request);
    let progress = request.take().unwrap();
    let Outcome::Unknown { error, progress: Some(value) } = &progress.outcome
    else {
        panic!("expected retained uncertain effect")
    };
    assert_eq!(number(value), 46);
    assert_eq!(
        *error,
        Error::with_id(
            90001,
            ResourceId {
                epoch: [0x51, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                serial: 17
            }
        )
    );
    exec.progress()
        .validate(&value.data, 0, LIMITS, |_, _, _| unreachable!())
        .unwrap();
    assert_eq!(request.take().err(), Some(Error::CLOSED));
    assert_eq!(request.reconcile(), Err(Error::BUSY));
    assert_eq!(request.finish(), Err(Error::BUSY));
    drop(progress);
    assert_eq!(library.count(c"fixture_released"), 1);
    request.reconcile().unwrap();
    ready(&mut request);
    let completed = request.take().unwrap();
    let Outcome::Ok(value) = &completed.outcome else { panic!("expected OK") };
    assert_eq!(number(value), 46); // Reconcile did not add five again.
    assert_eq!(request.reconcile(), Err(Error::CLOSED));
    assert_eq!(request.finish(), Err(Error::BUSY));
    drop(completed);
    request.finish().unwrap();
    drop(request);
    ready(&mut read);
    let result = read.take().unwrap();
    let Outcome::Ok(value) = &result.outcome else { panic!("expected read") };
    assert_eq!(number(value), 46);
    drop(result);
    read.finish().unwrap();
    drop(read);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_released"), 3);
    assert_eq!(library.count(c"fixture_finished"), 2);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn rejected_native_metadata_keeps_provider_ownership_with_its_supplier() {
    let library = Library::compile();
    let raw = library.create();
    let mut wrong = raw;
    wrong.bytes = size_of::<usize>();
    // SAFETY: wrong is fully initialized aligned storage. Only its advertised
    // size is invalid; rejection must not invoke provider callbacks.
    let rejected = unsafe {
        reovim_sdk::import_provider(&raw const wrong, library.clone(), LIMITS)
    };
    assert_eq!(rejected.err(), Some(Error::INVALID));
    assert_eq!(library.count(c"fixture_destroyed"), 0);
    // SAFETY: Rejection left the original fixture context owned and live; its
    // valid descriptor and library lease now transfer on successful import.
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    assert_eq!(
        provider.begin(0, Operation::Ioctl(0), &Value::unit()).err(),
        Some(Error::UNSUPPORTED)
    );
    // SAFETY: The fixture exports this exact inspection signature.
    let value: unsafe extern "C" fn(*const abi::Provider) -> u64 =
        unsafe { library.symbol(c"fixture_value") };
    // SAFETY: provider still owns raw.context and library keeps code mapped.
    assert_eq!(unsafe { value(&raw const raw) }, 41);
    assert_eq!(library.count(c"fixture_finished"), 0);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn factory_child_declaration()
-> reovim_kernel_interface::declaration::Declaration {
    let mut bytes = vec![0x75; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(2);
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 1, 0, 0, 0, 0, 4, 0]);
    bytes.extend_from_slice(b"read");
    bytes.extend_from_slice(&[0, 4, 0]);
    reovim_kernel_interface::declaration::Declaration::decode(&bytes).unwrap()
}

#[path = "native/open.rs"]
mod opened;

#[path = "native/packet.rs"]
mod packet;
