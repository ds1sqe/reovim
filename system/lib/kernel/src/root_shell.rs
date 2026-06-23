//! Root daemon parser and command execution primitives.
//!
//! This module keeps the OS-mode shell proof intentionally small: bounded input
//! tokenization, a minimal bash-like vocabulary, and static output through the
//! injected console write callback in `rootd`.

use {
    crate::{
        klog,
        rootd::{HardwareProbeResult, PayloadDescriptor, PayloadLaunchResult, RootDaemon},
        vfs::{self, Directory, File, Node, PathBuf, VfsError},
    },
    reovim_uapi_system::{BootInfo, DeviceEntry},
};

const MAX_ARGS: usize = 8;
const MAX_TOKEN_BYTES: usize = 64;

/// Mutable root-shell state carried across command dispatch.
pub struct RootShellSession {
    cwd: PathBuf,
}

impl RootShellSession {
    /// Starts a shell session at `/`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cwd: PathBuf::root(),
        }
    }

    /// Current working directory.
    #[must_use]
    pub fn cwd(&self) -> &str {
        self.cwd.as_str()
    }

    fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }
}

impl Default for RootShellSession {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ParseStatus {
    Empty,
    Ok,
    TooLong,
    TooMany,
}

#[derive(Clone, Copy, Debug)]
enum QuoteState {
    None,
    Single,
    Double,
}

/// Root-shell command execution status for kernel audit logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RootCommandStatus {
    /// The line carried no executable command.
    Empty,
    /// The command completed normally.
    Ok,
    /// The command was rejected or reported a command-level failure.
    Error,
    /// The command requested root-daemon shutdown.
    Halt,
}

impl RootCommandStatus {
    /// Stable text used in `klog` records.
    #[must_use]
    pub const fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Empty => b"empty",
            Self::Ok => b"ok",
            Self::Error => b"error",
            Self::Halt => b"halt",
        }
    }
}

/// Result from executing one root-shell line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RootCommandResult {
    status: RootCommandStatus,
}

impl RootCommandResult {
    const fn new(status: RootCommandStatus) -> Self {
        Self { status }
    }

    /// Returns the command status.
    #[must_use]
    pub const fn status(self) -> RootCommandStatus {
        self.status
    }

    /// Whether this command requested root-daemon shutdown.
    #[must_use]
    pub const fn should_halt(self) -> bool {
        matches!(self.status, RootCommandStatus::Halt)
    }
}

#[derive(Clone, Copy)]
struct Arg {
    bytes: [u8; MAX_TOKEN_BYTES],
    len: usize,
}

