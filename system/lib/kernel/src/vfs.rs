//! Bounded system-kernel VFS namespace for the root shell.
//!
//! This is not a public POSIX face. It is a small, read-only kernel namespace
//! over boot facts, device inventory, and diagnostics supplied to `rootd`.

use {
    crate::program::{self, ProgramDescriptor},
    reovim_uapi_system::{DeviceClass, DeviceEntry},
};

/// Maximum absolute path bytes carried by root-shell session state.
pub const MAX_PATH_BYTES: usize = 128;

/// Path/lookup errors surfaced by the kernel VFS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfsError {
    /// A command supplied an empty path where an explicit path is required.
    EmptyPath,
    /// The normalized path would exceed [`MAX_PATH_BYTES`].
    TooLong,
    /// The path does not resolve to a namespace node.
    NotFound,
    /// Traversal tried to enter a file-like node.
    NotDirectory,
    /// The node exists but does not support writes.
    NotWritable,
    /// A VFS file read buffer is already in use by this process.
    Busy,
    /// The generated file content exceeded the bounded read buffer.
    FileTooLarge,
    /// A descriptor-shaped file read or write failed after open.
    Io,
}

/// A bounded normalized absolute path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathBuf {
    bytes: [u8; MAX_PATH_BYTES],
    len: usize,
}

impl PathBuf {
    /// Returns `/`.
    #[must_use]
    pub const fn root() -> Self {
        let mut bytes = [0u8; MAX_PATH_BYTES];
        bytes[0] = b'/';
        Self { bytes, len: 1 }
    }

    /// Returns this path as UTF-8 shell text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // The shell tokenizer currently accepts ASCII command bytes; path
        // normalization preserves those bytes and only injects `/`.
        unsafe { core::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }

    fn push_component(&mut self, component: &[u8]) -> Result<(), VfsError> {
        if component.is_empty() || component == b"." {
            return Ok(());
        }
        if component == b".." {
            self.pop_component();
            return Ok(());
        }

        let needs_slash = self.len > 1;
        let new_len = self.len + usize::from(needs_slash) + component.len();
        if new_len > self.bytes.len() {
            return Err(VfsError::TooLong);
        }
        if needs_slash {
            self.bytes[self.len] = b'/';
            self.len += 1;
        }
        let start = self.len;
        self.bytes[start..start + component.len()].copy_from_slice(component);
        self.len += component.len();
        Ok(())
    }

    fn pop_component(&mut self) {
        if self.len == 1 {
            return;
        }
        let mut i = self.len - 1;
        while i > 0 && self.bytes[i] != b'/' {
            i -= 1;
        }
        self.len = if i == 0 { 1 } else { i };
    }
}

/// Directory nodes in the kernel VFS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Directory {
    /// `/`.
    Root,
    /// `/bin`.
    Bin,
    /// `/boot`.
    Boot,
    /// `/dev`.
    Dev,
    /// `/dump`.
    Dump,
    /// `/log`.
    Log,
    /// `/proc`.
    Proc,
}

/// One mounted namespace in the kernel VFS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MountEntry {
    /// Stable source label shown by the root shell.
    pub source: &'static str,
    /// Absolute mount target.
    pub target: &'static str,
    /// Kernel-local filesystem kind.
    pub fs_type: &'static str,
    /// Readable mount flags.
    pub flags: &'static str,
}

const MOUNT_TABLE: [MountEntry; 7] = [
    MountEntry {
        source: "kernel",
        target: "/",
        fs_type: "rootfs",
        flags: "ro,pseudo",
    },
    MountEntry {
        source: "programs",
        target: "/bin",
        fs_type: "binfs",
        flags: "ro,static",
    },
    MountEntry {
        source: "boot",
        target: "/boot",
        fs_type: "bootfs",
        flags: "ro,pseudo",
    },
    MountEntry {
        source: "devices",
        target: "/dev",
        fs_type: "devfs",
        flags: "ro,pseudo",
    },
    MountEntry {
        source: "dump",
        target: "/dump",
        fs_type: "dumpfs",
        flags: "ro,pseudo",
    },
    MountEntry {
        source: "klog",
        target: "/log",
        fs_type: "logfs",
        flags: "ro,pseudo",
    },
    MountEntry {
        source: "processes",
        target: "/proc",
        fs_type: "procfs",
        flags: "ro,pseudo",
    },
];

/// Returns the static mount table that backs the root-shell namespace.
#[must_use]
pub const fn mounts() -> &'static [MountEntry] {
    &MOUNT_TABLE
}

