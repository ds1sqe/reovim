//! Native declarations feed the existing scoped catalog and activation graph.

mod config;
pub use config::{Config, Initial, Scope, Snapshot, Source};

use alloc::{boxed::Box, format, vec::Vec};
use reovim_host_reference::native::Artifact;
use reovim_kernel_interface::{
    Contract, Error, RequestError, module, provider::Permissions,
};
use reovim_kernel_runtime::{Binding, Runtime};

pub struct Modules {
    sources: Vec<(Source, Artifact)>,
    start: Vec<Contract>,
    client_start: Vec<Contract>,
}
pub type Startup = core::pin::Pin<
    Box<dyn core::future::Future<Output = Result<(), RequestError>>>,
>;
impl Modules {
    /// Discovers declared artifacts without constructing module instances.
    /// # Errors
    /// Rejects inaccessible or malformed native declarations.
    pub fn discover(config: Config) -> Result<Self, Error> {
        let sources = config
            .sources
            .into_iter()
            .map(discover)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            sources,
            start: config.start,
            client_start: config.client_start,
        })
    }

    /// Installs declarations and returns owned startup work for this context.
    /// # Errors
    /// Preserves catalog/instance ownership on registration or startup failure.
    pub fn prepare(
        &self,
        runtime: &mut Runtime,
        owner: &Binding,
        scope: Scope,
    ) -> Result<Startup, RequestError> {
        let catalog_path = owner.publication_path("catalog")?;
        let catalog = runtime.register_namespace(&catalog_path)?;
        let mut declarations = Vec::new();
        for (source, artifact) in
            self.sources.iter().filter(|(s, _)| s.scope == scope)
        {
            let path = register(runtime, owner, source, artifact)?;
            declarations.push((artifact.identity(), path));
        }
        let required = match scope {
            Scope::Server => &self.start,
            Scope::Client => &self.client_start,
        };
        let selected = required
            .iter()
            .map(|identity| select(&declarations, identity))
            .collect::<Result<Vec<_>, _>>()?;
        // Dependency resolution sees this context's catalog, independently of
        // the provider's data imports and the client's shared aliases.
        let control = runtime.connect(&[(catalog, Permissions::ALL)])?;
        let resources = selected
            .into_iter()
            .map(|path| control.resolve(path).into_inner())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(activate(control, resources))
    }
}
fn activate(
    control: reovim_kernel_interface::Connection,
    resources: Vec<reovim_kernel_interface::ResourceId>,
) -> Startup {
    Box::pin(async move {
        for resource in resources {
            module::activate(&control, resource).await?;
        }
        Ok(())
    })
}
#[allow(unsafe_code)]
fn discover(source: Source) -> Result<(Source, Artifact), Error> {
    // Process configuration explicitly admits artifacts obeying the native ABI.
    let artifact = unsafe { Artifact::open(&source.library)? };
    Ok((source, artifact))
}
fn register(
    runtime: &mut Runtime,
    owner: &Binding,
    source: &Source,
    artifact: &Artifact,
) -> Result<Box<str>, Error> {
    let grants = source
        .grants
        .iter()
        .map(|(path, rights)| {
            owner.resource(path).map(|resource| (resource, *rights))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let binding = runtime.bind_for(owner, &grants)?;
    let mount = owner.publication_path(&source.mount)?;
    let path = owner.publication_path(&format!("catalog/{}", source.label))?;
    let factory = artifact.factory(&mount, binding.connection());
    runtime
        .register_bound_factory(
            &binding,
            &path,
            artifact.identity().clone(),
            factory,
        )
        .map_err(|(error, _, _)| error)?;
    Ok(path)
}
fn select<'a>(
    declarations: &'a [(&Contract, Box<str>)],
    identity: &Contract,
) -> Result<&'a str, Error> {
    let mut matching =
        declarations.iter().filter(|(declared, _)| *declared == identity);
    let missing = if declarations
        .iter()
        .any(|(declared, _)| declared.name == identity.name)
    {
        Error::UNSUPPORTED
    } else {
        Error::UNAVAILABLE
    };
    let selected = matching.next().ok_or(missing)?;
    if matching.next().is_some() {
        return Err(Error::INVALID);
    }
    Ok(&selected.1)
}