impl Arg {
    const fn new() -> Self {
        Self {
            bytes: [0u8; MAX_TOKEN_BYTES],
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn push_byte(&mut self, byte: u8, status: &mut ParseStatus) {
        if self.len >= MAX_TOKEN_BYTES {
            *status = ParseStatus::TooLong;
            return;
        }
        self.bytes[self.len] = byte;
        self.len += 1;
    }

    fn as_str(&self) -> &str {
        // Tokens are ASCII-only in shell fixtures and in current CLI transcripts.
        // SAFETY: this boundary intentionally accepts kernel-shell bytes and avoids
        // UTF-8 validation in hot-path parsing. If command sources include
        // non-ASCII by mistake, output will still be byte-preserving.
        unsafe { core::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }
}

#[derive(Clone, Copy)]
struct ParsedLine {
    args: [Arg; MAX_ARGS],
    argc: usize,
}

impl ParsedLine {
    const fn empty() -> Self {
        Self {
            args: [Arg::new(); MAX_ARGS],
            argc: 0,
        }
    }
}

fn push_arg(args: &mut ParsedLine, arg: &mut Arg, status: &mut ParseStatus) {
    if arg.len == 0 {
        return;
    }
    if args.argc >= MAX_ARGS {
        *status = ParseStatus::TooMany;
        return;
    }
    args.args[args.argc] = *arg;
    args.argc += 1;
    arg.clear();
}

fn tokenize(input: &[u8]) -> (ParsedLine, ParseStatus) {
    let mut line = ParsedLine::empty();
    let mut status = ParseStatus::Empty;
    let mut current = Arg::new();
    let mut quote = QuoteState::None;
    let mut i = 0usize;

    while i < input.len() {
        let byte = input[i];

        match quote {
            QuoteState::None => match byte {
                b'\n' | b'\r' => {
                    push_arg(&mut line, &mut current, &mut status);
                    break;
                }
                b' ' | b'\t' => {
                    push_arg(&mut line, &mut current, &mut status);
                }
                b'\'' => {
                    quote = QuoteState::Single;
                    if status == ParseStatus::Empty {
                        status = ParseStatus::Ok;
                    }
                }
                b'"' => {
                    quote = QuoteState::Double;
                    if status == ParseStatus::Empty {
                        status = ParseStatus::Ok;
                    }
                }
                b'#' if line.argc == 0 && current.len == 0 => {
                    status = ParseStatus::Ok;
                    break;
                }
                _ => {
                    if status == ParseStatus::Empty {
                        status = ParseStatus::Ok;
                    }
                    current.push_byte(byte, &mut status);
                }
            },
            QuoteState::Single => {
                if byte == b'\'' {
                    quote = QuoteState::None;
                } else {
                    current.push_byte(byte, &mut status);
                }
            }
            QuoteState::Double => {
                if byte == b'"' {
                    quote = QuoteState::None;
                } else {
                    current.push_byte(byte, &mut status);
                }
            }
        }

        i += 1;
    }

    if status == ParseStatus::Empty && current.len > 0 {
        status = ParseStatus::Ok;
    }
    push_arg(&mut line, &mut current, &mut status);

    (line, status)
}

fn write_u64_dec(daemon: &RootDaemon<'_>, value: u64) {
    let mut buf = [0u8; 24];
    let len = u64_to_dec(value, &mut buf);
    daemon.write_bytes(&buf[..len]);
}

fn write_u64_hex(daemon: &RootDaemon<'_>, value: u64) {
    let mut buf = [0u8; 18];
    let len = u64_to_hex(value, &mut buf[2..]);
    buf[0] = b'0';
    buf[1] = b'x';
    daemon.write_bytes(&buf[..2 + len]);
}

fn write_kv_num(daemon: &RootDaemon<'_>, key: &str, value: u64) {
    daemon.write_bytes(key.as_bytes());
    daemon.write_bytes(b"=");
    write_u64_dec(daemon, value);
    daemon.write_bytes(b"\n");
}

fn write_path_error(daemon: &RootDaemon<'_>, command: &str, path: &str, error: VfsError) {
    daemon.write_bytes(command.as_bytes());
    daemon.write_bytes(b": ");
    if !path.is_empty() {
        daemon.write_bytes(path.as_bytes());
        daemon.write_bytes(b": ");
    }
    match error {
        VfsError::EmptyPath => daemon.write_bytes(b"empty path"),
        VfsError::TooLong => daemon.write_bytes(b"path too long"),
        VfsError::NotFound => daemon.write_bytes(b"no such file or directory"),
        VfsError::NotDirectory => daemon.write_bytes(b"not a directory"),
    }
    daemon.write_bytes(b"\n");
}

fn write_memory_summary(daemon: &RootDaemon<'_>, info: BootInfo) {
    daemon.write_line("boot_info:");
    write_kv_num(daemon, "  ranges", info.memory.range_count() as u64);
    write_kv_num(daemon, "  usable_bytes", info.memory.usable_bytes());
    write_kv_num(daemon, "  cpu_count", info.cpu_count as u64);
    write_kv_num(daemon, "  heap_total_bytes", info.heap_total_bytes);
    write_kv_num(daemon, "  cpu_freq_hz", info.cpu_freq_hz);
    write_kv_num(daemon, "  mem_freq_hz", info.mem_freq_hz);
    write_kv_num(daemon, "  cache_line_bytes", info.cache_line_bytes as u64);
}

fn write_device_row(daemon: &RootDaemon<'_>, device: &DeviceEntry, index: usize) {
    daemon.write_bytes(b"- [");
    write_u64_dec(daemon, index as u64);
    daemon.write_bytes(b"] ");
    daemon.write_bytes(vfs::device_class_name(device.class).as_bytes());
    daemon.write_bytes(b" compat=");
    daemon.write_bytes(device.compatible.as_bytes());
    daemon.write_bytes(b" mmio=");
    write_u64_hex(daemon, device.mmio_base);
    daemon.write_bytes(b"/");
    write_u64_hex(daemon, device.mmio_len);
    daemon.write_bytes(b" irq=");
    write_u64_dec(daemon, device.irq as u64);
    daemon.write_bytes(b"\n");
}

fn write_device_name(daemon: &RootDaemon<'_>, devices: &[DeviceEntry], index: usize) {
    let (class, ordinal) = vfs::device_name_parts(devices, index);
    daemon.write_bytes(class.as_bytes());
    write_u64_dec(daemon, ordinal as u64);
}

fn cmd_device(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("device: too many arguments");
        return RootCommandStatus::Error;
    }
    write_memory_summary(daemon, daemon.boot_info());
    daemon.write_bytes(b"devices:\n");
    let devices = daemon.devices();
    let mut index = 0usize;
    while index < devices.len() {
        write_device_row(daemon, &devices[index], index);
        index += 1;
    }
    RootCommandStatus::Ok
}

fn cmd_dmesg(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("dmesg: too many arguments");
        return RootCommandStatus::Error;
    }
    daemon.write_line("dmesg:");
    write_log_dmesg(daemon);
    RootCommandStatus::Ok
}

fn cmd_pwd(
    daemon: &RootDaemon<'_>,
    session: &RootShellSession,
    line: &ParsedLine,
) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("pwd: too many arguments");
        return RootCommandStatus::Error;
    }
    daemon.write_line(session.cwd());
    RootCommandStatus::Ok
}

