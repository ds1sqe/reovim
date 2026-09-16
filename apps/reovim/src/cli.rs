//! `reovim cli`: one link, one `Endpoint`, commands by path or rd.
//! One command runs and exits; none reads commands from stdin until EOF
//! or `quit`, on the same link.

use std::io::{Read, Write};

use reovim_host::link::{self, Link};
use reovim_kernel_interface::wire::{Endpoint, Message};
use reovim_kernel_interface::{
    Decl, Error, Handle, Hash, Method, Op, Out, Rd, Reply, Rights, Ty,
};

use crate::Fail;

/// Known to every client; everything else is `lookup`.
const ROOT: Rd = Rd(0);
const CHUNK: u32 = 64 * 1024;

const USAGE: &str = "\
lookup <target>                 rd
info <target>                   name, flags, times, apis
children <target>               rd and name per line
read <target>                   bytes to stdout
write <target> [bytes]          bytes, or stdin without them, at 0
call <target> <method> [arg]    output decoded by the declaration
quit                            end an interactive session
<target> is a path from / or rd:<n>";

/// Run `command`, or an interactive session when it is empty.
pub fn run(socket: &str, command: &[String]) -> Result<(), Fail> {
    let mut client = Client::connect(socket)?;
    if command.is_empty() {
        return interactive(&mut client);
    }
    client.command(command)
}

/// One command per stdin line until EOF or `quit`; an error ends the
/// line, not the session.
fn interactive(client: &mut Client) -> Result<(), Fail> {
    let stdin = std::io::stdin();
    let mut line = String::new();
    while stdin.read_line(&mut line).map_err(|_| Fail::Wire)? != 0 {
        let words: Vec<String> =
            line.split_whitespace().map(str::to_string).collect();
        line.clear();
        match words.first().map(String::as_str) {
            None => {}
            Some("quit") => break,
            Some(_) => report(client.command(&words)),
        }
    }
    Ok(())
}

fn report(result: Result<(), Fail>) {
    if let Err(fail) = result {
        eprintln!("reovim: {fail}");
    }
}

/// The next reply on the link; bytes that are not one are dropped.
fn next_reply(end: &Link, endpoint: &mut Endpoint) -> Result<Reply<Out>, Fail> {
    loop {
        let bytes = link::recv(end)?;
        if let Some((_, Message::Reply(reply))) = endpoint.receive(&bytes) {
            return Ok(reply);
        }
    }
}

fn stdin_bytes() -> Result<Vec<u8>, Fail> {
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes).map_err(|_| Fail::Wire)?;
    Ok(bytes)
}

struct Client {
    end: Link,
    endpoint: Endpoint,
}

impl Client {
    fn connect(socket: &str) -> Result<Self, Fail> {
        let end = link::open(socket)?;
        let mut client = Self { end, endpoint: Endpoint::default() };
        let connect =
            Message::Connect { credential: Vec::new(), session: None };
        client.exchange(&connect)?;
        Ok(client)
    }

    /// One message out, the reply to it back. The cli sends once; a
    /// lost reply ends the link with `Io` rather than waiting.
    fn exchange(&mut self, message: &Message) -> Result<Out, Fail> {
        let (_, bytes) = self.endpoint.send(message);
        link::send(&self.end, &bytes)?;
        match next_reply(&self.end, &mut self.endpoint)? {
            Reply::Ready(Ok(out)) => Ok(out),
            Reply::Ready(Err(error)) => Err(Fail::Kernel(error)),
            Reply::Pending(token) => self.wait(token),
        }
    }

    fn wait(
        &mut self,
        token: reovim_kernel_interface::Token,
    ) -> Result<Out, Fail> {
        loop {
            match self.exchange(&Message::Request(Op::Ask { token })) {
                Err(Fail::Kernel(Error::Busy)) => std::thread::yield_now(),
                other => return other,
            }
        }
    }

    fn request(&mut self, op: Op) -> Result<Out, Fail> {
        self.exchange(&Message::Request(op))
    }

