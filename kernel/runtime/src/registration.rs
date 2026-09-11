use crate::Limits;
use reovim_kernel_interface::{
    Contract, Error, buffer::ModelProvider, provider::ResourceProvider,
};

pub fn resource(
    provider: &ResourceProvider,
    limits: Limits,
) -> Result<(), Error> {
    if provider.surfaces.is_empty() {
        return Err(Error::INVALID);
    }
    if provider.surfaces.len() > limits.resources {
        return Err(Error::PRESSURE);
    }
    for (index, surface) in provider.surfaces.iter().enumerate() {
        if provider.forwarding
            && (!surface.input.is_empty() || !surface.output.is_empty())
        {
            return Err(Error::INVALID);
        }
        contract(&surface.contract, limits)?;
        if surface.input.len() > limits.resources
            || surface.output.len() > limits.resources
        {
            return Err(Error::PRESSURE);
        }
        let ambiguous = provider.surfaces[..index].iter().any(|prior| {
            prior.contract == surface.contract
                && prior.methods.intersects(surface.methods)
        });
        if ambiguous {
            return Err(Error::INVALID);
        }
    }
    Ok(())
}

pub fn model(provider: &ModelProvider, limits: Limits) -> Result<(), Error> {
    if crate::closing::contract(&provider.edit)
        || crate::sessions::closing(&provider.edit)
        || reovim_kernel_interface::session::is_create(&provider.create)
    {
        return Err(Error::INVALID);
    }
    for value in [&provider.create, &provider.read, &provider.edit] {
        contract(value, limits)?;
    }
    Ok(())
}
fn contract(value: &Contract, limits: Limits) -> Result<(), Error> {
    if value.name.is_empty() {
        return Err(Error::INVALID);
    }
    if value.name.len() > limits.path_bytes {
        return Err(Error::PRESSURE);
    }
    Ok(())
}
