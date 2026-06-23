//! Root daemon parser and command execution primitives.
//!
//! This module keeps the OS-mode shell proof intentionally small: bounded input
//! tokenization, a minimal bash-like vocabulary, and static output through the
//! injected console write callback in `rootd`.

use {
    crate::rootd::{PayloadDescriptor, PayloadLaunchResult, RootDaemon, device_class_name},
    reovim_uapi_system::{BootInfo, DeviceEntry},
};

const MAX_ARGS: usize = 8;
const MAX_TOKEN_BYTES: usize = 64;

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
    daemon.write_bytes(device_class_name(device.class).as_bytes());
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

fn cmd_device(daemon: &RootDaemon<'_>) {
    write_memory_summary(daemon, daemon.boot_info());
    daemon.write_bytes(b"devices:\n");
    let devices = daemon.devices();
    let mut index = 0usize;
    while index < devices.len() {
        write_device_row(daemon, &devices[index], index);
        index += 1;
    }
}

fn cmd_dmesg(daemon: &RootDaemon<'_>) {
    if let Some(snapshot) = daemon.dmesg_fn() {
        daemon.write_line("dmesg:");
        daemon.write_line(snapshot());
        return;
    }
    daemon.write_line("dmesg: (no diagnostics source)");
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
    match result {
        PayloadLaunchResult::Ready => daemon.write_bytes(b"payload.ready"),
        PayloadLaunchResult::NotConfigured => daemon.write_bytes(b"payload.not_configured"),
        PayloadLaunchResult::Failed => daemon.write_bytes(b"payload.failed"),
    }
    daemon.write_bytes(b"\n");
}

fn cmd_launch(daemon: &RootDaemon<'_>, line: &ParsedLine) {
    if !daemon.launch_enabled() {
        daemon.write_line("launch disabled for this profile");
        return;
    }

    if line.argc == 1 {
        write_payload_list(daemon, daemon.payloads());
        return;
    }

    if line.argc > 2 {
        daemon.write_line("launch: too many arguments");
        return;
    }

    let payload = line.args[1].as_str();
    if payload.is_empty() {
        daemon.write_line("launch: no payload name");
        return;
    }

    write_payload_result(daemon, payload, daemon.launch_payload_by_name(payload));
}

fn cmd_help(daemon: &RootDaemon<'_>) {
    daemon.write_line("reovim root shell");
    daemon.write_line("commands: help, device, dmesg, launch, halt");
    daemon.write_line("reserved: ls, cd, pwd, cat, mount, reovim");
}

fn cmd_halt(daemon: &RootDaemon<'_>) {
    daemon.write_line("halt: ok");
    daemon.halt_kernel();
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
/// Returns `true` when the command requests halting the daemon.
pub fn execute_root_command(daemon: &RootDaemon<'_>, line: &[u8]) -> bool {
    let (parsed, status) = tokenize(line);

    match status {
        ParseStatus::Empty => return false,
        ParseStatus::TooLong => {
            daemon.write_line("error: command token too long");
            return false;
        }
        ParseStatus::TooMany => {
            daemon.write_line("error: too many arguments");
            return false;
        }
        ParseStatus::Ok => {}
    }

    if parsed.argc == 0 {
        return false;
    }

    let command = parsed.args[0].as_str();
    match command {
        "help" => cmd_help(daemon),
        "device" => cmd_device(daemon),
        "dmesg" => cmd_dmesg(daemon),
        "launch" => cmd_launch(daemon, &parsed),
        "halt" => {
            cmd_halt(daemon);
            return true;
        }
        "ls" | "cd" | "pwd" | "cat" | "mount" | "reovim" => {
            daemon.write_line("reserved: supported after VFS/current-directory rollout");
        }
        _ => daemon.write_line("error: unknown command, try `help`"),
    }

    false
}

#[cfg(feature = "selftest")]
#[path = "root_shell_tests.rs"]
mod tests;
