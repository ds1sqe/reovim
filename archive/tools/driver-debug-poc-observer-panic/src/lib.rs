//! Cdylib fixture whose `DebugObserver::next_frame` panics on its
//! second invocation.
//!
//! The round-trip test asserts the macro-generated observer
//! trampoline catches the panic via `catch_unwind` and surfaces the
//! `-2` return code as `LoadError::DriverPanicked` instead of aborting
//! the host process. This is the one genuinely novel code path in the
//! Phase 0 debug-surface sub-plan — no existing driver family
//! exercises a sub-handle iterator pump.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_debug::{
        client_debug::{ClientDebugSurface, DebugError, DebugProbe},
        observer::DebugObserver,
    },
    reovim_driver_macros::declare_client_debug_driver,
    std::ffi::c_void,
};

pub struct ObserverPanicDriver;

pub struct PanickingObserver {
    call_count: u32,
}

impl DebugObserver for PanickingObserver {
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, DebugError> {
        self.call_count += 1;
        assert!(self.call_count < 2, "deliberate observer panic for catch_unwind test");
        Ok(Some(b"first-frame".to_vec()))
    }
}

impl ClientDebugSurface for ObserverPanicDriver {
    fn probe() -> DebugProbe {
        DebugProbe::new("observer-panic", "Observer-panic PoC", &["panic-frames"], &[])
    }

    fn construct(_platform: *mut c_void) -> Result<Self, DebugError> {
        Ok(Self)
    }

    fn observe(
        &mut self,
        _selector: &[u8],
    ) -> Result<Box<dyn DebugObserver + Send + '_>, DebugError> {
        Ok(Box::new(PanickingObserver { call_count: 0 }))
    }

    fn drive(&mut self, _command: &[u8]) -> Result<Vec<u8>, DebugError> {
        Err(DebugError("drive not supported by this fixture".into()))
    }

    fn shutdown(&mut self) -> Result<(), DebugError> {
        Ok(())
    }
}

declare_client_debug_driver!(ObserverPanicDriver);
