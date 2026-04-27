//! Tiny HTTP/1.1 responder for the Flight-76 canonical-frame spike.
//!
//! No hyper, no axum. A hand-rolled loop-accept responder is less code
//! than any framework bootstrap for a one-route / one-body spike. The
//! server serves the same body on every path — the spike only cares
//! that `reovim-web` returns a canonical rendering, not that it
//! implements routing.
//!
//! ## Request read cap (locked)
//!
//! The header read is bounded at **8 KiB**. A client that never sends
//! `\r\n\r\n` is dropped without a response once the cap is hit. This
//! is the only hard guard against a trivial slowloris.

use std::{io, net::SocketAddr};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// Maximum bytes read per connection while waiting for `\r\n\r\n`.
///
/// Documented in the module header.
pub(crate) const HEADER_READ_CAP: usize = 8 * 1024;

/// Accept connections on `listen` and serve `body_fn()` on every request.
///
/// `body_fn` is called once per request so tests can substitute a
/// fixed-body stub. Runs until `cancel` completes — pass
/// `std::future::pending()` for the production default, or
/// `tokio::signal::ctrl_c()` in the standalone bin.
///
/// # Errors
///
/// Returns an error if binding `listen` fails. Per-connection errors
/// are logged to stderr and the connection is dropped; one bad client
/// does not take down the server.
pub async fn serve<F, C>(listen: SocketAddr, body_fn: F, cancel: C) -> io::Result<()>
where
    F: Fn() -> String + Send + Sync + 'static,
    C: std::future::Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(listen).await?;
    serve_with_listener(listener, body_fn, cancel).await
}

/// Variant of [`serve`] for tests that need to bind on `127.0.0.1:0`
/// and read back the assigned port before spawning the loop.
///
/// # Errors
///
/// Returns an error if the accept loop is terminated by an I/O
/// failure that is not recoverable per-connection. Per-connection
/// errors are logged and dropped, matching [`serve`].
pub async fn serve_with_listener<F, C>(
    listener: TcpListener,
    body_fn: F,
    cancel: C,
) -> io::Result<()>
where
    F: Fn() -> String + Send + Sync + 'static,
    C: std::future::Future<Output = ()> + Send + 'static,
{
    let body_fn = std::sync::Arc::new(body_fn);
    tokio::pin!(cancel);
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _addr)) => {
                        let body_fn = body_fn.clone();
                        tokio::spawn(async move {
                            let body = body_fn();
                            let _ = handle_connection(stream, &body).await;
                        });
                    }
                    Err(err) => {
                        eprintln!("reovim-web: accept error: {err}");
                    }
                }
            }
            () = &mut cancel => return Ok(()),
        }
    }
}

/// Read the request headers up to the 8-KiB cap, then write the
/// canonical response. Silently drops the connection on any error or
/// cap-overflow — the spike does not report 4xx/5xx.
async fn handle_connection(mut stream: TcpStream, body: &str) -> io::Result<()> {
    let mut buf = [0u8; HEADER_READ_CAP];
    let mut filled: usize = 0;
    loop {
        if filled == buf.len() {
            // Cap hit without finding the request terminator — drop.
            return Ok(());
        }
        let n = stream.read(&mut buf[filled..]).await?;
        if n == 0 {
            // Peer closed before sending headers.
            return Ok(());
        }
        filled += n;
        if has_double_crlf(&buf[..filled]) {
            break;
        }
    }

    let resp = build_response(body);
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

pub(crate) fn has_double_crlf(buf: &[u8]) -> bool {
    buf.windows(4).any(|w| w == b"\r\n\r\n")
}

pub(crate) fn build_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        len = body.len(),
    )
}