    fn rd(&mut self, op: Op) -> Result<Rd, Fail> {
        match self.request(op)? {
            Out::Rd(rd) => Ok(rd),
            _ => Err(Fail::Wire),
        }
    }

    fn lookup(&mut self, rd: Rd, name: &str) -> Result<Rd, Fail> {
        self.rd(Op::Lookup { rd, name: name.to_string() })
    }

    fn open(&mut self, rd: Rd, rights: Rights) -> Result<Handle, Fail> {
        match self.request(Op::Open { rd, rights })? {
            Out::Handle(h) => Ok(h),
            _ => Err(Fail::Wire),
        }
    }

    fn close(&mut self, h: Handle) -> Result<(), Fail> {
        self.request(Op::Close { h }).map(|_| ())
    }

    fn info(&mut self, rd: Rd) -> Result<reovim_kernel_interface::Info, Fail> {
        match self.request(Op::Info { rd })? {
            Out::Info(info) => Ok(info),
            _ => Err(Fail::Wire),
        }
    }

    /// `/a/b/c` by lookup from the root, or `rd:<n>`.
    fn target(&mut self, target: &str) -> Result<Rd, Fail> {
        if let Some(number) = target.strip_prefix("rd:") {
            return number.parse().map(Rd).map_err(|_| usage());
        }
        let Some(path) = target.strip_prefix('/') else { return Err(usage()) };
        let mut rd = ROOT;
        for name in path.split('/').filter(|n| !n.is_empty()) {
            rd = self.lookup(rd, name)?;
        }
        Ok(rd)
    }

    /// Everything readable at `rd`.
    fn read_all(&mut self, rd: Rd) -> Result<Vec<u8>, Fail> {
        let h = self.open(rd, Rights::READ)?;
        let mut out = Vec::new();
        loop {
            let at = out.len() as u64;
            match self.request(Op::Read { h, at, len: CHUNK })? {
                Out::Bytes(bytes) if bytes.is_empty() => break,
                Out::Bytes(bytes) => out.extend_from_slice(&bytes),
                _ => return Err(Fail::Wire),
            }
        }
        self.close(h)?;
        Ok(out)
    }

    fn command(&mut self, words: &[String]) -> Result<(), Fail> {
        let word =
            |i: usize| words.get(i).map(String::as_str).ok_or_else(usage);
        let target = self.target(word(1)?)?;
        match (word(0)?, words.len()) {
            ("lookup", 2) => println!("{}", target.0),
            ("info", 2) => self.show_info(target)?,
            ("children", 2) => self.show_children(target)?,
            ("read", 2) => {
                let bytes = self.read_all(target)?;
                std::io::stdout().write_all(&bytes).map_err(|_| Fail::Wire)?;
            }
            ("write", 2 | 3) => self.write(target, words.get(2))?,
            ("call", 3 | 4) => self.call(target, word(2)?, words.get(3))?,
            _ => return Err(usage()),
        }
        Ok(())
    }

    fn show_info(&mut self, rd: Rd) -> Result<(), Fail> {
        let info = self.info(rd)?;
        let flags = ["READ", "WRITE", "CONTAINER"]
            .iter()
            .zip([1, 2, 4])
            .filter(|(_, bit)| info.flags.bits() & bit != 0)
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join("|");
        let apis: Vec<String> =
            info.apis.iter().map(|h| format!("{:016x}", h.0)).collect();
        println!(
            "name={} flags={flags} ctime={} mtime={} atime={} apis={}",
            info.name.unwrap_or_default(),
            info.ctime,
            info.mtime,
            info.atime,
            apis.join(",")
        );
        Ok(())
    }

    fn show_children(&mut self, rd: Rd) -> Result<(), Fail> {
        let Out::Rds(rds) = self.request(Op::Children { rd })? else {
            return Err(Fail::Wire);
        };
        for child in rds {
            let name = self.info(child)?.name.unwrap_or_default();
            println!("{} {name}", child.0);
        }
        Ok(())
    }

