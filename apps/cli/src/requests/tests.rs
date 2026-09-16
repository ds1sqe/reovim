use super::*;
use std::{
    io::BufReader,
    net::{TcpListener, TcpStream},
    path::PathBuf,
};

struct Transcript(PathBuf);
impl Drop for Transcript {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}
#[test]
fn reopen_after_lost_allocation_reply_sends_exact_original_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let transcript = Transcript(std::env::temp_dir().join(format!(
        "reovim-cli-wire-{}-{}",
        std::process::id(),
        listener.local_addr().unwrap().port()
    )));
    let peer = std::thread::spawn(move || lost_reply(&listener));
    let mut client = Client::open(&address, [7; 16], &transcript.0).unwrap();
    assert!(client.exchange(Action::Reserve(1)).is_err());
    drop(client);
    let mut client = Client::open(&address, [7; 16], &transcript.0).unwrap();
    assert_eq!(client.retry().unwrap(), Reply::Reserved(81));
    assert_eq!(client.journal.next().unwrap(), 2);
    assert_eq!(client.receipts().copied().collect::<Vec<_>>(), vec![81]);
    drop(client);
    peer.join().unwrap();
}
fn lost_reply(listener: &TcpListener) {
    let first = receive(accept(listener));
    // The allocated ID exists, but the observation socket closes without it.
    let mut stream = accept(listener);
    let repeated = receive(stream.try_clone().unwrap());
    assert_eq!(first, repeated);
    assert_eq!(first.action, Action::Reserve(1));
    reovim_service::transport::send(
        &mut stream,
        &Reply::Reserved(81).encode().unwrap(),
    )
    .unwrap();
}
fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) => assert_eq!(error.kind(), io::ErrorKind::WouldBlock),
        }
        assert!(Instant::now() < deadline, "client did not connect");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn receive(stream: TcpStream) -> Frame {
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let line = reovim_service::transport::receive(&mut BufReader::new(stream))
        .unwrap();
    Frame::decode(&line).unwrap()
}
