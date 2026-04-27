//! Minimal cdylib fixture for the Phase 0 driver-ABI round-trip test.
//!
//! Implements the smallest possible `ClientRender` driver and uses
//! `declare_client_render_driver!` to export the vtable. The integration
//! test at
//! `clients/lib/subsys/driver-loader/tests/dlopen_roundtrip.rs` dlopens
//! the resulting `.so` (or `.dylib`/`.dll`), validates the header,
//! constructs an instance, submits a payload, and drops.
//!
//! This is a test fixture: `publish = false`, not part of any
//! production binary.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_render::{
        client_render::{ClientRender, ClientRenderDriverProbe, ClientRenderError},
        target::{RenderError, RenderTarget},
    },
    reovim_driver_macros::declare_client_render_driver,
    std::{
        ffi::c_void,
        sync::atomic::{AtomicU32, Ordering},
    },
};

/// Flags set by the driver's lifecycle methods. Read by integration
/// tests via the exported `reovim_driver_abi_poc_lifecycle_flags`
/// accessor to verify `shutdown` ran before `destroy` and before the
/// cdylib was unmapped. Stored as a single `AtomicU32` (bit 0 =
/// shutdown, bit 1 = destroy) so one `dlsym` covers both.
static LIFECYCLE_FLAGS: AtomicU32 = AtomicU32::new(0);

const FLAG_SHUTDOWN: u32 = 1 << 0;

/// Accessor read by the integration test after a driver instance has
/// been dropped. Lives in the cdylib so the test observes the same
/// `LIFECYCLE_FLAGS` the driver writes.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_driver_abi_poc_lifecycle_flags() -> u32 {
    LIFECYCLE_FLAGS.load(Ordering::SeqCst)
}

pub struct PocDriver {
    target_impl: PocRenderTarget,
}

pub struct PocRenderTarget;

impl RenderTarget for PocRenderTarget {
    fn submit(&mut self, _data: &[u8]) -> Result<(), RenderError> {
        Ok(())
    }
}

impl ClientRender for PocDriver {
    fn probe() -> ClientRenderDriverProbe {
        ClientRenderDriverProbe::new("poc", "Phase 0 driver-ABI PoC")
    }

    fn construct(_platform: *mut c_void) -> Result<Self, ClientRenderError> {
        // TODO(O7): FfiPlatformCaps repr(C) stability — Phase 2 tightens this.
        Ok(Self {
            target_impl: PocRenderTarget,
        })
    }

    fn target(&mut self) -> &mut dyn RenderTarget {
        &mut self.target_impl
    }

    fn shutdown(&mut self) -> Result<(), ClientRenderError> {
        LIFECYCLE_FLAGS.fetch_or(FLAG_SHUTDOWN, Ordering::SeqCst);
        Ok(())
    }
}

declare_client_render_driver!(PocDriver);
