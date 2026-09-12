use reovim_call::{DefaultCall, Output};
use std::io::{self, IsTerminal, Write};

mod completion;
mod interactive;

pub struct Shell {
    address: String,
    client: String,
    running: bool,
}

impl Shell {
    pub fn new(address: &str, client: &str) -> Self {
        Self { address: address.into(), client: client.into(), running: true }
    }

    pub fn execute(&mut self, fields: &[&str]) -> io::Result<()> {
        if fields.is_empty() {
            return Ok(());
        }
        let invocation = crate::invocation::parse(fields)?;
        let cwd =
            if invocation.needs_cwd() { self.cwd()? } else { String::new() };
        let path = self.command(&cwd, invocation.path)?;
        let (name, version, data, output) = match invocation.contract {
            Some((name, version)) => {
                (name.into(), version, Vec::new(), Output::Message)
            }
            None => self.default(&path, invocation.args)?,
        };
        let data = invocation.hex.map_or_else(
            || Ok(data),
            |hex| {
                reovim_service::unhex(hex)
                    .ok_or_else(|| io::Error::other("invalid hex data"))
            },
        )?;
        let mut fields = vec![
            "write".into(),
            path,
            name,
            version.to_string(),
            reovim_service::hex(&data),
        ];
        if let Some(references) = invocation.references {
            fields.push(
                references
                    .split(',')
                    .map(|value| resolve(&cwd, value))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        let reply = self.exchange(&fields)?;
        match output {
            Output::Message => println!("{reply}"),
            Output::Bytes => io::stdout().write_all(&data_bytes(&reply)?)?,
            Output::Close => self.running = false,
        }
        Ok(())
    }

    fn exchange(&self, fields: &[String]) -> io::Result<String> {
        crate::requests::call(&self.address, &self.client, fields)
    }

    fn default(
        &self,
        path: &str,
        args: &[&str],
    ) -> io::Result<(String, u32, Vec<u8>, Output)> {
        let reply = self.exchange(&[
            "read".into(),
            format!("{path}/info"),
            "resource.bytes".into(),
            "1".into(),
        ])?;
        let default = DefaultCall::parse(&data_bytes(&reply)?)
            .map_err(io::Error::other)?;
        let data = default.data(args).map_err(io::Error::other)?;
        Ok((
            default.contract.name.into(),
            default.contract.version,
            data.into(),
            default.output,
        ))
    }

    fn cwd(&self) -> io::Result<String> {
        Ok(self.environment("CWD")?.unwrap_or_default())
    }

    fn environment(&self, name: &str) -> io::Result<Option<String>> {
        let reply = self.exchange(&[
            "read".into(),
            format!("env/{name}"),
            "resource.bytes".into(),
            "1".into(),
        ]);
        let reply = match reply {
            Ok(reply) => reply,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        String::from_utf8(data_bytes(&reply)?)
            .map(Some)
            .map_err(io::Error::other)
    }

    fn command(&self, cwd: &str, name: &str) -> io::Result<String> {
        if name.starts_with('@')
            || name.contains('/')
            || matches!(name, "." | "..")
        {
            return Ok(resolve(cwd, name));
        }
        let directories = self.directories()?;
        self.search(&directories, name)
    }

    fn directories(&self) -> io::Result<Vec<String>> {
        let path = self.environment("PATH")?.unwrap_or_else(|| "/bin".into());
        let directories: Vec<_> =
            path.split(':').filter(|p| !p.is_empty()).collect();
        if directories.len() > 32
            || path.len() > 4096
            || directories.iter().any(|p| !p.starts_with('/'))
        {
            return Err(io::Error::other(
                "PATH requires at most 32 absolute Resource directories",
            ));
        }
        Ok(directories.into_iter().map(String::from).collect())
    }

    fn search(&self, directories: &[String], name: &str) -> io::Result<String> {
        for directory in directories {
            let path = reovim_call::path(directory, name);
            let result = self.exchange(&["resolve".into(), path]);
            match result {
                Ok(reply) => return resolved(&reply),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("command not found in PATH: {name}"),
        ))
    }

    fn prompt(&self) -> String {
        match self.cwd() {
            Ok(cwd) => format!("resource:/{}> ", cwd.escape_debug()),
            Err(error) => {
                eprintln!("CWD: {error}; use an absolute path to repair it");
                "resource:?> ".into()
            }
        }
    }
}

fn resolved(reply: &str) -> io::Result<String> {
    let reference = reply
        .strip_prefix("ok\t@")
        .ok_or_else(|| io::Error::other("invalid resolve reply"))?;
    let id: u64 = reference.parse().map_err(io::Error::other)?;
    Ok(format!("@{id}"))
}

pub fn run(address: &str, client: &str) -> io::Result<()> {
    let mut shell = Shell::new(address, client);
    if io::stdin().is_terminal() {
        return interactive::run(shell);
    }
    while shell.running {
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if let Err(error) = shell.execute(&fields) {
            eprintln!("{error}");
        }
    }
    Ok(())
}

fn resolve(cwd: &str, path: &str) -> String {
    if path.starts_with('@') {
        path.into()
    } else {
        reovim_call::path(cwd, path)
    }
}

fn data_bytes(reply: &str) -> io::Result<Vec<u8>> {
    reply
        .split('\t')
        .nth(3)
        .and_then(reovim_service::unhex)
        .ok_or_else(|| io::Error::other("invalid Message reply"))
}
