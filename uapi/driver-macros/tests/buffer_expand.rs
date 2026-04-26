//! Expansion smoke test for `declare_buffer_driver!`.
//!
//! Verifies the macro expansion compiles cleanly with a minimal user
//! driver type and that the generated vtable carries the canonical
//! constants for the text-buffer ABI.

#![allow(unsafe_code)]

use {
    reovim_content_codec::ByteNotifiable,
    reovim_driver_macros::declare_buffer_driver,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
    reovim_subsys_buffer::{
        Buffer, BufferDriver, BufferError, CodecAttachmentId,
        abi::{BufferDriverProbe, BufferVTable, REOVIM_BUFFER_DRIVER_ABI_VERSION},
    },
    std::{
        ffi::CStr,
        ops::Range,
        sync::{Arc, Mutex},
    },
};

// ── Stub Buffer ──────────────────────────────────────────────────────────────

pub struct HygieneBuffer {
    id: BufferId,
    bytes: Mutex<Vec<u8>>,
}

impl Buffer for HygieneBuffer {
    fn id(&self) -> BufferId {
        self.id
    }

    fn file_path(&self) -> Option<String> {
        None
    }

    fn set_file_path(&self, _path: Option<String>) -> Result<(), BufferError> {
        Ok(())
    }

    fn is_modified(&self) -> bool {
        false
    }

    fn size(&self) -> usize {
        self.bytes.lock().unwrap().len()
    }

    fn read_bytes(&self, range: Range<usize>) -> Result<Vec<u8>, BufferError> {
        let g = self.bytes.lock().unwrap();
        if range.end > g.len() || range.start > range.end {
            return Err(BufferError::InvalidEdit("range out of bounds".into()));
        }
        Ok(g[range].to_vec())
    }

    fn apply_edit(&self, _edit: ByteEdit) -> Result<(), BufferError> {
        Ok(())
    }

    fn write_to(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        writer.write_all(&self.bytes.lock().unwrap())
    }

    fn attach_codec(
        &self,
        _name: &str,
        _codec: Box<dyn ByteNotifiable>,
    ) -> Result<CodecAttachmentId, BufferError> {
        Err(BufferError::Driver("hygiene stub: attach not implemented".into()))
    }

    fn detach_codec(&self, _id: CodecAttachmentId) -> Result<Box<dyn ByteNotifiable>, BufferError> {
        Err(BufferError::Driver("hygiene stub: detach not implemented".into()))
    }

    fn list_codecs(&self) -> Vec<(CodecAttachmentId, String)> {
        Vec::new()
    }
}

// ── Stub Driver ──────────────────────────────────────────────────────────────

pub struct HygieneBufferDriver;

impl HygieneBufferDriver {
    #[must_use]
    pub fn probe() -> BufferDriverProbe {
        BufferDriverProbe::new("buffer", "hygiene-buffer")
    }

    /// # Errors
    /// Infallible in this stub; signature mirrors the production contract.
    pub const fn construct() -> Result<Self, BufferError> {
        Ok(Self)
    }
}

impl BufferDriver for HygieneBufferDriver {
    fn kind(&self) -> &'static str {
        "buffer"
    }

    fn create_buffer(
        &self,
        initial_bytes: &[u8],
        _file_path: Option<String>,
    ) -> Result<Arc<dyn Buffer>, BufferError> {
        Ok(Arc::new(HygieneBuffer {
            id: BufferId::new(),
            bytes: Mutex::new(initial_bytes.to_vec()),
        }))
    }

    fn open_buffer(&self, _file_path: &str) -> Result<Arc<dyn Buffer>, BufferError> {
        Err(BufferError::Io("hygiene stub: open not implemented".into()))
    }

    fn list_buffers(&self) -> Vec<BufferId> {
        Vec::new()
    }

    fn get_buffer(&self, _id: BufferId) -> Option<Arc<dyn Buffer>> {
        None
    }

    fn close_buffer(&self, id: BufferId) -> Result<(), BufferError> {
        Err(BufferError::NotFound(id))
    }
}

declare_buffer_driver!(HygieneBufferDriver);

// ── Vtable shape assertions ─────────────────────────────────────────────────

#[test]
fn vtable_symbol_exported_with_constants() {
    assert_eq!(REOVIM_BUFFER_DRIVER_VTABLE.abi_version, REOVIM_BUFFER_DRIVER_ABI_VERSION);
    assert_eq!(REOVIM_BUFFER_DRIVER_VTABLE.api_version.major, 1);
    assert_eq!(REOVIM_BUFFER_DRIVER_VTABLE.api_version.minor, 0);
    assert_eq!(REOVIM_BUFFER_DRIVER_VTABLE.api_version.patch, 0);
    assert_eq!(REOVIM_BUFFER_DRIVER_VTABLE.size_of_self, std::mem::size_of::<BufferVTable>());
}

#[test]
fn probe_trampoline_returns_kind_and_name() {
    // SAFETY: the probe slot is a `pub static` extern "C" fn pointer
    // that takes no arguments and never reads memory; calling it is
    // sound at any point after macro expansion.
    let probe = unsafe { (REOVIM_BUFFER_DRIVER_VTABLE.probe)() };

    let kind_nul = probe
        .kind
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(probe.kind.len());
    let name_nul = probe
        .name
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(probe.name.len());

    let kind = std::str::from_utf8(&probe.kind[..kind_nul]).unwrap();
    let name = std::str::from_utf8(&probe.name[..name_nul]).unwrap();
    assert_eq!(kind, "buffer");
    assert_eq!(name, "hygiene-buffer");
}

#[test]
fn construct_destroy_round_trip_succeeds() {
    use std::ffi::{c_char, c_void};

    let mut instance: *mut c_void = std::ptr::null_mut();
    let mut err: *mut c_char = std::ptr::null_mut();

    // SAFETY: `construct` writes `instance`; on success `err` stays
    // null. We immediately balance with `destroy`.
    let rc = unsafe { (REOVIM_BUFFER_DRIVER_VTABLE.construct)(&raw mut instance, &raw mut err) };
    assert_eq!(rc, 0);
    assert!(!instance.is_null());
    assert!(err.is_null(), "construct populated err on success");

    // SAFETY: `instance` was just produced by `construct`.
    unsafe {
        (REOVIM_BUFFER_DRIVER_VTABLE.destroy)(instance);
    }
}

#[test]
fn destroy_error_string_handles_null() {
    // SAFETY: passing null is the documented no-op contract.
    unsafe {
        (REOVIM_BUFFER_DRIVER_VTABLE.destroy_error_string)(std::ptr::null_mut());
    }
}

#[test]
fn destroy_error_string_round_trip() {
    use std::ffi::CString;

    let cs = CString::new("hygiene-test-error").unwrap();
    let raw = cs.into_raw();
    // Sanity: the pointer round-trips through CStr first.
    // SAFETY: `raw` is a valid CString allocation we just made.
    let view = unsafe { CStr::from_ptr(raw) };
    assert_eq!(view.to_bytes(), b"hygiene-test-error");
    // SAFETY: `raw` came from `CString::into_raw`; the destructor
    // re-boxes it via `CString::from_raw` and drops.
    unsafe {
        (REOVIM_BUFFER_DRIVER_VTABLE.destroy_error_string)(raw);
    }
}