fn cmd_cd(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    line: &ParsedLine,
) -> RootCommandStatus {
    if line.argc > 2 {
        daemon.write_line("cd: too many arguments");
        return RootCommandStatus::Error;
    }
    let target = if line.argc == 1 {
        "/"
    } else {
        line.args[1].as_str()
    };
    let path = match vfs::normalize(session.cwd(), target) {
        Ok(path) => path,
        Err(error) => {
            write_path_error(daemon, "cd", target, error);
            return RootCommandStatus::Error;
        }
    };
    match vfs::lookup(path.as_str(), daemon.devices()) {
        Ok(Node::Directory(_)) => {
            session.set_cwd(path);
            RootCommandStatus::Ok
        }
        Ok(Node::File(_)) => {
            write_path_error(daemon, "cd", target, VfsError::NotDirectory);
            RootCommandStatus::Error
        }
        Err(error) => {
            write_path_error(daemon, "cd", target, error);
            RootCommandStatus::Error
        }
    }
}

fn cmd_ls(
    daemon: &RootDaemon<'_>,
    session: &RootShellSession,
    line: &ParsedLine,
) -> RootCommandStatus {
    if line.argc > 2 {
        daemon.write_line("ls: too many arguments");
        return RootCommandStatus::Error;
    }
    let target = if line.argc == 1 {
        session.cwd()
    } else {
        line.args[1].as_str()
    };
    let path = match vfs::normalize(session.cwd(), target) {
        Ok(path) => path,
        Err(error) => {
            write_path_error(daemon, "ls", target, error);
            return RootCommandStatus::Error;
        }
    };
    match vfs::lookup(path.as_str(), daemon.devices()) {
        Ok(Node::Directory(directory)) => {
            write_directory(daemon, directory);
            RootCommandStatus::Ok
        }
        Ok(Node::File(_)) => {
            daemon.write_line(path_basename(path.as_str()));
            RootCommandStatus::Ok
        }
        Err(error) => {
            write_path_error(daemon, "ls", target, error);
            RootCommandStatus::Error
        }
    }
}

fn write_directory(daemon: &RootDaemon<'_>, directory: Directory) {
    match directory {
        Directory::Root => {
            daemon.write_line("boot");
            daemon.write_line("dev");
            daemon.write_line("log");
        }
        Directory::Boot => {
            daemon.write_line("devices");
            daemon.write_line("help");
            daemon.write_line("image");
            daemon.write_line("input");
            daemon.write_line("memory");
            daemon.write_line("mounts");
            daemon.write_line("proof");
            daemon.write_line("profile");
            daemon.write_line("status");
        }
        Directory::Dev => {
            let devices = daemon.devices();
            let mut index = 0usize;
            while index < devices.len() {
                write_device_name(daemon, devices, index);
                daemon.write_bytes(b"\n");
                index += 1;
            }
        }
        Directory::Log => daemon.write_line("dmesg"),
    }
}

fn cmd_cat(
    daemon: &RootDaemon<'_>,
    session: &RootShellSession,
    line: &ParsedLine,
) -> RootCommandStatus {
    if line.argc == 1 {
        daemon.write_line("cat: missing path");
        return RootCommandStatus::Error;
    }

    let mut status = RootCommandStatus::Ok;
    let mut index = 1usize;
    while index < line.argc {
        let target = line.args[index].as_str();
        let path = match vfs::normalize(session.cwd(), target) {
            Ok(path) => path,
            Err(error) => {
                write_path_error(daemon, "cat", target, error);
                status = RootCommandStatus::Error;
                index += 1;
                continue;
            }
        };
        match vfs::lookup(path.as_str(), daemon.devices()) {
            Ok(Node::File(file)) => write_file(daemon, file),
            Ok(Node::Directory(_)) => {
                write_path_error(daemon, "cat", target, VfsError::NotDirectory);
                status = RootCommandStatus::Error;
            }
            Err(error) => {
                write_path_error(daemon, "cat", target, error);
                status = RootCommandStatus::Error;
            }
        }
        index += 1;
    }
    status
}

fn write_file(daemon: &RootDaemon<'_>, file: File) {
    match file {
        File::BootHelp => write_help_catalog(daemon),
        File::BootProfile => write_boot_profile(daemon),
        File::BootImage => write_boot_image(daemon),
        File::BootInput => write_boot_input(daemon),
        File::BootProof => write_boot_proof(daemon),
        File::BootStatus => write_boot_status(daemon),
        File::BootMemory => write_boot_memory(daemon),
        File::BootDevices => write_boot_devices(daemon),
        File::BootMounts => write_mount_table(daemon),
        File::LogDmesg => write_log_dmesg(daemon),
        File::DevDevice(index) => write_device_row(daemon, &daemon.devices()[index], index),
    }
}

fn write_boot_profile(daemon: &RootDaemon<'_>) {
    daemon.write_bytes(b"profile=");
    daemon.write_bytes(daemon.profile_name().as_bytes());
    daemon.write_bytes(b"\nlaunch=");
    daemon.write_bytes(if daemon.launch_enabled() {
        b"enabled"
    } else {
        b"disabled"
    });
    daemon.write_bytes(b"\npayloads=");
    write_u64_dec(daemon, daemon.payloads().len() as u64);
    daemon.write_bytes(b"\nprompt=");
    daemon.write_bytes(daemon.prompt().as_bytes());
    let input = daemon.console_input();
    daemon.write_bytes(b"\ninput=");
    daemon.write_bytes(input.source.as_bytes());
    daemon.write_bytes(b"\ninput_mode=");
    daemon.write_bytes(input.mode.as_bytes());
    daemon.write_bytes(b"\nusb_keyboard=");
    daemon.write_bytes(input_state_word(input.usb_keyboard));
    daemon.write_bytes(b"\n");
}

