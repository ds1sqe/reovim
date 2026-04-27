//! Hygiene test: `declare_client_render_driver!` must expand cleanly
//! inside a `#[no_implicit_prelude]` module so it can be invoked from
//! an ext crate that scopes its own prelude.

#![no_implicit_prelude]
#![allow(unsafe_code)]

mod hygienic {
    #![no_implicit_prelude]

    use {
        ::reovim_client_subsys_render::{
            client_render::{ClientRender, ClientRenderDriverProbe, ClientRenderError},
            target::{RenderError, RenderTarget},
        },
        ::reovim_driver_macros::declare_client_render_driver,
        ::std::{ffi::c_void, result::Result},
    };

    pub struct HygieneProbeDriver {
        tgt: HygieneTarget,
    }

    pub struct HygieneTarget;

    impl RenderTarget for HygieneTarget {
        fn submit(&mut self, _data: &[u8]) -> Result<(), RenderError> {
            Result::Ok(())
        }
    }

    impl ClientRender for HygieneDriver {
        fn probe() -> ClientRenderDriverProbe {
            ClientRenderDriverProbe::new("hygiene", "Hygiene Probe")
        }
        fn construct(_platform: *mut c_void) -> Result<Self, ClientRenderError> {
            Result::Ok(Self { tgt: HygieneTarget })
        }
        fn target(&mut self) -> &mut dyn RenderTarget {
            &mut self.tgt
        }
        fn shutdown(&mut self) -> Result<(), ClientRenderError> {
            Result::Ok(())
        }
    }

    pub type HygieneDriver = HygieneProbeDriver;

    // If the macro relied on an implicit prelude item, this line
    // would fail to compile. The test is the build itself.
    declare_client_render_driver!(HygieneDriver);
}
