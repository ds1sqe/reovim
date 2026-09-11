#![no_std]
#![deny(unsafe_code)]
//! Assembly of independent implementations; consumers use only Interface.

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "std", target_os = "linux"))]
pub mod boot;
#[cfg(all(feature = "std", not(target_os = "linux")))]
#[path = "boot/unsupported.rs"]
pub mod boot;
#[cfg(feature = "std")]
mod loading;
#[cfg(feature = "std")]
pub mod service;

pub mod client;

use alloc::vec;
use reovim_kernel_interface::driver::DriverOps;
use reovim_kernel_interface::provider::{Contribution, Permissions};
use reovim_kernel_interface::{Connection, Error, ResourceId};
use reovim_kernel_runtime::{Limits, Runtime};

pub struct Service {
    pub runtime: Runtime,
    pub driver: DriverOps<Runtime>,
    pub client: Connection,
    pub source: ResourceId,
    pub heading: ResourceId,
    pub keyed: ResourceId,
}

/// # Errors
/// Reports failed construction/registration before supplying caller access.
pub fn assemble() -> Result<Service, Error> {
    let mut runtime = Runtime::new(1, Limits::default())?;
    runtime.start()?;
    let installed = runtime
        .register_contributions(vec![
            Contribution::Resource {
                path: "resource/notes".into(),
                provider: reovim_resource_local::memory(b"# a\n\xffopaque"),
            },
            Contribution::Model {
                path: "models/heading".into(),
                provider: reovim_representation_basic::heading(),
            },
            Contribution::Model {
                path: "models/keys".into(),
                provider: reovim_representation_basic::keyed(),
            },
        ])
        .map_err(|(error, _)| error)?;
    let [source, heading, keyed] = installed[..] else {
        return Err(Error::PROVIDER);
    };
    let client = reovim_sdk::local(runtime.connect(&[
        (source, Permissions::READ),
        (heading, Permissions::ALL),
        (keyed, Permissions::ALL),
    ])?);
    Ok(Service {
        runtime,
        driver: Runtime::DRIVER,
        client,
        source,
        heading,
        keyed,
    })
}
