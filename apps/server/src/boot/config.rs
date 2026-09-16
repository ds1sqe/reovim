//! Explicit artifact selections; native metadata owns package identities.

use alloc::{string::String, vec::Vec};
use reovim_kernel_interface::Error;
use std::{io, path::PathBuf};

/// One configured artifact, e.g. label `memory`, library `/modules/memory.so`.
pub struct Source {
    /// Local configuration name; not a package or API identity.
    pub label: String,
    /// Native path passed unchanged to the Host admission policy.
    pub library: PathBuf,
}

/// Passive artifacts and required startup labels, e.g. `--start memory`.
/// Actual mounts, grants and Client module policy belong to composition.
#[derive(Default)]
pub struct Config {
    /// At most 64 distinct artifact selections.
    pub sources: Vec<Source>,
    /// Required labels, resolved before admission effects.
    pub start: Vec<String>,
}

impl Config {
    /// Parses `--module LABEL LIBRARY` and `--start LABEL`.
    /// Use programmatic `PathBuf` values for non-UTF-8 native paths.
    /// # Errors
    /// Unknown options, missing/duplicate selections and invalid names reject.
    pub fn parse(
        mut arguments: impl Iterator<Item = String>,
    ) -> io::Result<Self> {
        let mut config = Self::default();
        while let Some(option) = arguments.next() {
            config.option(&option, &mut arguments)?;
        }
        config.validate().map_err(|_| invalid("invalid boot selections"))?;
        Ok(config)
    }

    fn option(
        &mut self,
        option: &str,
        args: &mut impl Iterator<Item = String>,
    ) -> io::Result<()> {
        match option {
            "--module" if self.sources.len() < 64 => {
                self.sources.push(Source {
                    label: field(args)?,
                    library: field(args)?.into(),
                });
            }
            "--start" if self.start.len() < 64 => {
                self.start.push(field(args)?);
            }
            _ => return Err(invalid("unknown boot option or capacity")),
        }
        Ok(())
    }

    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.sources.len() > 64 || self.start.len() > 64 {
            return Err(Error::PRESSURE);
        }
        for (index, source) in self.sources.iter().enumerate() {
            validate_source(source, &self.sources[..index])?;
        }
        for (index, label) in self.start.iter().enumerate() {
            validate_start(label, &self.start[..index], &self.sources)?;
        }
        Ok(())
    }
}

fn validate_source(source: &Source, previous: &[Source]) -> Result<(), Error> {
    if source.label.is_empty()
        || source.label.len() > 255
        || !source.label.is_ascii()
        || source.label.bytes().any(|b| b.is_ascii_control() || b == b'/')
        || matches!(source.label.as_str(), "." | "..")
        || source.library.as_os_str().is_empty()
    {
        return Err(Error::INVALID);
    }
    if previous.iter().any(|old| old.label == source.label) {
        return Err(Error::CONFLICT);
    }
    Ok(())
}

fn validate_start(
    label: &str,
    previous: &[String],
    sources: &[Source],
) -> Result<(), Error> {
    if previous.iter().any(|old| old == label) {
        return Err(Error::CONFLICT);
    }
    if !sources.iter().any(|source| source.label == label) {
        return Err(Error::UNAVAILABLE);
    }
    Ok(())
}

fn field(args: &mut impl Iterator<Item = String>) -> io::Result<String> {
    args.next().ok_or_else(|| invalid("missing boot option field"))
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
