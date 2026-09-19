//! `load` and `unload` through a real server: hello.rvm built by the
//! system cc, driven by `reovim cli`.

mod common;

use common::Server;
use reovim_host::link;
use reovim_kernel_interface::wire::{Endpoint, Message};
use reovim_kernel_interface::{Op, Out, Rd, Reply, Rights};

#[test]
fn hello_loads_and_unloads_through_the_cli() {
    let server = Server::start("hello");
    let hello = server.hello();
    assert_eq!(
        server.ran(&["load", "/nowhere.rvm"]),
        Err("reovim: load /nowhere.rvm: invalid".into())
    );
    assert_eq!(server.ran(&["load", &hello]), Ok(String::new()));
    assert_eq!(
        server.ran(&["load", &hello]),
        Err(format!("reovim: load {hello}: invalid")),
        "taken"
    );
    assert_eq!(server.ran(&["cat", "/module/hello/count"]), Ok("0".into()));
    // `>` is what puts bytes into an rd: the output of a run, here the
    // one byte a file on the server's own disk holds.
    server.file("seven", b"7");
    let poured = ["cat", "/module/fs/root/seven", ">", "/module/hello/count"];
    assert_eq!(server.ran(&poured), Ok(String::new()));
    assert_eq!(server.ran(&["cat", "/module/hello/count"]), Ok("7".into()));
    assert_eq!(
        server.ran(&["unload", "fs"]),
        Err("reovim: unload fs: denied".into())
    );

    // A raw link holds a handle open: unload waits for it.
    let end = link::open(&server.socket).unwrap();
    let mut endpoint = Endpoint::default();
    let mut ask = |message: &Message| {
        let (_, bytes) = endpoint.send(message);
        link::send(&end, &bytes).unwrap();
        let (_, reply) = endpoint.receive(&link::recv(&end).unwrap()).unwrap();
        reply
    };
    ask(&Message::Connect { credential: Vec::new(), session: None });
    let count: u64 = server.rd("/module/hello/count").parse().unwrap();
    let open = Op::Open { rd: Rd(count), rights: Rights::READ };
    let Message::Reply(Reply::Ready(Ok(Out::Handle(h)))) =
        ask(&Message::Request(open))
    else {
        panic!("open")
    };
    assert_eq!(
        server.ran(&["unload", "hello"]),
        Err("reovim: unload hello: busy; try again".into())
    );
    ask(&Message::Request(Op::Close { h }));
    assert_eq!(server.ran(&["unload", "hello"]), Ok(String::new()));
    assert_eq!(
        server.ran(&["stat", "/module/hello"]),
        Err("reovim: stat /module/hello: invalid".into())
    );
    let log = server.line(&["log"]);
    assert!(
        log.contains("load module=hello")
            && log.contains("unload module=hello"),
        "{log}"
    );
}
