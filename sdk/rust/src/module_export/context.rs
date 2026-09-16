use reovim_kernel_interface::{Error, ResourceId, abi, module::ModuleScope};

pub(super) fn identities(
    context: &abi::ModuleContext,
) -> Result<(ResourceId, ResourceId, ModuleScope), Error> {
    let instance = identity(context.instance)?;
    let namespace = identity(context.namespace_resource)?;
    let scope = match context.scope {
        0 if context.session == abi::Identity::default() => ModuleScope::Global,
        1 => ModuleScope::Session(identity(context.session)?),
        _ => return Err(Error::INVALID),
    };
    Ok((instance, namespace, scope))
}

fn identity(id: abi::Identity) -> Result<ResourceId, Error> {
    if id.serial == 0 || id.epoch == [0; 16] {
        return Err(Error::INVALID);
    }
    Ok(ResourceId { epoch: id.epoch, serial: id.serial })
}
