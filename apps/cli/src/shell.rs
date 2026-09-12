use reovim_call::{DefaultCall, Output};
use std::io::{self, IsTerminal, Write};

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
        let path = resolve(&cwd, invocation.path);
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
        let reply = self.exchange(&[
            "read".into(),
            "env/CWD".into(),
            "resource.bytes".into(),
            "1".into(),
        ]);
        let reply = match reply {
            Ok(reply) => reply,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(String::new());
            }
            Err(error) => return Err(error),
        };
        String::from_utf8(data_bytes(&reply)?).map_err(io::Error::other)
    }

    fn prompt(&self) -> io::Result<()> {
        match self.cwd() {
            Ok(cwd) => print!("resource:/{}> ", cwd.escape_debug()),
            Err(error) => {
                eprintln!("CWD: {error}; use an absolute path to repair it");
                print!("resource:?> ");
            }
        }
        io::stdout().flush()
    }
}

pub fn run(address: &str, client: &str) -> io::Result<()> {
    let mut shell = Shell::new(address, client);
    let terminal = io::stdin().is_terminal();
    while shell.running {
        if terminal {
            shell.prompt()?;
        }
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
