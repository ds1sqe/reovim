mod invocation;
mod requests;
mod shell;

use reovim_service::transport::{response, send};
use std::{
    env,
    io::{self, BufReader},
    net::TcpStream,
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let address = args.next().unwrap_or_else(|| "127.0.0.1:7878".into());
    let mut fields: Vec<_> = args.collect();
    if fields.as_slice() == ["--connect"] {
        println!("{}", connect(&address)?);
        return Ok(());
    }
    let selected = if fields.first().is_some_and(|field| field == "--client") {
        if fields.len() < 2 {
            return Err("--client needs a token".into());
        }
        let token = fields.remove(1);
        fields.remove(0);
        Some(token)
    } else {
        match env::var("REOVIM_CLIENT") {
            Ok(token) => Some(token),
            Err(env::VarError::NotPresent) => None,
            Err(error) => return Err(error.into()),
        }
    };
    let client = selected.map_or_else(|| connect(&address), Ok)?;
    if requests::control(&address, &client, &fields)? {
        return Ok(());
    }
    if fields.is_empty() {
        return shell::run(&address, &client).map_err(Into::into);
    }
    let mut shell = shell::Shell::new(&address, &client);
    shell.execute(&fields.iter().map(String::as_str).collect::<Vec<_>>())?;
    Ok(())
}

fn connect(address: &str) -> io::Result<String> {
    let reply = exchange(address, &["open".into()])?;
    let token = reply
        .strip_prefix("ok\t")
        .filter(|value| {
            value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
        })
        .ok_or_else(|| io::Error::other("invalid client identity"))?;
    Ok(token.into())
}

fn encode_frame(fields: &[String]) -> io::Result<String> {
    if fields.is_empty()
        || fields.iter().any(|f| f.contains(['\t', '\r', '\n']))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid command arguments",
        ));
    }
    let frame = format!("v1\t{}", fields.join("\t"));
    if frame.len() >= reovim_service::MAX_LINE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame too large",
        ));
    }
    Ok(frame)
}
fn frame(address: &str, fields: &[String]) -> io::Result<String> {
    send_frame(address, &encode_frame(fields)?)
}
fn send_frame(address: &str, frame: &str) -> io::Result<String> {
    let mut stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    send(&mut stream, frame)?;
    response(&mut BufReader::new(stream))
}

fn exchange(address: &str, fields: &[String]) -> io::Result<String> {
    accept(frame(address, fields)?)
}
fn accept(reply: String) -> io::Result<String> {
    if reply == "ok" || reply.starts_with("ok\t") {
        return Ok(reply);
    }
    let kind = if reply.starts_with("error\trejected\t2\t") {
        io::ErrorKind::NotFound
    } else {
        io::ErrorKind::Other
    };
    Err(io::Error::new(kind, reply))
}
