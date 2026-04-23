//! Cdylib fixture whose `construct()` always returns an error.
//!
//! Exercises the loader's error-propagation path: the driver allocates
//! a C-string via `ClientRenderError`, the macro wraps it into
//! `out_err`, and the loader's `translate_rc` frees it through the
//! destroy slot while surfacing `LoadError::DriverError`.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_render::{
        client_render::{ClientRender, ClientRenderDriverProbe, ClientRenderError},
        target::{RenderError, RenderTarget},
    },
    reovim_driver_macros::declare_client_render_driver,
    std::ffi::c_void,
};

pub struct ConstructErrDriver;

pub struct UnreachableTarget;

impl RenderTarget for UnreachableTarget {
    fn submit(&mut self, _data: &[u8]) -> Result<(), RenderError> {
        unreachable!("construct always errors; target never materializes")
    }
}

impl ClientRender for ConstructErrDriver {
    fn probe() -> ClientRenderDriverProbe {
        ClientRenderDriverProbe::new("construct-err", "Construct-error PoC")
    }
    fn construct(_platform: *mut c_void) -> Result<Self, ClientRenderError> {
        Err(ClientRenderError("deliberate construct failure".to_owned()))
    }
    fn target(&mut self) -> &mut dyn RenderTarget {
        unreachable!("construct always errors; target never materializes")
    }
    fn shutdown(&mut self) -> Result<(), ClientRenderError> {
        unreachable!("construct always errors; shutdown never fires")
    }
}

declare_client_render_driver!(ConstructErrDriver);
