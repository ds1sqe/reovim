//! A message through a socket pair in one process, and the defaults.

use reovim_host::{Error, dirs, link};

fn socket(test: &str) -> String {
    let name = format!("reovim-host-{}-{test}.sock", std::process::id());
    std::env::temp_dir().join(name).to_str().unwrap().to_string()
}

#[test]
fn a_message_crosses_the_link_whole() {
    let name = socket("whole");
    let listener = link::listen(&name).unwrap();
    let client = std::thread::spawn({
        let name = name.clone();
        move || {
            let end = link::open(&name).unwrap();
            link::send(&end, b"ping").unwrap();
            link::send(&end, b"").unwrap();
            let answer = link::recv(&end).unwrap();
            link::close(end);
            answer
        }
    });
    let end = link::accept(&listener).unwrap();
    assert_eq!(link::recv(&end).unwrap(), b"ping");
    assert_eq!(link::recv(&end).unwrap(), b"");
    link::send(&end, &vec![7; 70_000]).unwrap();
    assert_eq!(client.join().unwrap(), vec![7; 70_000]);
    // The other end closed: the next receive fails.
    assert_eq!(link::recv(&end), Err(Error::Io));
    // Listening again at the same name replaces the stale socket.
    drop(listener);
    link::listen(&name).unwrap();
    let _ = std::fs::remove_file(&name);
}

#[test]
fn nothing_listening_is_not_found() {
    assert_eq!(link::open(&socket("nobody")).map(|_| ()), Err(Error::NotFound));
}

#[test]
fn the_defaults_exist() {
    let runtime = dirs::runtime().unwrap();
    assert!(std::fs::metadata(&runtime).unwrap().is_dir());
    let work = dirs::work().unwrap();
    assert_eq!(work, std::env::current_dir().unwrap().to_str().unwrap());
}
