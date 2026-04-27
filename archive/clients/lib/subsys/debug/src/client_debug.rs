//! `ClientDebugSurface` driver trait.
//!
//! A client-debug driver owns a per-driver debug protocol and hands out
//! `DebugObserver` sub-handles for streaming frames plus a one-shot
//! `drive` command. The trait is exposed at the subsys tier so
//! `uapi/driver-macros/` can generate FFI trampolines against it.

use {crate::observer::DebugObserver, std::ffi::c_void};

/// Maximum byte length (including the trailing NUL) of a schema name
/// stored in [`DebugProbe`].
pub const SCHEMA_NAME_LEN: usize = 64;

/// Maximum number of observe- or drive-schema slots in [`DebugProbe`].
///
/// Static ceiling for v1: adding a ninth schema kind is a vtable
/// ABI-version bump.
pub const MAX_SCHEMAS: usize = 8;

/// Driver error reported at the FFI boundary.
///
/// At the boundary this is converted to `i32` + `*mut *mut c_char`
/// out-param. The macro-generated trampolines allocate the error string
/// via `CString::into_raw` on the driver side; the host reads it and
/// returns it through the vtable's `destroy_error_string` slot.
#[derive(Debug, Clone)]
pub struct DebugError(pub String);

impl std::fmt::Display for DebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DebugError {}

/// Fixed-size probe metadata published by a client-debug driver.
///
/// Read by the host during the scan phase before any driver code runs.
/// Fields are byte arrays instead of pointers so the probe can live
/// inside the static vtable as plain `#[repr(C)]` data, writable from a
/// `const fn` initializer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DebugProbe {
    /// Short driver identifier (null-terminated, max 63 bytes + NUL).
    pub driver_name: [u8; 64],
    /// Human-readable description (null-terminated, max 191 bytes + NUL).
    pub description: [u8; 192],

    /// Number of populated `observe_schemas[..]` entries; `<= MAX_SCHEMAS`.
    pub observe_schemas_count: u32,
    /// Observe-schema names; each slot is null-terminated, max
    /// `SCHEMA_NAME_LEN - 1` bytes + NUL.
    pub observe_schemas: [[u8; SCHEMA_NAME_LEN]; MAX_SCHEMAS],

    /// Number of populated `drive_schemas[..]` entries; `<= MAX_SCHEMAS`.
    pub drive_schemas_count: u32,
    /// Drive-schema names; each slot is null-terminated, max
    /// `SCHEMA_NAME_LEN - 1` bytes + NUL.
    pub drive_schemas: [[u8; SCHEMA_NAME_LEN]; MAX_SCHEMAS],
}

impl DebugProbe {
    /// Build a probe from static strings. Truncates inputs that exceed
    /// the field buffers or the schema-list ceilings. `const fn` so this
    /// can appear in a `#[unsafe(no_mangle)] pub static` vtable
    /// initializer.
    #[must_use]
    pub const fn new(
        driver_name: &str,
        description: &str,
        observe_schemas: &[&str],
        drive_schemas: &[&str],
    ) -> Self {
        let mut out = Self {
            driver_name: [0; 64],
            description: [0; 192],
            observe_schemas_count: 0,
            observe_schemas: [[0; SCHEMA_NAME_LEN]; MAX_SCHEMAS],
            drive_schemas_count: 0,
            drive_schemas: [[0; SCHEMA_NAME_LEN]; MAX_SCHEMAS],
        };

        let name_bytes = driver_name.as_bytes();
        let name_len = if name_bytes.len() < 63 {
            name_bytes.len()
        } else {
            63
        };
        let mut i = 0;
        while i < name_len {
            out.driver_name[i] = name_bytes[i];
            i += 1;
        }

        let desc_bytes = description.as_bytes();
        let desc_len = if desc_bytes.len() < 191 {
            desc_bytes.len()
        } else {
            191
        };
        i = 0;
        while i < desc_len {
            out.description[i] = desc_bytes[i];
            i += 1;
        }

        let obs_count = if observe_schemas.len() < MAX_SCHEMAS {
            observe_schemas.len()
        } else {
            MAX_SCHEMAS
        };
        let mut slot = 0;
        while slot < obs_count {
            let s = observe_schemas[slot].as_bytes();
            let len = if s.len() < SCHEMA_NAME_LEN - 1 {
                s.len()
            } else {
                SCHEMA_NAME_LEN - 1
            };
            let mut j = 0;
            while j < len {
                out.observe_schemas[slot][j] = s[j];
                j += 1;
            }
            slot += 1;
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            out.observe_schemas_count = obs_count as u32;
        }

        let drive_count = if drive_schemas.len() < MAX_SCHEMAS {
            drive_schemas.len()
        } else {
            MAX_SCHEMAS
        };
        slot = 0;
        while slot < drive_count {
            let s = drive_schemas[slot].as_bytes();
            let len = if s.len() < SCHEMA_NAME_LEN - 1 {
                s.len()
            } else {
                SCHEMA_NAME_LEN - 1
            };
            let mut j = 0;
            while j < len {
                out.drive_schemas[slot][j] = s[j];
                j += 1;
            }
            slot += 1;
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            out.drive_schemas_count = drive_count as u32;
        }

        out
    }
}

/// A loadable client-debug driver.
///
/// Implementations are produced by driver cdylibs. The host loads the
/// cdylib, reads `REOVIM_CLIENT_DEBUG_DRIVER_VTABLE`, validates the
/// vtable header, calls `construct`, then drives the driver's
/// observe/drive surface.
pub trait ClientDebugSurface: Send + Sync {
    /// Metadata surfaced before any driver code runs. `reovim cli debug
    /// probe` consumes this through the loader's scan path without
    /// instantiating any driver.
    fn probe() -> DebugProbe
    where
        Self: Sized;

    /// Instantiate the driver.
    ///
    /// The `platform` pointer is the host's platform handle, reserved
    /// for the master-plan §O7 tightening. In Phase 0 the host passes
    /// `std::ptr::null_mut()` and drivers ignore the value.
    ///
    /// # Errors
    ///
    /// Returns `DebugError` if construction fails. The error string is
    /// reported back to the host through the vtable's `construct`
    /// out-param.
    fn construct(platform: *mut c_void) -> Result<Self, DebugError>
    where
        Self: Sized;

    /// Open an observation stream for the given schema/selector.
    ///
    /// The returned observer borrows `&mut self`, so two observers
    /// cannot coexist on the same driver instance at the Rust type
    /// level. This matches the ABI's `observe_start` semantics: a new
    /// observer handle is produced per call, and the Rust borrow makes
    /// double-subscription a compile error rather than a runtime check.
    ///
    /// # Errors
    ///
    /// Returns `DebugError` if the selector is not recognized or the
    /// driver cannot open the stream.
    fn observe(
        &mut self,
        selector: &[u8],
    ) -> Result<Box<dyn DebugObserver + Send + '_>, DebugError>;

    /// Execute a one-shot drive command. Opaque bytes in, opaque bytes
    /// out. The response `Vec` is allocated by the driver and its
    /// buffer is freed by the host through the ABI's `destroy_bytes`
    /// slot.
    ///
    /// # Errors
    ///
    /// Returns `DebugError` if the command cannot be processed.
    fn drive(&mut self, command: &[u8]) -> Result<Vec<u8>, DebugError>;

    /// Drain outstanding work and prepare for destruction.
    ///
    /// # Errors
    ///
    /// Returns `DebugError` if shutdown fails.
    fn shutdown(&mut self) -> Result<(), DebugError>;
}

#[cfg(test)]
#[path = "client_debug_tests.rs"]
mod tests;