fn write_boot_image(daemon: &RootDaemon<'_>) {
    let image = daemon.boot_image();
    daemon.write_bytes(b"package=");
    daemon.write_bytes(image.package.as_bytes());
    daemon.write_bytes(b"\nversion=");
    daemon.write_bytes(image.version.as_bytes());
    daemon.write_bytes(b"\ntarget=");
    daemon.write_bytes(image.target.as_bytes());
    daemon.write_bytes(b"\nselected_profile=");
    daemon.write_bytes(image.selected_profile.as_bytes());
    daemon.write_bytes(b"\nprofile_request=");
    daemon.write_bytes(image.profile_request.as_bytes());
    daemon.write_bytes(b"\nbootline=");
    daemon.write_bytes(image.bootline.as_bytes());
    daemon.write_bytes(b"\nlaunch_profile_feature=");
    daemon.write_bytes(image.launch_profile_feature.as_bytes());
    daemon.write_bytes(b"\n");
}

fn write_boot_input(daemon: &RootDaemon<'_>) {
    let input = daemon.console_input();
    daemon.write_bytes(b"source=");
    daemon.write_bytes(input.source.as_bytes());
    daemon.write_bytes(b"\nsource_state=");
    daemon.write_bytes(input_state_word(input.source_state));
    daemon.write_bytes(b"\nmode=");
    daemon.write_bytes(input.mode.as_bytes());
    daemon.write_bytes(b"\nusb_keyboard=");
    daemon.write_bytes(input_state_word(input.usb_keyboard));
    daemon.write_bytes(b"\nusb_keyboard_pending_bytes=");
    write_u64_dec(daemon, input.usb_keyboard_pending_bytes as u64);
    daemon.write_bytes(b"\nusb_keyboard_probe=");
    write_bool_word(daemon, input.usb_keyboard_probe_enabled);
    daemon.write_bytes(b"\nusb_keyboard_poll_interval_ms=");
    write_u64_dec(daemon, input.usb_keyboard_poll_interval_ms as u64);
    daemon.write_bytes(b"\n");
}

fn write_boot_proof(daemon: &RootDaemon<'_>) {
    daemon.write_line("proof:");
    daemon.write_line("commands:");
    daemon.write_line("  help");
    daemon.write_line("  cat /boot/help");
    daemon.write_line("  clear");
    daemon.write_line("  screentest");
    daemon.write_line("  pwd");
    daemon.write_line("  ls /");
    daemon.write_line("  ls /boot");
    daemon.write_line("  ls /dev");
    daemon.write_line("  mount");
    daemon.write_line("  cat /boot/mounts");
    daemon.write_line("  device");
    daemon.write_line("  cat /boot/memory");
    daemon.write_line("  cat /boot/devices");
    daemon.write_line("  cd /dev");
    daemon.write_line("  pwd");
    daemon.write_line("  ls");
    daemon.write_line("  cat uart0");
    daemon.write_line("  cd /");
    daemon.write_line("  cat /boot/image");
    daemon.write_line("  status");
    daemon.write_line("  cat /boot/status");
    daemon.write_line("  input");
    daemon.write_line("  cat /boot/input");
    daemon.write_line("  probe help");
    daemon.write_line("  probe pcie");
    daemon.write_line("  probe usb-keyboard");
    daemon.write_line("  cat /boot/profile");
    daemon.write_line("  launch");
    daemon.write_line("  reovim");
    daemon.write_line("  dmesg");
    daemon.write_line("  cat /log/dmesg");
    daemon.write_line("terminal:");
    daemon.write_line("  halt");
    daemon.write_line("expected:");
    write_boot_proof_image_facts(daemon);
    write_boot_proof_profile_facts(daemon);
    daemon.write_line("  bootline=absent");
    daemon.write_line("  source=usb-keyboard+uart-fallback");
    daemon.write_line("  source_state=ready");
    daemon.write_line("  mode=live");
    daemon.write_line("  input_mode=live");
    daemon.write_line("  usb_keyboard=ready");
    daemon.write_line("  usb_keyboard_probe=enabled");
    daemon.write_line("  help catalog available through /boot/help");
    daemon.write_line("  probe targets include pcie");
    daemon.write_line("  probe targets include usb-keyboard");
    daemon.write_line("  probe targets include xhci-read-keyboard-report");
    daemon.write_line("  launch/reovim disabled in shell-only profile");
    daemon.write_line("  shell.status=ok");
    daemon.write_line("  shell.status=error for disabled payload commands");
    daemon.write_line("  halt typed last prints halt: ok and stops root daemon");
}

