use reovim_kernel_interface::{
    Error, Outcome, Value,
    caller::{Call, Operation, Status},
    declaration::Declaration,
    resource::AccessId,
};
use reovim_service::protocol::{Action, Frame, Reply};
use std::{
    io::{BufRead, BufReader},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Artifacts {
    pub directory: PathBuf,
}
impl Artifacts {
    pub fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "reovim-process-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        Self { directory }
    }
    pub fn compile(&self, name: &str) -> PathBuf {
        self.compile_source(name, "module.c")
    }
    pub fn compile_source(&self, name: &str, source: &str) -> PathBuf {
        let library = self.directory.join(name);
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let status = Command::new("cc")
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            ])
            .arg("-I")
            .arg(manifest.join("../../host/reference/include"))
            .arg(manifest.join("../../sdk/rust/tests/native").join(source))
            .arg("-o")
            .arg(&library)
            .status()
            .unwrap();
        assert!(status.success());
        library
    }
}
impl Drop for Artifacts {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

pub struct Server {
    child: Child,
    reader: Option<std::thread::JoinHandle<()>>,
    pub peer: Peer,
}
impl Server {
    pub fn start(artifacts: &Artifacts, initial: Option<&Path>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_reovim-server"));
        command
            .args(["127.0.0.1:0", "--trusted-module-root"])
            .arg(&artifacts.directory)
            .stdout(Stdio::piped());
        if let Some(library) = initial {
            command
                .args(["--module", "device"])
                .arg(library)
                .args(["--start", "device"]);
        }
        let child = command.spawn().unwrap();
        let mut server = Self {
            child,
            reader: None,
            peer: Peer {
                address: "127.0.0.1:1".parse().unwrap(),
                token: [0; 16],
                next: 0,
            },
        };
        let stdout = server.child.stdout.take().unwrap();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        server.reader = Some(std::thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(stdout).read_line(&mut line).unwrap();
            let _ = send.send(line);
        }));
        let line = receive.recv_timeout(Duration::from_secs(10)).unwrap();
        server.reader.take().unwrap().join().unwrap();
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.first(), Some(&"ready"));
        server.peer.address = fields[1].parse().unwrap();
        for (index, byte) in server.peer.token.iter_mut().enumerate() {
            *byte =
                u8::from_str_radix(&fields[2][index * 2..index * 2 + 2], 16)
                    .unwrap();
        }
        server
    }
    pub fn terminate(&self) {
        assert!(
            Command::new("kill")
                .args(["-TERM", &self.child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
    }
    pub fn finished(&mut self) -> ExitStatus {
        finished(&mut self.child)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        self.child.wait().unwrap();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
    }
}

pub struct Peer {
    pub address: SocketAddr,
    pub token: [u8; 16],
    next: u64,
}
impl Peer {
    pub fn send(&self, action: Action) -> TcpStream {
        let mut stream = TcpStream::connect(self.address).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let frame = Frame { caller: self.token, action }.encode().unwrap();
        reovim_service::transport::send(&mut stream, &frame).unwrap();
        stream
    }
    pub fn exchange(&self, action: Action) -> Reply {
        let stream = self.send(action);
        let line =
            reovim_service::transport::receive(&mut BufReader::new(stream))
                .unwrap();
        Reply::decode(&line).unwrap()
    }
    pub fn reserve(&mut self) -> u64 {
        self.next += 1;
        let Reply::Reserved(id) = self.exchange(Action::Reserve(self.next))
        else {
            panic!("reserve rejected");
        };
        id
    }
    pub fn call(
        &mut self,
        operation: Operation,
        subject: AccessId,
        command: u32,
        input: Value,
    ) -> Value {
        let request = self.reserve();
        assert_eq!(
            self.exchange(Action::Call(Call {
                request,
                operation,
                subject,
                command,
                expected: None,
                input,
            })),
            Reply::Accepted
        );
        let value = self.outcome(request);
        assert_eq!(self.exchange(Action::Adopt(request)), Reply::Accepted);
        assert_eq!(self.exchange(Action::Release(request)), Reply::Accepted);
        value
    }
    fn outcome(&self, request: u64) -> Value {
        outcome(self, request)
    }
    pub fn entries(
        &mut self,
        path: &[u8],
    ) -> Vec<reovim_kernel_interface::namespace::ResourceEntry> {
        let mut locator = 0_u32.to_le_bytes().to_vec();
        locator.extend_from_slice(
            &u32::try_from(path.len()).unwrap().to_le_bytes(),
        );
        locator.extend_from_slice(path);
        let root = self
            .call(
                Operation::Acquire,
                AccessId(0),
                0,
                Value::copy(&locator, &[]).unwrap(),
            )
            .references[0];
        self.call(
            Operation::SetAccess,
            root,
            0,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        );
        let mut input = 0_u64.to_le_bytes().to_vec();
        input.extend_from_slice(&65536_u64.to_le_bytes());
        let listed = self.call(
            Operation::Entries,
            root,
            0,
            Value::copy(&input, &[]).unwrap(),
        );
        decode_entries(&listed)
    }

    pub fn bind_path(&mut self, path: &[u8]) -> (AccessId, Declaration) {
        let mut locator = 0_u32.to_le_bytes().to_vec();
        locator.extend_from_slice(
            &u32::try_from(path.len()).unwrap().to_le_bytes(),
        );
        locator.extend_from_slice(path);
        self.bind_locator(&locator)
    }
    pub fn bind_locator(&mut self, locator: &[u8]) -> (AccessId, Declaration) {
        let opened = self
            .call(
                Operation::Acquire,
                AccessId(0),
                0,
                Value::copy(locator, &[]).unwrap(),
            )
            .references[0];
        let mut page = 0_u64.to_le_bytes().to_vec();
        page.extend_from_slice(&65536_u64.to_le_bytes());
        let apis = self.call(
            Operation::Interfaces,
            opened,
            0,
            Value::copy(&page, &[]).unwrap(),
        );
        let length =
            u32::from_le_bytes(apis.data[4..8].try_into().unwrap()) as usize;
        let declaration =
            Declaration::decode(&apis.data[8..8 + length]).unwrap();
        let bound = self
            .call(
                Operation::Bind,
                opened,
                0,
                Value::copy(&apis.data[4..8 + length], &[]).unwrap(),
            )
            .references[0];
        self.call(
            Operation::SetAccess,
            opened,
            0,
            Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
        );
        (bound, declaration)
    }
    pub fn await_closed(&mut self) {
        await_closed(self);
    }
}
fn success(outcome: &Outcome) -> Value {
    let Outcome::Ok(value) = outcome else {
        panic!("call failed: {outcome:?}")
    };
    value.clone()
}

pub fn finished(child: &mut Child) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "process did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn outcome(peer: &Peer, request: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Reply::Status(Status::Outcome(outcome)) =
            peer.exchange(Action::Result(request))
        {
            return success(outcome.as_ref());
        }
        assert!(Instant::now() < deadline, "request did not finish");
    }
}

fn await_closed(peer: &mut Peer) {
    peer.next += 1;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match peer.exchange(Action::Reserve(peer.next)) {
            Reply::Error(Error::CLOSED) => return,
            Reply::Reserved(id) => {
                peer.exchange(Action::Release(id));
                peer.next += 1;
            }
            other => panic!("unexpected shutdown evidence: {other:?}"),
        }
        assert!(Instant::now() < deadline, "shutdown did not fence admission");
    }
}

fn decode_entries(
    value: &Value,
) -> Vec<reovim_kernel_interface::namespace::ResourceEntry> {
    use reovim_kernel_interface::{ResourceId, namespace::ResourceEntry};
    let count = u32::from_le_bytes(value.data[..4].try_into().unwrap());
    let mut bytes = &value.data[4..];
    let mut entries = Vec::new();
    for _ in 0..count {
        let length =
            u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let name = bytes[4..4 + length].into();
        bytes = &bytes[4 + length..];
        let resource = ResourceId {
            epoch: bytes[..16].try_into().unwrap(),
            serial: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        };
        bytes = &bytes[24..];
        entries.push(ResourceEntry { name, resource });
    }
    assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 0);
    entries
}
