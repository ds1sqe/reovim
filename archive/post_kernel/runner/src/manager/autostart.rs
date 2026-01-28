//! Manager auto-start logic.
//!
//! Ensures the manager daemon is running when needed, starting it
//! automatically if necessary.

use std::{
    io::{self, BufRead, BufReader},
    process::{Command, Stdio},
    time::Duration,
};

use tokio::{net::TcpStream, time::timeout};

use super::{MANAGER_HOST, MANAGER_PORT};

/// Timeout for manager startup.
const MANAGER_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for checking if manager is alive.
const MANAGER_ALIVE_TIMEOUT: Duration = Duration::from_millis(100);

/// Ensure the manager daemon is running, starting it if necessary.
///
/// This function:
/// 1. Checks if the manager is already running
/// 2. If not, spawns a new manager process
/// 3. Waits for the manager to signal readiness
///
/// # Errors
///
/// Returns an error if:
/// - The manager cannot be started
/// - The manager fails to become ready within the timeout
pub async fn ensure_manager_running() -> io::Result<()> {
    if is_manager_alive().await {
        return Ok(());
    }

    // Start manager process
    let exe = std::env::current_exe()?;
    let mut child = Command::new(&exe)
        .args(["manager", "start", "--ready-signal"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    // Wait for ready signal (stdout is piped, so this should always succeed)
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return Err(io::Error::other("Failed to capture manager stdout"));
    };
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // Use a blocking read with timeout
    // We need to do this synchronously because child.stdout is not async
    let start = std::time::Instant::now();
    loop {
        if start.elapsed() > MANAGER_STARTUP_TIMEOUT {
            let _ = child.kill();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Manager failed to start within timeout",
            ));
        }

        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // EOF - manager exited
                let _ = child.kill();
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Manager exited before signaling ready",
                ));
            }
            Ok(_) => {
                if line.starts_with("READY") {
                    // Manager is ready, detach from child
                    drop(child);
                    return Ok(());
                }
                // Not a ready signal, continue reading
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Non-blocking read returned no data, sleep and retry
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(e);
            }
        }
    }
}

/// Check if the manager daemon is alive and responding.
///
/// Attempts a quick TCP connection to the manager port.
pub async fn is_manager_alive() -> bool {
    is_manager_alive_at(MANAGER_HOST, MANAGER_PORT).await
}

/// Check if the manager daemon is alive at a specific address.
pub async fn is_manager_alive_at(host: &str, port: u16) -> bool {
    timeout(MANAGER_ALIVE_TIMEOUT, async { TcpStream::connect((host, port)).await.is_ok() })
        .await
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_manager_alive_not_running() {
        // Use a port that's unlikely to have anything listening
        let result = is_manager_alive_at("127.0.0.1", 59999).await;
        assert!(!result);
    }
}