fn write_boot_proof_image_facts(daemon: &RootDaemon<'_>) {
    let image = daemon.boot_image();
    daemon.write_bytes(b"  package=");
    daemon.write_bytes(image.package.as_bytes());
    daemon.write_bytes(b"\n  version=");
    daemon.write_bytes(image.version.as_bytes());
    daemon.write_bytes(b"\n  target=");
    daemon.write_bytes(image.target.as_bytes());
    daemon.write_bytes(b"\n  selected_profile=");
    daemon.write_bytes(image.selected_profile.as_bytes());
    daemon.write_bytes(b"\n  profile_request=");
    daemon.write_bytes(image.profile_request.as_bytes());
    daemon.write_bytes(b"\n  launch_profile_feature=");
    daemon.write_bytes(image.launch_profile_feature.as_bytes());
    daemon.write_bytes(b"\n");
}

fn write_boot_proof_profile_facts(daemon: &RootDaemon<'_>) {
    daemon.write_bytes(b"  profile=");
    daemon.write_bytes(daemon.profile_name().as_bytes());
    daemon.write_bytes(b"\n  launch=");
    daemon.write_bytes(if daemon.launch_enabled() {
        b"enabled"
    } else {
        b"disabled"
    });
    daemon.write_bytes(b"\n  payloads=");
    write_u64_dec(daemon, daemon.payloads().len() as u64);
    daemon.write_bytes(b"\n");
}

fn write_boot_status(daemon: &RootDaemon<'_>) {
    let image = daemon.boot_image();
    let input = daemon.console_input();
    daemon.write_bytes(b"package=");
    daemon.write_bytes(image.package.as_bytes());
    daemon.write_bytes(b"\nversion=");
    daemon.write_bytes(image.version.as_bytes());
    daemon.write_bytes(b"\ntarget=");
    daemon.write_bytes(image.target.as_bytes());
    daemon.write_bytes(b"\nselected_profile=");
    daemon.write_bytes(image.selected_profile.as_bytes());
    daemon.write_bytes(b"\nprofile_request=");
    daemon.write_bytes(image.profile_request.as_bytes());
    daemon.write_bytes(b"\nbootline=");
    daemon.write_bytes(image.bootline.as_bytes());
    daemon.write_bytes(b"\nlaunch_profile_feature=");
    daemon.write_bytes(image.launch_profile_feature.as_bytes());
    daemon.write_bytes(b"\nprofile=");
    daemon.write_bytes(daemon.profile_name().as_bytes());
    daemon.write_bytes(b"\nlaunch=");
    daemon.write_bytes(if daemon.launch_enabled() {
        b"enabled"
    } else {
        b"disabled"
    });
    daemon.write_bytes(b"\npayloads=");
    write_u64_dec(daemon, daemon.payloads().len() as u64);
    daemon.write_bytes(b"\ninput=");
    daemon.write_bytes(input.source.as_bytes());
    daemon.write_bytes(b"\nsource_state=");
    daemon.write_bytes(input_state_word(input.source_state));
    daemon.write_bytes(b"\ninput_mode=");
    daemon.write_bytes(input.mode.as_bytes());
    daemon.write_bytes(b"\nusb_keyboard=");
    daemon.write_bytes(input_state_word(input.usb_keyboard));
    daemon.write_bytes(b"\nusb_keyboard_probe=");
    write_bool_word(daemon, input.usb_keyboard_probe_enabled);
    daemon.write_bytes(b"\nusb_keyboard_poll_interval_ms=");
    write_u64_dec(daemon, input.usb_keyboard_poll_interval_ms as u64);
    daemon.write_bytes(b"\nusb_keyboard_pending_bytes=");
    write_u64_dec(daemon, input.usb_keyboard_pending_bytes as u64);
    daemon.write_bytes(b"\nmanual_next=");
    daemon.write_bytes(manual_next_step(input));
    daemon.write_bytes(b"\n");
}

fn write_boot_memory(daemon: &RootDaemon<'_>) {
    let info = daemon.boot_info();
    write_kv_num(daemon, "ranges", info.memory.range_count() as u64);
    write_kv_num(daemon, "usable_bytes", info.memory.usable_bytes());
    write_kv_num(daemon, "cpu_count", info.cpu_count as u64);
    write_kv_num(daemon, "heap_total_bytes", info.heap_total_bytes);
    write_kv_num(daemon, "cache_line_bytes", info.cache_line_bytes as u64);
}

fn write_boot_devices(daemon: &RootDaemon<'_>) {
    let devices = daemon.devices();
    let mut index = 0usize;
    while index < devices.len() {
        write_device_row(daemon, &devices[index], index);
        index += 1;
    }
}

fn write_mount_table(daemon: &RootDaemon<'_>) {
    for mount in vfs::mounts() {
        daemon.write_bytes(mount.source.as_bytes());
        daemon.write_bytes(b" on ");
        daemon.write_bytes(mount.target.as_bytes());
        daemon.write_bytes(b" type ");
        daemon.write_bytes(mount.fs_type.as_bytes());
        daemon.write_bytes(b" (");
        daemon.write_bytes(mount.flags.as_bytes());
        daemon.write_bytes(b")\n");
    }
}

fn write_log_dmesg(daemon: &RootDaemon<'_>) {
    let wrote_kernel_log = klog::write_to(|bytes| daemon.write_bytes(bytes));
    if let Some(snapshot) = daemon.dmesg_fn() {
        if wrote_kernel_log {
            daemon.write_line("external diagnostics:");
        }
        daemon.write_line(snapshot());
    } else if !wrote_kernel_log {
        daemon.write_line("(kernel log empty)");
    }
}

