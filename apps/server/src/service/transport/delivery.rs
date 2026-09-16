use std::{
    io,
    string::String,
    sync::mpsc::{Receiver, RecvTimeoutError},
    time::Duration,
};

/// Wait only for dispatch evidence; a timeout supplies no operation status.
pub fn response(
    result: &Receiver<String>,
    timeout: Duration,
) -> io::Result<String> {
    result.recv_timeout(timeout).map_err(|error| match error {
        RecvTimeoutError::Timeout => io::Error::new(
            io::ErrorKind::TimedOut,
            "service dispatch response timed out",
        ),
        RecvTimeoutError::Disconnected => io::Error::new(
            io::ErrorKind::BrokenPipe,
            "service dispatch owner stopped",
        ),
    })
}
