use alloc::{format, rc::Rc, vec, vec::Vec};
use reovim_kernel_interface::{
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Module},
    provider::{Contribution, ResourceProvider},
};
use reovim_kernel_runtime::{Binding, Runtime};
use std::io;

pub fn install(
    runtime: &mut Runtime,
    owner: &Binding,
    inputs: Vec<crate::boot::Snapshot>,
) -> io::Result<()> {
    for (index, input) in inputs.into_iter().enumerate() {
        let directory = reovim_host_reference::snapshot::Directory::open(
            &input.source,
            &input.scratch,
        )?;
        let mount =
            owner.publication_path(&input.mount).map_err(io::Error::other)?;
        let provider = directory.provider(owner.connection());
        let relative = format!("host/files-{index}");
        let path =
            owner.publication_path(&relative).map_err(io::Error::other)?;
        runtime
            .activate_bound_module(owner, &path, contribution(&mount, provider))
            .map_err(|(error, _)| io::Error::other(error))?;
        let kernel = owner.connection();
        let instance =
            kernel.resolve(&relative).into_inner().map_err(io::Error::other)?;
        super::session::finish(runtime, module::ready(&kernel, instance))?;
    }
    Ok(())
}

fn contribution(path: &str, provider: ResourceProvider) -> Module {
    Module::new(
        Lifecycle::Constructed,
        DriverOps {
            status: |state| *state,
            advance: |state, _| {
                if *state == Lifecycle::Closed {
                    return Ok(Progress::Stopped);
                }
                *state = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        vec![Contribution::Resource { path: path.into(), provider }],
        Rc::new(()),
    )
}
