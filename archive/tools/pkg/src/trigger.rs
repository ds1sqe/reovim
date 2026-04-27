//! `pkg trigger`: build a [`LazyRegistry`] from the lockfile and open
//! every cdylib whose package matches the supplied trigger event.
//! Prints one line per opened cdylib.

use std::str::FromStr;

use {
    anyhow::{Context, Result},
    reovim_dylib_loader::{Kind, Library, cdylib_filename},
    reovim_pkg_lazyload::{LazyRegistry, TriggerEvent, names_to_load},
    reovim_pkg_lockfile::Lockfile,
};

use crate::cli::{TriggerArgs, TriggerKind};

pub fn run(args: &TriggerArgs) -> Result<()> {
    let raw = std::fs::read_to_string(&args.lockfile)
        .with_context(|| format!("failed to read lockfile at `{}`", args.lockfile.display()))?;
    let lock = Lockfile::from_toml_str(&raw)
        .with_context(|| format!("failed to parse lockfile at `{}`", args.lockfile.display()))?;
    let registry = LazyRegistry::from_lockfile(&lock).context("failed to build lazy registry")?;

    let event = match args.kind {
        TriggerKind::Domain => TriggerEvent::Domain(&args.name),
        TriggerKind::Event => TriggerEvent::Event(&args.name),
        TriggerKind::Capability => TriggerEvent::Capability(&args.name),
    };
    let names = names_to_load(&registry, &event);
    if names.is_empty() {
        return Ok(());
    }

    for name in names {
        let kind = lookup_kind(&lock, name)?;
        let path = args
            .library_root
            .join(kind.subdir())
            .join(cdylib_filename(name));
        Library::open(&path).with_context(|| format!("failed to open `{}`", path.display()))?;
        println!("loaded {name} from {}", path.display());
    }
    Ok(())
}

fn lookup_kind(lock: &Lockfile, name: &str) -> Result<Kind> {
    let pkg = lock
        .packages
        .iter()
        .find(|p| p.name == name)
        .with_context(|| format!("package `{name}` missing from lockfile"))?;
    let raw = pkg
        .kind
        .as_deref()
        .with_context(|| format!("package `{name}` has no `kind` field in the lockfile"))?;
    Kind::from_str(raw).with_context(|| format!("package `{name}` has invalid kind"))
}