fn path_basename(path: &str) -> &str {
    let bytes = path.as_bytes();
    let mut start = bytes.len();
    while start > 0 {
        if bytes[start - 1] == b'/' {
            break;
        }
        start -= 1;
    }
    &path[start..]
}

fn input_state_word(state: crate::rootd::BootCheckState) -> &'static [u8] {
    match state {
        crate::rootd::BootCheckState::Ok => b"ready",
        crate::rootd::BootCheckState::Warn => b"unavailable",
    }
}

fn manual_next_step(input: crate::rootd::ConsoleInputSummary) -> &'static [u8] {
    if input.usb_keyboard == crate::rootd::BootCheckState::Ok {
        b"type-shell-command"
    } else if input.usb_keyboard_probe_enabled {
        b"probe-usb-keyboard"
    } else {
        b"probe-help"
    }
}

fn write_payload_list(daemon: &RootDaemon<'_>, payloads: &[PayloadDescriptor]) {
    if payloads.is_empty() {
        daemon.write_line("launch: no payloads registered");
        return;
    }

    daemon.write_line("launch: available payloads:");
    let mut index = 0usize;
    while index < payloads.len() {
        daemon.write_bytes(b"  ");
        daemon.write_bytes(payloads[index].name.as_bytes());
        daemon.write_bytes(b": ");
        daemon.write_bytes(payloads[index].summary.as_bytes());
        daemon.write_bytes(b"\n");
        index += 1;
    }
}

fn write_payload_result(daemon: &RootDaemon<'_>, payload_name: &str, result: PayloadLaunchResult) {
    daemon.write_bytes(b"launch ");
    daemon.write_bytes(payload_name.as_bytes());
    daemon.write_bytes(b": ");
    write_payload_status(daemon, result);
}

fn write_payload_status(daemon: &RootDaemon<'_>, result: PayloadLaunchResult) {
    match result {
        PayloadLaunchResult::Ready => daemon.write_bytes(b"payload.ready"),
        PayloadLaunchResult::NotConfigured => daemon.write_bytes(b"payload.not_configured"),
        PayloadLaunchResult::Failed => daemon.write_bytes(b"payload.failed"),
    }
    daemon.write_bytes(b"\n");
}

fn write_bool_word(daemon: &RootDaemon<'_>, value: bool) {
    daemon.write_bytes(if value { b"enabled" } else { b"disabled" });
}

fn payload_result_status(result: PayloadLaunchResult) -> RootCommandStatus {
    match result {
        PayloadLaunchResult::Ready => RootCommandStatus::Ok,
        PayloadLaunchResult::NotConfigured | PayloadLaunchResult::Failed => {
            RootCommandStatus::Error
        }
    }
}

fn cmd_launch(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if !daemon.launch_enabled() {
        daemon.write_line("launch disabled for this profile");
        return RootCommandStatus::Error;
    }

    if line.argc == 1 {
        write_payload_list(daemon, daemon.payloads());
        return RootCommandStatus::Ok;
    }

    if line.argc > 2 {
        daemon.write_line("launch: too many arguments");
        return RootCommandStatus::Error;
    }

    let payload = line.args[1].as_str();
    if payload.is_empty() {
        daemon.write_line("launch: no payload name");
        return RootCommandStatus::Error;
    }

    let result = daemon.launch_payload_by_name(payload);
    write_payload_result(daemon, payload, result);
    payload_result_status(result)
}

fn cmd_probe(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc == 1 {
        daemon.write_line("probe: missing target, try `probe help`");
        return RootCommandStatus::Error;
    }
    if line.argc > 2 {
        daemon.write_line("probe: too many arguments");
        return RootCommandStatus::Error;
    }

    let target = line.args[1].as_str();
    match daemon.run_hardware_probe(target) {
        Some(HardwareProbeResult::Handled) => RootCommandStatus::Ok,
        Some(HardwareProbeResult::UnknownTarget) => {
            daemon.write_bytes(b"probe: unknown target: ");
            daemon.write_bytes(target.as_bytes());
            daemon.write_bytes(b"\n");
            RootCommandStatus::Error
        }
        None => {
            daemon.write_line("probe: no lower probe provider");
            RootCommandStatus::Error
        }
    }
}

