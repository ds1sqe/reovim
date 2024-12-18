use rustix::fd::{BorrowedFd, OwnedFd};
use std::{io, os::fd::AsFd};

pub enum FileDesc<'a> {
    Owned(OwnedFd),
    Borrowed(BorrowedFd<'a>),
}

pub fn tty_fd() -> io::Result<FileDesc<'static>> {
    use std::fs::File;

    let stdin = rustix::stdio::stdin();
    let fd = if rustix::termios::isatty(stdin) {
        FileDesc::Borrowed(stdin)
    } else {
        let dev_tty = File::options().read(true).write(true).open("/dev/tty")?;
        FileDesc::Owned(dev_tty.into())
    };
    Ok(fd)
}

impl AsFd for FileDesc<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        match self {
            FileDesc::Owned(fd) => fd.as_fd(),
            FileDesc::Borrowed(fd) => fd.as_fd(),
        }
    }
}