    fn write(&mut self, rd: Rd, bytes: Option<&String>) -> Result<(), Fail> {
        let bytes = bytes
            .map_or_else(stdin_bytes, |text| Ok(text.as_bytes().to_vec()))?;
        let h = self.open(rd, Rights::WRITE)?;
        let Out::Len(len) = self.request(Op::Write { h, at: 0, bytes })? else {
            return Err(Fail::Wire);
        };
        self.close(h)?;
        println!("{len}");
        Ok(())
    }

    /// The declaration at `/sys/decls/<hash>`.
    fn decl(&mut self, hash: Hash) -> Result<Decl, Fail> {
        let name = format!("{:016x}", hash.0);
        let rd = self.target(&format!("/sys/decls/{name}"))?;
        let bytes = self.read_all(rd)?;
        Ok(Decl::parse(&bytes)?)
    }

    /// The first declaration `rd` offers that names `method`.
    fn method(&mut self, rd: Rd, method: &str) -> Result<(Hash, Method), Fail> {
        let mut decls = Vec::new();
        for hash in self.info(rd)?.apis {
            decls.push((hash, self.decl(hash)?));
        }
        let named = |(hash, decl): (Hash, Decl)| {
            decl.methods
                .into_iter()
                .find(|m| m.name == method)
                .map(|m| (hash, m))
        };
        decls.into_iter().find_map(named).ok_or_else(|| {
            Fail::Usage(format!("no method `{method}` on rd {}", rd.0))
        })
    }

    fn call(
        &mut self,
        rd: Rd,
        name: &str,
        arg: Option<&String>,
    ) -> Result<(), Fail> {
        let (api, method) = self.method(rd, name)?;
        let input = encode_arg(&method.input, arg.map(String::as_str))?;
        let h = self.open(rd, method.right)?;
        let op = Op::Call { h, api, method: method.number, input };
        let Out::Bytes(output) = self.request(op)? else {
            return Err(Fail::Wire);
        };
        self.close(h)?;
        show_output(&method.output, &output)
    }
}

fn usage() -> Fail {
    Fail::Usage(USAGE.to_string())
}

/// `arg` in the declared input encoding; a missing or unfit argument
/// fails here, before anything is sent.
fn encode_arg(ty: &Ty, arg: Option<&str>) -> Result<Vec<u8>, Fail> {
    let bad = || Fail::Usage(format!("argument does not fit {ty:?}"));
    let number = || arg.ok_or_else(bad)?.parse::<u64>().map_err(|_| bad());
    let bytes = match (ty, arg) {
        (Ty::Unit, None) => Vec::new(),
        (Ty::Rd | Ty::U64, Some(_)) => number()?.to_le_bytes().to_vec(),
        (Ty::U32, Some(_)) => {
            u32::try_from(number()?).map_err(|_| bad())?.to_le_bytes().to_vec()
        }
        (Ty::Utf8(_) | Ty::Bytes(_), Some(text)) => {
            let len = u32::try_from(text.len()).map_err(|_| bad())?;
            let mut out = len.to_le_bytes().to_vec();
            out.extend_from_slice(text.as_bytes());
            out
        }
        _ => return Err(bad()),
    };
    if !reovim_kernel_interface::value::fits(&bytes, ty) {
        return Err(bad());
    }
    Ok(bytes)
}

fn show_output(ty: &Ty, output: &[u8]) -> Result<(), Fail> {
    match ty {
        Ty::Unit => {}
        Ty::Rd | Ty::U64 => {
            let bytes = output.try_into().map_err(|_| Fail::Wire)?;
            println!("{}", u64::from_le_bytes(bytes));
        }
        Ty::U32 => {
            let bytes = output.try_into().map_err(|_| Fail::Wire)?;
            println!("{}", u32::from_le_bytes(bytes));
        }
        Ty::Utf8(_) => {
            let text = output.get(4..).ok_or(Fail::Wire)?;
            println!("{}", String::from_utf8_lossy(text));
        }
        _ => std::io::stdout().write_all(output).map_err(|_| Fail::Wire)?,
    }
    Ok(())
}