fn cmd_help(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 2 {
        daemon.write_line("help: too many arguments");
        return RootCommandStatus::Error;
    }

    if line.argc == 1 {
        write_help_catalog(daemon);
        return RootCommandStatus::Ok;
    }

    let command = line.args[1].as_str();
    match command {
        "help" => daemon.write_line("help [command] - show command help"),
        "clear" => daemon.write_line("clear - clear framebuffer console and terminal"),
        "screentest" => daemon.write_line("screentest - print renderer diagnostics"),
        "pwd" => daemon.write_line("pwd - print current kernel VFS directory"),
        "ls" => daemon.write_line("ls [path] - list a kernel VFS directory"),
        "cd" => daemon.write_line("cd [path] - change current kernel VFS directory"),
        "cat" => daemon.write_line("cat path... - print kernel VFS pseudo files"),
        "mount" => daemon.write_line("mount - print kernel VFS mount table"),
        "input" => daemon.write_line("input - print live console input diagnostics"),
        "status" => daemon.write_line("status - print boot and input summary"),
        "proof" => daemon.write_line("proof - print physical input proof checklist"),
        "device" => daemon.write_line("device - print boot memory and device inventory"),
        "dmesg" => daemon.write_line("dmesg - print retained kernel log"),
        "probe" => daemon.write_line("probe target - run lower hardware probe; try `probe help`"),
        "launch" => daemon.write_line("launch [payload] - list or run registered payloads"),
        "reovim" => daemon.write_line("reovim - run the default reovim payload alias"),
        "halt" => daemon.write_line("halt - request root daemon shutdown"),
        _ => {
            daemon.write_bytes(b"help: unknown command: ");
            daemon.write_bytes(command.as_bytes());
            daemon.write_bytes(b"\n");
            return RootCommandStatus::Error;
        }
    }
    RootCommandStatus::Ok
}

fn write_help_catalog(daemon: &RootDaemon<'_>) {
    daemon.write_line("reovim root shell");
    daemon.write_line(
        "commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt",
    );
    daemon.write_line("usage: help [command]");
}

fn cmd_clear(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("clear: too many arguments");
        return RootCommandStatus::Error;
    }
    crate::console::clear_screen();
    daemon.write_bytes(b"\x1b[2J\x1b[H");
    RootCommandStatus::Ok
}

fn cmd_screentest(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("screentest: too many arguments");
        return RootCommandStatus::Error;
    }
    daemon.write_line("screen test:");
    daemon.write_line("  target: framebuffer/serial tty renderer subset");
    daemon.write_bytes(b"  fg16: \x1b[30;47mblack\x1b[0m \x1b[31mred\x1b[0m \x1b[32mgreen\x1b[0m \x1b[33myellow\x1b[0m \x1b[34mblue\x1b[0m \x1b[35mmagenta\x1b[0m \x1b[36mcyan\x1b[0m \x1b[37mwhite\x1b[0m\n");
    daemon.write_bytes(
        b"  fg16+: \x1b[90mgray\x1b[0m \x1b[91mbr-red\x1b[0m \x1b[92mbr-green\x1b[0m \x1b[93mbr-yellow\x1b[0m \x1b[94mbr-blue\x1b[0m \x1b[95mbr-magenta\x1b[0m \x1b[96mbr-cyan\x1b[0m \x1b[97;40mbr-white\x1b[0m\n",
    );
    daemon.write_bytes(b"  bg16: \x1b[37;40m 40 \x1b[0m \x1b[30;41m 41 \x1b[0m \x1b[30;42m 42 \x1b[0m \x1b[30;43m 43 \x1b[0m \x1b[37;44m 44 \x1b[0m \x1b[30;45m 45 \x1b[0m \x1b[30;46m 46 \x1b[0m \x1b[30;47m 47 \x1b[0m\n");
    daemon.write_bytes(
        b"  bg16+: \x1b[30;100m 100 \x1b[0m \x1b[30;101m 101 \x1b[0m \x1b[30;102m 102 \x1b[0m \x1b[30;103m 103 \x1b[0m \x1b[30;104m 104 \x1b[0m \x1b[30;105m 105 \x1b[0m \x1b[30;106m 106 \x1b[0m \x1b[30;107m 107 \x1b[0m\n",
    );
    daemon.write_bytes(b"  idx-fg: \x1b[38;5;21midx21\x1b[0m \x1b[38;5;46midx46\x1b[0m \x1b[38;5;51midx51\x1b[0m \x1b[38;5;93midx93\x1b[0m \x1b[38;5;160midx160\x1b[0m \x1b[38;5;196midx196\x1b[0m \x1b[38;5;201midx201\x1b[0m \x1b[38;5;226midx226\x1b[0m\n");
    daemon.write_bytes(b"  idx-bg: \x1b[48;5;21m 21 \x1b[0m \x1b[48;5;46m 46 \x1b[0m \x1b[48;5;51m 51 \x1b[0m \x1b[48;5;93m 93 \x1b[0m \x1b[48;5;160m 160 \x1b[0m \x1b[48;5;196m 196 \x1b[0m \x1b[48;5;201m 201 \x1b[0m \x1b[48;5;226m 226 \x1b[0m\n");
    daemon.write_bytes(b"  rgb-fg: \x1b[38;2;255;92;87mwarm\x1b[0m \x1b[38;2;114;214;86mgreen\x1b[0m \x1b[38;2;35;132;255msky\x1b[0m \x1b[38;2;190;120;255mviolet\x1b[0m \x1b[38;2;255;255;255;48;2;0;0;0mwhite-on-black\x1b[0m\n");
    daemon.write_bytes(b"  rgb-bg: \x1b[48;2;255;92;87m  warm  \x1b[0m \x1b[48;2;114;214;86m  green  \x1b[0m \x1b[48;2;35;132;255m  sky  \x1b[0m \x1b[48;2;190;120;255m  violet  \x1b[0m\n");
    daemon.write_bytes(b"  attrs: \x1b[1mbold\x1b[22m \x1b[2mdim\x1b[22m \x1b[3mitalic\x1b[23m \x1b[4munderline\x1b[24m \x1b[7mreverse\x1b[27m \x1b[1;4mbold+underline\x1b[0m normal\n");
    daemon.write_bytes(b"  reset: \x1b[31mred\x1b[39m default-fg \x1b[48;2;48;48;48mgray-bg\x1b[49m default-bg \x1b[1;7mbold-rev\x1b[0m plain\n");
    daemon.write_bytes(b"  cr: left-side-should-vanish\r  cr: overwritten\n");
    daemon.write_bytes(b"  bs: AB\x08 \x08C (should read AC)\n");
    daemon.write_bytes(b"  wrap: 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz end\n");
    daemon.write_line("  done");
    RootCommandStatus::Ok
}

