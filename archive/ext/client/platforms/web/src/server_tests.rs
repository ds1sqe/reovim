use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use crate::{
    frame::canonical_frame,
    server::{HEADER_READ_CAP, build_response, has_double_crlf, serve_with_listener},
    svg::render_page,
};

async fn bind_ephemeral() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

async fn get_root(port: u16) -> (Vec<u8>, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
        .await
        .unwrap();
    stream.flush().await.unwrap();
    let mut buf = Vec::new();
    tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut buf))
        .await
        .unwrap()
        .unwrap();
    let text = String::from_utf8_lossy(&buf).into_owned();
    (buf, text)
}

#[tokio::test]
async fn serve_responds_200_with_provided_body() {
    let body = "hello-test-body";
    let (listener, port) = bind_ephemeral().await;
    let (tx, rx) = oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = serve_with_listener(listener, move || body.to_string(), async move {
            let _ = rx.await;
        })
        .await;
    });

    let (_raw, text) = get_root(port).await;
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "got: {text}");
    assert!(
        text.contains("Content-Type: text/html; charset=utf-8\r\n"),
        "missing content-type: {text}"
    );
    assert!(
        text.contains(&format!("Content-Length: {}\r\n", body.len())),
        "missing/wrong content-length: {text}"
    );
    let (_, after_headers) = text.split_once("\r\n\r\n").expect("split headers");
    assert_eq!(after_headers, body);

    let _ = tx.send(());
    let _ = handle.await;
}

#[tokio::test]
async fn serve_responds_to_repeated_connections() {
    let body = "repeat-me";
    let (listener, port) = bind_ephemeral().await;
    let (tx, rx) = oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = serve_with_listener(listener, move || body.to_string(), async move {
            let _ = rx.await;
        })
        .await;
    });

    for _ in 0..3 {
        let (_, text) = get_root(port).await;
        let (_, after) = text.split_once("\r\n\r\n").expect("split headers");
        assert_eq!(after, body);
    }

    let _ = tx.send(());
    let _ = handle.await;
}

#[tokio::test]
async fn serve_drops_oversized_headers_without_response() {
    let (listener, port) = bind_ephemeral().await;
    let (tx, rx) = oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = serve_with_listener(listener, || "body".to_string(), async move {
            let _ = rx.await;
        })
        .await;
    });

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    // Write 16 KiB of header junk with no CRLFCRLF terminator.
    let junk = vec![b'A'; 16 * 1024];
    // Writing may succeed fully or fail partway once the server drops
    // the connection — either is acceptable. We only assert that the
    // server responds with nothing.
    let _ = stream.write_all(&junk).await;
    let _ = stream.shutdown().await;

    let mut buf = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut buf)).await;
    assert!(
        buf.is_empty(),
        "server should not respond when the 8-KiB header cap is breached; got {} bytes",
        buf.len()
    );

    let _ = tx.send(());
    let _ = handle.await;
}

#[tokio::test]
async fn serve_responds_with_canonical_frame() {
    let (listener, port) = bind_ephemeral().await;
    let (tx, rx) = oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = serve_with_listener(listener, || render_page(&canonical_frame()), async move {
            let _ = rx.await;
        })
        .await;
    });

    let (_, text) = get_root(port).await;
    let (_, body) = text.split_once("\r\n\r\n").expect("split headers");
    // Body must contain the canonical SVG title and the slategray-tinted
    // HELLO line.
    assert!(
        body.contains("<title>reovim-web — canonical frame</title>"),
        "canonical title missing"
    );
    assert!(body.contains("fill=\"slategray\""), "canonical slategray glyphs missing");

    let _ = tx.send(());
    let _ = handle.await;
}

#[test]
fn build_response_formats_headers_and_body() {
    let r = build_response("abc");
    assert!(r.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(r.contains("Content-Length: 3\r\n"));
    assert!(r.ends_with("\r\nabc"));
}

#[test]
fn has_double_crlf_detects_terminator() {
    assert!(has_double_crlf(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"));
    assert!(!has_double_crlf(b"GET / HTTP/1.1\r\n"));
    assert!(!has_double_crlf(b""));
}

#[test]
fn header_read_cap_is_8_kib() {
    // Guard against accidental reduction. Documented in module header.
    assert_eq!(HEADER_READ_CAP, 8 * 1024);
}
