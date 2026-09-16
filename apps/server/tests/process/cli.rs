use super::support::{Artifacts, Server, finished};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{Receiver, channel},
    thread::JoinHandle,
    time::Duration,
};

struct Cli {
    child: Child,
    output: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}
impl Cli {
    fn start(binary: &Path, server: &Server, journal: &Path) -> Self {
        let child = Command::new(binary)
            .arg(server.peer.address.to_string())
            .arg(reovim_service::hex(&server.peer.token))
            .arg(journal)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let (send, output) = channel();
        let mut cli = Self { child, output, reader: None };
        let stdout = cli.child.stdout.take().unwrap();
        cli.reader = Some(std::thread::spawn(move || forward(stdout, &send)));
        assert_eq!(cli.line(), "ready");
        cli
    }
    fn line(&self) -> String {
        self.output.recv_timeout(Duration::from_secs(40)).unwrap()
    }
    fn command(&mut self, command: &str) -> Vec<String> {
        writeln!(self.child.stdin.as_mut().unwrap(), "{command}").unwrap();
        self.child.stdin.as_mut().unwrap().flush().unwrap();
        collect(&self.output)
    }
    fn exit(&mut self) {
        self.command("exit");
        assert!(finished(&mut self.child).success());
        self.reader.take().unwrap().join().unwrap();
    }
}
impl Drop for Cli {
    fn drop(&mut self) {
        let _ = self.child.kill();
        self.child.wait().unwrap();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
    }
}
fn collect(output: &Receiver<String>) -> Vec<String> {
    let mut lines = Vec::new();
    loop {
        let line = output.recv_timeout(Duration::from_secs(40)).unwrap();
        assert_ne!(line, "failed", "CLI command failed");
        if line == "ok" {
            return lines;
        }
        lines.push(line);
    }
}
fn build() -> PathBuf {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../product/Cargo.toml");
    let executable = std::env::current_exe().unwrap();
    let directory = executable.parent().unwrap().parent().unwrap();
    let profile = match directory.file_name().unwrap().to_str().unwrap() {
        "debug" => "dev",
        profile => profile,
    };
    let output = Command::new(env!("CARGO"))
        .args(["build", "--manifest-path"])
        .arg(&manifest)
        .args(["--target-dir"])
        .arg(directory.parent().unwrap())
        .args(["--profile", profile])
        .env_remove("CARGO_BUILD_TARGET")
        .args(["-p", "reovim-cli", "--features", "std", "--bin", "reovim-cli"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    directory.join("reovim-cli")
}
fn method(lines: &[String], operation: &str) -> String {
    lines
        .iter()
        .find_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.first() == Some(&"api") && fields[4].starts_with(operation))
                .then(|| fields[3].to_owned())
        })
        .unwrap()
}
fn data(lines: &[String]) -> Vec<u8> {
    let line = lines.iter().find(|line| line.starts_with("data ")).unwrap();
    reovim_service::unhex(line.split_whitespace().nth(1).unwrap()).unwrap()
}
#[test]
fn unchanged_cli_discovers_backend_built_after_both_processes_start() {
    use std::os::unix::ffi::OsStrExt;
    let binary = build();
    let artifacts = Artifacts::new();
    let mut server = Server::start(&artifacts, None);
    let journal = artifacts.directory.join("caller.log");
    let mut cli = Cli::start(&binary, &server, &journal);
    let library = artifacts.compile("after-cli-ready.so");
    let admission = method(&cli.command("inspect /load"), "Exec");
    let path = library.as_os_str().as_bytes();
    let mut input = u32::try_from(path.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(path);
    let package = data(&cli.command(&format!(
        "invoke /load {admission} {}",
        reovim_service::hex(&input)
    )));
    std::fs::remove_file(library).unwrap();
    let package = format!("@{}", reovim_service::hex(&package));
    let activation =
        method(&cli.command(&format!("inspect {package}")), "Exec");
    cli.command(&format!("invoke {package} {activation} -"));
    let entries = cli.command("ls /loaded");
    let mut observed = Vec::new();
    for entry in entries {
        let fields: Vec<_> = entry.split_whitespace().collect();
        let name = reovim_service::unhex(fields[1]).unwrap();
        let identity = fields[3];
        let read =
            method(&cli.command(&format!("inspect {identity}")), "Ioctl");
        let value = data(&cli.command(&format!("invoke {identity} {read} -")));
        observed.push((name, u64::from_le_bytes(value.try_into().unwrap())));
    }
    assert_eq!(
        observed,
        vec![(b"first".to_vec(), 81), (b"second".to_vec(), 82)]
    );
    cli.exit();
    let mut cli = Cli::start(&binary, &server, &journal);
    assert_eq!(cli.command("receipts"), vec!["[]"]);
    assert_eq!(
        cli.command("ls /loaded").first().unwrap().split_whitespace().nth(1),
        Some(reovim_service::hex(b"first").as_str())
    );
    cli.exit();
    server.terminate();
    assert!(server.finished().success());
}

fn forward(
    stdout: std::process::ChildStdout,
    send: &std::sync::mpsc::Sender<String>,
) {
    for line in BufReader::new(stdout).lines() {
        if send.send(line.unwrap()).is_err() {
            return;
        }
    }
}

fn activate(cli: &mut Cli, library: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let admission = method(&cli.command("inspect /load"), "Exec");
    let path = library.as_os_str().as_bytes();
    let mut input = u32::try_from(path.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(path);
    let package = data(&cli.command(&format!(
        "invoke /load {admission} {}",
        reovim_service::hex(&input)
    )));
    std::fs::remove_file(library).unwrap();
    let package = format!("@{}", reovim_service::hex(&package));
    let activation =
        method(&cli.command(&format!("inspect {package}")), "Exec");
    cli.command(&format!("invoke {package} {activation} -"));
}
fn identity(row: &str) -> &str {
    row.split_whitespace().nth(3).unwrap()
}
#[test]
fn running_cli_materializes_native_namespace_children_after_activation() {
    let binary = build();
    let artifacts = Artifacts::new();
    let mut server = Server::start(&artifacts, None);
    let journal = artifacts.directory.join("caller.log");
    let mut cli = Cli::start(&binary, &server, &journal);
    let library =
        artifacts.compile_source("namespace.so", "namespace_module.c");
    activate(&mut cli, &library);
    let mount = cli.command("ls /loaded");
    let directory = identity(&mount[0]);
    let children = cli.command(&format!("ls {directory}"));
    let child = identity(&children[0]);
    assert_eq!(child, identity(&children[1]));
    let read = method(&cli.command(&format!("inspect {child}")), "Ioctl");
    // Inspection closed its access. Listing carries no implementation lease;
    // rediscover the current Kernel lifetime before the separate invocation.
    let children = cli.command(&format!("ls {directory}"));
    let current = identity(&children[0]);
    assert_ne!(child, current);
    let value = data(&cli.command(&format!("invoke {current} {read} -")));
    assert_eq!(u64::from_le_bytes(value.try_into().unwrap()), 41);
    let read = method(&cli.command("inspect /loaded/directory/disk"), "Ioctl");
    let value =
        data(&cli.command(&format!("invoke /loaded/directory/alias {read} -")));
    assert_eq!(u64::from_le_bytes(value.try_into().unwrap()), 41);
    cli.exit();
    server.terminate();
    assert!(server.finished().success());
}