fn cmd_halt(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("halt: too many arguments");
        return RootCommandStatus::Error;
    }
    daemon.write_line("halt: ok");
    RootCommandStatus::Halt
}

fn cmd_mount(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("mount: too many arguments");
        return RootCommandStatus::Error;
    }
    write_mount_table(daemon);
    RootCommandStatus::Ok
}

fn cmd_input(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("input: too many arguments");
        return RootCommandStatus::Error;
    }
    write_boot_input(daemon);
    RootCommandStatus::Ok
}

fn cmd_status(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("status: too many arguments");
        return RootCommandStatus::Error;
    }
    write_boot_status(daemon);
    RootCommandStatus::Ok
}

fn cmd_proof(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("proof: too many arguments");
        return RootCommandStatus::Error;
    }
    write_boot_proof(daemon);
    RootCommandStatus::Ok
}

fn cmd_reovim(daemon: &RootDaemon<'_>, line: &ParsedLine) -> RootCommandStatus {
    if line.argc > 1 {
        daemon.write_line("reovim: too many arguments");
        return RootCommandStatus::Error;
    }
    if !daemon.launch_enabled() {
        daemon.write_line("reovim disabled for this profile");
        return RootCommandStatus::Error;
    }

    daemon.write_bytes(b"reovim: ");
    let result = daemon.launch_payload_by_name("reovim");
    write_payload_status(daemon, result);
    payload_result_status(result)
}

fn u64_to_dec(mut value: u64, out: &mut [u8]) -> usize {
    let mut tmp = [0u8; 24];
    if value == 0 {
        out[0] = b'0';
        return 1;
    }

    let mut len = 0usize;
    while value > 0 {
        tmp[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }

    let mut i = 0usize;
    while len > 0 {
        len -= 1;
        out[i] = tmp[len];
        i += 1;
    }
    i
}

fn u64_to_hex(mut value: u64, out: &mut [u8]) -> usize {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    if value == 0 {
        out[0] = b'0';
        return 1;
    }

    let mut tmp = [0u8; 16];
    let mut len = 0usize;
    while value > 0 {
        tmp[len] = HEX[(value & 0xf) as usize];
        len += 1;
        value >>= 4;
    }

    let mut i = 0usize;
    while len > 0 {
        len -= 1;
        out[i] = tmp[len];
        i += 1;
    }
    i
}

/// Execute one root-daemon line from the attached input source.
///
/// Returns the command status, including whether the line requested halt.
pub(crate) fn execute_root_command(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    line: &[u8],
) -> RootCommandResult {
    let (parsed, status) = tokenize(line);

    match status {
        ParseStatus::Empty => return RootCommandResult::new(RootCommandStatus::Empty),
        ParseStatus::TooLong => {
            daemon.write_line("error: command token too long");
            return RootCommandResult::new(RootCommandStatus::Error);
        }
        ParseStatus::TooMany => {
            daemon.write_line("error: too many arguments");
            return RootCommandResult::new(RootCommandStatus::Error);
        }
        ParseStatus::Ok => {}
    }

    if parsed.argc == 0 {
        return RootCommandResult::new(RootCommandStatus::Empty);
    }

    let command = parsed.args[0].as_str();
    let status = match command {
        "help" => cmd_help(daemon, &parsed),
        "clear" => cmd_clear(daemon, &parsed),
        "screentest" => cmd_screentest(daemon, &parsed),
        "pwd" => cmd_pwd(daemon, session, &parsed),
        "ls" => cmd_ls(daemon, session, &parsed),
        "cd" => cmd_cd(daemon, session, &parsed),
        "cat" => cmd_cat(daemon, session, &parsed),
        "mount" => cmd_mount(daemon, &parsed),
        "input" => cmd_input(daemon, &parsed),
        "status" => cmd_status(daemon, &parsed),
        "proof" => cmd_proof(daemon, &parsed),
        "device" => cmd_device(daemon, &parsed),
        "dmesg" => cmd_dmesg(daemon, &parsed),
        "probe" => cmd_probe(daemon, &parsed),
        "launch" => cmd_launch(daemon, &parsed),
        "reovim" => cmd_reovim(daemon, &parsed),
        "halt" => cmd_halt(daemon, &parsed),
        _ => {
            daemon.write_line("error: unknown command, try `help`");
            RootCommandStatus::Error
        }
    };

    RootCommandResult::new(status)
}

#[cfg(feature = "selftest")]
#[path = "root_shell_tests.rs"]
mod tests;
