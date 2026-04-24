//! Minimal cdylib fixture for the Phase 0 client-debug driver-ABI
//! round-trip test.
//!
//! Implements the smallest `ClientDebugSurface` driver and uses
//! `declare_client_debug_driver!` to export the vtable. The integration
//! test at
//! `clients/lib/subsys/driver-loader/tests/debug_dlopen_roundtrip.rs`
//! dlopens the resulting `.so` (or `.dylib`/`.dll`), validates the
//! header, probes, constructs, observes three frames, drives an echo
//! command, and drops.
//!
//! This is a test fixture: `publish = false`, not part of any
//! production binary.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_debug::{
        client_debug::{ClientDebugSurface, DebugError, DebugProbe},
        observer::DebugObserver,
    },
    reovim_driver_macros::declare_client_debug_driver,
    std::{
        ffi::c_void,
        sync::atomic::{AtomicU32, Ordering},
    },
};

/// Flags set by the driver's lifecycle methods. Read by integration
/// tests via the exported `reovim_driver_debug_poc_lifecycle_flags`
/// accessor to verify `shutdown` ran before `destroy` and before the
/// cdylib was unmapped.
static LIFECYCLE_FLAGS: AtomicU32 = AtomicU32::new(0);

const FLAG_SHUTDOWN: u32 = 1 << 0;

/// Accessor read by the integration test after a driver instance has
/// been dropped.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_driver_debug_poc_lifecycle_flags() -> u32 {
    LIFECYCLE_FLAGS.load(Ordering::SeqCst)
}

pub struct DebugPocDriver {
    frame_count: u32,
}

pub struct PocObserver {
    remaining: u32,
}

impl DebugObserver for PocObserver {
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, DebugError> {
        if self.remaining == 0 {
            Ok(None)
        } else {
            self.remaining -= 1;
            Ok(Some(format!("frame-{}", self.remaining).into_bytes()))
        }
    }
}

impl ClientDebugSurface for DebugPocDriver {
    fn probe() -> DebugProbe {
        DebugProbe::new(
            "debug-poc",
            "Phase 0 debug-surface PoC",
            &["poc-frames"],
            &["poc-echo"],
        )
    }

    fn construct(_platform: *mut c_void) -> Result<Self, DebugError> {
        Ok(Self { frame_count: 3 })
    }

    fn observe(
        &mut self,
        selector: &[u8],
    ) -> Result<Box<dyn DebugObserver + Send + '_>, DebugError> {
        if selector == b"poc-frames" {
            Ok(Box::new(PocObserver {
                remaining: self.frame_count,
            }))
        } else {
            Err(DebugError(format!(
                "unknown selector: {}",
                String::from_utf8_lossy(selector)
            )))
        }
    }

    fn drive(&mut self, command: &[u8]) -> Result<Vec<u8>, DebugError> {
        // "poc-echo" schema: echo the command back as response.
        Ok(command.to_vec())
    }

    fn shutdown(&mut self) -> Result<(), DebugError> {
        LIFECYCLE_FLAGS.fetch_or(FLAG_SHUTDOWN, Ordering::SeqCst);
        Ok(())
    }
}

declare_client_debug_driver!(DebugPocDriver);
