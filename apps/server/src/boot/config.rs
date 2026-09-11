//! Explicit process configuration; module identity comes from native metadata.

use alloc::{string::String, vec::Vec};
use reovim_kernel_interface::{Contract, Message, provider::Permissions};
use std::{io, path::PathBuf};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Scope {
    Server,
    Client,
}

pub struct Source {
    pub scope: Scope,
    pub label: String,
    pub library: PathBuf,
    pub mount: String,
    pub grants: Vec<(String, Permissions)>,
}
pub struct Session {
    pub model: String,
    pub initial: Message,
}
#[derive(Default)]
pub struct Config {
    pub sources: Vec<Source>,
    pub start: Vec<Contract>,
    pub client_start: Vec<Contract>,
    pub session: Option<Session>,
    pub output: Option<PathBuf>,
}

impl Config {
    /// Parses native declarations and required startup contracts.
    /// # Errors
    /// Rejects unknown options, missing fields, invalid schemas or grant modes.
    pub fn parse(
        mut arguments: impl Iterator<Item = String>,
    ) -> io::Result<Self> {
        let mut config = Self::default();
        while let Some(option) = arguments.next() {
            config.option(&option, &mut arguments)?;
        }
        Ok(config)
    }

    fn option(
        &mut self,
        option: &str,
        args: &mut impl Iterator<Item = String>,
    ) -> io::Result<()> {
        match option {
            "--module" | "--client-module" => self.source(option, args)?,
            "--grant" => self.grant(args)?,
            "--start" => required(&mut self.start, args)?,
            "--client-start" => required(&mut self.client_start, args)?,
            "--session" if self.session.is_none() => {
                self.session = Some(session(args)?);
            }
            "--export-root" if self.output.is_none() => {
                self.output = Some(field(args)?.into());
            }
            _ => return Err(invalid("unknown boot option")),
        }
        Ok(())
    }

    fn source(
        &mut self,
        option: &str,
        args: &mut impl Iterator<Item = String>,
    ) -> io::Result<()> {
        if self.sources.len() == 64 {
            return Err(invalid("module capacity"));
        }
        self.sources.push(Source {
            scope: if option == "--module" {
                Scope::Server
            } else {
                Scope::Client
            },
            label: field(args)?,
            library: field(args)?.into(),
            mount: field(args)?,
            grants: Vec::new(),
        });
        Ok(())
    }

    fn grant(
        &mut self,
        args: &mut impl Iterator<Item = String>,
    ) -> io::Result<()> {
        let source = self
            .sources
            .last_mut()
            .ok_or_else(|| invalid("grant needs a module"))?;
        if source.grants.len() == 64 {
            return Err(invalid("grant capacity"));
        }
        let path = field(args)?;
        let permissions = match field(args)?.as_str() {
            "read" => Permissions::READ,
            "write" => Permissions::WRITE,
            "all" => Permissions::ALL,
            _ => return Err(invalid("expected read, write or all grant")),
        };
        source.grants.push((path, permissions));
        Ok(())
    }
}
fn field(args: &mut impl Iterator<Item = String>) -> io::Result<String> {
    args.next().ok_or_else(|| invalid("missing boot option field"))
}
fn contract(args: &mut impl Iterator<Item = String>) -> io::Result<Contract> {
    let name = field(args)?;
    if name.is_empty() || name.len() > 1024 {
        return Err(invalid("invalid contract name"));
    }
    let schema =
        field(args)?.parse().map_err(|_| invalid("invalid contract schema"))?;
    Ok(Contract::new(&name, schema))
}
fn required(
    selected: &mut Vec<Contract>,
    args: &mut impl Iterator<Item = String>,
) -> io::Result<()> {
    if selected.len() == 64 {
        return Err(invalid("startup capacity"));
    }
    selected.push(contract(args)?);
    Ok(())
}
fn session(args: &mut impl Iterator<Item = String>) -> io::Result<Session> {
    let model = field(args)?;
    let contract = contract(args)?;
    let data = reovim_service::unhex(&field(args)?)
        .ok_or_else(|| invalid("invalid initial hex"))?;
    Ok(Session { model, initial: Message::new(contract, &data) })
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