/// Read-only file or pseudo-device nodes in the kernel VFS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    /// `/bin/{program}` by stable program descriptor ID.
    BinProgram(usize),
    /// `/boot/help`.
    BootHelp,
    /// `/boot/profile`.
    BootProfile,
    /// `/boot/image`.
    BootImage,
    /// `/boot/input`.
    BootInput,
    /// `/boot/proof`.
    BootProof,
    /// `/boot/probes`.
    BootProbes,
    /// `/boot/payloads`.
    BootPayloads,
    /// `/boot/status`.
    BootStatus,
    /// `/boot/memory`.
    BootMemory,
    /// `/boot/devices`.
    BootDevices,
    /// `/boot/mounts`.
    BootMounts,
    /// `/log/dmesg`.
    LogDmesg,
    /// `/log/events`.
    LogEvents,
    /// `/log/stats`.
    LogStats,
    /// `/dump/status`.
    DumpStatus,
    /// `/dump/snapshot`.
    DumpSnapshot,
    /// `/proc/processes`.
    ProcProcesses,
    /// `/proc/continuations`.
    ProcContinuations,
    /// `/proc/execs`.
    ProcExecs,
    /// `/proc/address-spaces`.
    ProcAddressSpaces,
    /// `/proc/page-tables`.
    ProcPageTables,
    /// `/proc/pages`.
    ProcPages,
    /// `/proc/memory-objects`.
    ProcMemoryObjects,
    /// `/proc/pending`.
    ProcPending,
    /// `/proc/media`.
    ProcMedia,
    /// `/proc/self`.
    ProcSelf,
    /// `/proc/session`.
    ProcSession,
    /// `/proc/services`.
    ProcServices,
    /// `/proc/scheduler`.
    ProcScheduler,
    /// `/proc/sources`.
    ProcSources,
    /// `/proc/syscalls`.
    ProcSyscalls,
    /// `/proc/tasks`.
    ProcTasks,
    /// `/proc/waits`.
    ProcWaits,
    /// `/dev/tty`.
    DevTty,
    /// `/dev/{class}{ordinal}`.
    DevDevice(usize),
}

/// Resolved VFS node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    /// A directory.
    Directory(Directory),
    /// A read-only file/pseudo node.
    File(File),
}

/// Normalizes `input` against `cwd` into an absolute path.
pub fn normalize(cwd: &str, input: &str) -> Result<PathBuf, VfsError> {
    if input.is_empty() {
        return Err(VfsError::EmptyPath);
    }

    let mut out = PathBuf::root();
    if input.as_bytes()[0] != b'/' {
        push_components(&mut out, cwd.as_bytes())?;
    }
    push_components(&mut out, input.as_bytes())?;
    Ok(out)
}

fn push_components(out: &mut PathBuf, path: &[u8]) -> Result<(), VfsError> {
    let mut start = 0usize;
    let mut i = 0usize;
    while i <= path.len() {
        if i == path.len() || path[i] == b'/' {
            out.push_component(&path[start..i])?;
            start = i + 1;
        }
        i += 1;
    }
    Ok(())
}

