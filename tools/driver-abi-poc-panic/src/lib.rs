//! Cdylib fixture whose `submit` panics unconditionally.
//!
//! Round-trip test asserts the macro-generated trampoline catches the
//! panic via `catch_unwind` and surfaces the `-2` return code instead
//! of aborting the host process.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_render::{
        client_render::{ClientRender, ClientRenderDriverProbe, ClientRenderError},
        target::{RenderError, RenderTarget},
    },
    reovim_driver_macros::declare_client_render_driver,
    std::ffi::c_void,
};

pub struct PanicDriver {
    tgt: PanicTarget,
}

pub struct PanicTarget;

impl RenderTarget for PanicTarget {
    fn submit(&mut self, _data: &[u8]) -> Result<(), RenderError> {
        panic!("deliberate panic for catch_unwind test");
    }
}

impl ClientRender for PanicDriver {
    fn probe() -> ClientRenderDriverProbe {
        ClientRenderDriverProbe::new("panic", "Panic PoC")
    }
    fn construct(_platform: *mut c_void) -> Result<Self, ClientRenderError> {
        // TODO(O7): FfiPlatformCaps repr(C) stability — Phase 2 tightens this.
        Ok(Self { tgt: PanicTarget })
    }
    fn target(&mut self) -> &mut dyn RenderTarget {
        &mut self.tgt
    }
    fn shutdown(&mut self) -> Result<(), ClientRenderError> {
        Ok(())
    }
}

declare_client_render_driver!(PanicDriver);