/// Resolves a normalized absolute path against the current root namespace.
pub fn lookup(
    path: &str,
    devices: &[DeviceEntry],
    programs: &'static [ProgramDescriptor],
) -> Result<Node, VfsError> {
    match path {
        "/" => return Ok(Node::Directory(Directory::Root)),
        "/bin" => return Ok(Node::Directory(Directory::Bin)),
        "/boot" => return Ok(Node::Directory(Directory::Boot)),
        "/dev" => return Ok(Node::Directory(Directory::Dev)),
        "/dump" => return Ok(Node::Directory(Directory::Dump)),
        "/log" => return Ok(Node::Directory(Directory::Log)),
        "/proc" => return Ok(Node::Directory(Directory::Proc)),
        "/boot/help" => return Ok(Node::File(File::BootHelp)),
        "/boot/profile" => return Ok(Node::File(File::BootProfile)),
        "/boot/image" => return Ok(Node::File(File::BootImage)),
        "/boot/input" => return Ok(Node::File(File::BootInput)),
        "/boot/proof" => return Ok(Node::File(File::BootProof)),
        "/boot/probes" => return Ok(Node::File(File::BootProbes)),
        "/boot/payloads" => return Ok(Node::File(File::BootPayloads)),
        "/boot/status" => return Ok(Node::File(File::BootStatus)),
        "/boot/memory" => return Ok(Node::File(File::BootMemory)),
        "/boot/devices" => return Ok(Node::File(File::BootDevices)),
        "/boot/mounts" => return Ok(Node::File(File::BootMounts)),
        "/dump/status" => return Ok(Node::File(File::DumpStatus)),
        "/dump/snapshot" => return Ok(Node::File(File::DumpSnapshot)),
        "/log/dmesg" => return Ok(Node::File(File::LogDmesg)),
        "/log/events" => return Ok(Node::File(File::LogEvents)),
        "/log/stats" => return Ok(Node::File(File::LogStats)),
        "/proc/processes" => return Ok(Node::File(File::ProcProcesses)),
        "/proc/continuations" => return Ok(Node::File(File::ProcContinuations)),
        "/proc/execs" => return Ok(Node::File(File::ProcExecs)),
        "/proc/address-spaces" => return Ok(Node::File(File::ProcAddressSpaces)),
        "/proc/page-tables" => return Ok(Node::File(File::ProcPageTables)),
        "/proc/pages" => return Ok(Node::File(File::ProcPages)),
        "/proc/memory-objects" => return Ok(Node::File(File::ProcMemoryObjects)),
        "/proc/pending" => return Ok(Node::File(File::ProcPending)),
        "/proc/media" => return Ok(Node::File(File::ProcMedia)),
        "/proc/self" => return Ok(Node::File(File::ProcSelf)),
        "/proc/session" => return Ok(Node::File(File::ProcSession)),
        "/proc/services" => return Ok(Node::File(File::ProcServices)),
        "/proc/scheduler" => return Ok(Node::File(File::ProcScheduler)),
        "/proc/sources" => return Ok(Node::File(File::ProcSources)),
        "/proc/syscalls" => return Ok(Node::File(File::ProcSyscalls)),
        "/proc/tasks" => return Ok(Node::File(File::ProcTasks)),
        "/proc/waits" => return Ok(Node::File(File::ProcWaits)),
        "/dev/tty" => return Ok(Node::File(File::DevTty)),
        _ => {}
    }

    if let Some(rest) = path.strip_prefix("/bin/") {
        let (name, tail) = split_first(rest);
        if let Some((_index, program)) = program::find_by_bin_name(programs, name) {
            return if tail.is_empty() {
                Ok(Node::File(File::BinProgram(program.id)))
            } else {
                Err(VfsError::NotDirectory)
            };
        }
        return Err(VfsError::NotFound);
    }

    if let Some(rest) = path.strip_prefix("/dev/") {
        let (name, tail) = split_first(rest);
        if name == "tty" {
            return if tail.is_empty() {
                Ok(Node::File(File::DevTty))
            } else {
                Err(VfsError::NotDirectory)
            };
        }
        if let Some(index) = find_device(devices, name) {
            return if tail.is_empty() {
                Ok(Node::File(File::DevDevice(index)))
            } else {
                Err(VfsError::NotDirectory)
            };
        }
        return Err(VfsError::NotFound);
    }

    for file in [
        "/boot/profile/",
        "/boot/help/",
        "/boot/image/",
        "/boot/input/",
        "/boot/proof/",
        "/boot/probes/",
        "/boot/payloads/",
        "/boot/status/",
        "/boot/memory/",
        "/boot/devices/",
        "/boot/mounts/",
        "/dump/status/",
        "/dump/snapshot/",
        "/log/dmesg/",
        "/log/events/",
        "/log/stats/",
        "/proc/processes/",
        "/proc/continuations/",
        "/proc/execs/",
        "/proc/address-spaces/",
        "/proc/page-tables/",
        "/proc/pages/",
        "/proc/memory-objects/",
        "/proc/pending/",
        "/proc/media/",
        "/proc/self/",
        "/proc/session/",
        "/proc/services/",
        "/proc/scheduler/",
        "/proc/sources/",
        "/proc/syscalls/",
        "/proc/tasks/",
        "/proc/waits/",
    ] {
        if path.starts_with(file) {
            return Err(VfsError::NotDirectory);
        }
    }

    Err(VfsError::NotFound)
}

fn split_first(path: &str) -> (&str, &str) {
    if let Some(index) = path.as_bytes().iter().position(|&byte| byte == b'/') {
        (&path[..index], &path[index + 1..])
    } else {
        (path, "")
    }
}

fn find_device(devices: &[DeviceEntry], name: &str) -> Option<usize> {
    let mut index = 0usize;
    while index < devices.len() {
        let (class, ordinal) = device_name_parts(devices, index);
        if matches_device_name(name, class, ordinal) {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// Returns the stable VFS name parts for a device entry.
#[must_use]
pub fn device_name_parts(devices: &[DeviceEntry], index: usize) -> (&'static str, usize) {
    let class = device_class_name(devices[index].class);
    let mut ordinal = 0usize;
    let mut i = 0usize;
    while i < index {
        if devices[i].class == devices[index].class {
            ordinal += 1;
        }
        i += 1;
    }
    (class, ordinal)
}

fn matches_device_name(name: &str, class: &str, ordinal: usize) -> bool {
    let bytes = name.as_bytes();
    let prefix = class.as_bytes();
    if bytes.len() <= prefix.len() || !bytes.starts_with(prefix) {
        return false;
    }
    let mut value = 0usize;
    let mut i = prefix.len();
    while i < bytes.len() {
        let byte = bytes[i];
        if !byte.is_ascii_digit() {
            return false;
        }
        value = value
            .saturating_mul(10)
            .saturating_add((byte - b'0') as usize);
        i += 1;
    }
    value == ordinal
}

/// Human-readable class names used by `/dev` and `device` output.
#[must_use]
pub const fn device_class_name(class: DeviceClass) -> &'static str {
    match class {
        DeviceClass::Uart => "uart",
        DeviceClass::Interrupt => "interrupt",
        DeviceClass::Mailbox => "mailbox",
        DeviceClass::Block => "block",
        DeviceClass::Usb => "usb",
        DeviceClass::Bus => "bus",
        DeviceClass::Unknown => "unknown",
    }
}

#[cfg(feature = "selftest")]
#[path = "vfs_tests.rs"]
mod tests;
