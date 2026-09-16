//! The wire round-trips every message; the `Endpoint` knows what went
//! unanswered; a declaration parses back from its canonical bytes.

use reovim_kernel_interface::wire::{Endpoint, Message, decode, encode};
use reovim_kernel_interface::{
    Decl, Error, Event, Field, Flags, Handle, Hash, Info, Kind, Method, Op,
    Out, Rd, Reply, Rights, Token, Ty,
};

fn round_trip(seq: u64, message: &Message) {
    let bytes = encode(seq, message);
    assert_eq!(decode(&bytes), Some((seq, message.clone())), "{message:?}");
}

fn every_op() -> Vec<Op> {
    let h = Handle(7);
    vec![
        Op::Lookup { rd: Rd(1), name: "name".to_string() },
        Op::Open { rd: Rd(2), rights: Rights::READ | Rights::WRITE },
        Op::Read { h, at: 3, len: 4 },
        Op::Write { h, at: 5, bytes: b"six".to_vec() },
        Op::Call { h, api: Hash(8), method: 9, input: vec![10, 11] },
        Op::Watch { h, on: true },
        Op::Close { h },
        Op::Ask { token: Token(12) },
        Op::Cancel { token: Token(13) },
        Op::Since { cursor: 14 },
        Op::Info { rd: Rd(15) },
        Op::Parent { rd: Rd(16) },
        Op::Children { rd: Rd(17) },
    ]
}

fn every_out() -> Vec<Out> {
    let kinds = [
        Kind::Modify,
        Kind::Attrib,
        Kind::Create,
        Kind::Delete,
        Kind::Move,
        Kind::Gone,
        Kind::Overflow,
    ];
    let events = kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| Event {
            seq: i as u64,
            rd: Rd(20),
            kind: *kind,
            child: (i % 2 == 0).then_some(Rd(21)),
        })
        .collect();
    vec![
        Out::Unit,
        Out::Rd(Rd(1)),
        Out::Handle(Handle(2)),
        Out::Len(3),
        Out::Bytes(b"four".to_vec()),
        Out::Info(Info {
            name: Some("five".to_string()),
            flags: Flags::READ | Flags::CONTAINER,
            ctime: 6,
            mtime: 7,
            atime: 8,
            apis: vec![Hash(9), Hash(10)],
        }),
        Out::Info(Info {
            name: None,
            flags: Flags::empty(),
            ctime: 0,
            mtime: 0,
            atime: 0,
            apis: Vec::new(),
        }),
        Out::Rds(vec![Rd(11), Rd(12)]),
        Out::Events { events, cursor: 22 },
    ]
}

const ERRORS: [Error; 9] = [
    Error::Denied,
    Error::Unsupported,
    Error::Invalid,
    Error::Stale,
    Error::Busy,
    Error::NoSpace,
    Error::Closed,
    Error::Gone,
    Error::Cancelled,
];

#[test]
fn every_message_round_trips() {
    round_trip(
        0,
        &Message::Connect { credential: b"me".to_vec(), session: None },
    );
    round_trip(
        1,
        &Message::Connect { credential: Vec::new(), session: Some(Rd(5)) },
    );
    for (i, op) in every_op().into_iter().enumerate() {
        round_trip(i as u64, &Message::Request(op));
    }
    for (i, out) in every_out().into_iter().enumerate() {
        round_trip(i as u64, &Message::Reply(Reply::ok(out)));
    }
    for error in ERRORS {
        round_trip(u64::MAX, &Message::Reply(Reply::err(error)));
    }
    round_trip(3, &Message::Reply(Reply::Pending(Token(4))));
    round_trip(5, &Message::Load { path: "/x/hello.rvm".to_string() });
    round_trip(6, &Message::Unload { name: "hello".to_string() });
}

#[test]
fn malformed_bytes_decode_to_nothing() {
    let good = encode(1, &Message::Request(Op::Since { cursor: 2 }));
    assert!(decode(&good[..good.len() - 1]).is_none(), "truncated");
    let mut long = good.clone();
    long.push(0);
    assert!(decode(&long).is_none(), "trailing byte");
    let mut tag = good;
    tag[8] = 9;
    assert!(decode(&tag).is_none(), "unknown message tag");
    let bad_error = encode(1, &Message::Reply(Reply::err(Error::Denied)));
    let mut bad_error = bad_error;
    *bad_error.last_mut().unwrap() = 9;
    assert!(decode(&bad_error).is_none(), "unknown error tag");
    assert!(decode(&[]).is_none());
}

#[test]
fn the_endpoint_knows_what_went_unanswered() {
    let mut endpoint = Endpoint::default();
    let (first, bytes) =
        endpoint.send(&Message::Request(Op::Info { rd: Rd(1) }));
    let (second, _) =
        endpoint.send(&Message::Request(Op::Parent { rd: Rd(1) }));
    assert_ne!(first, second);
    assert_eq!(decode(&bytes).map(|(seq, _)| seq), Some(first));
    let stray = encode(99, &Message::Reply(Reply::ok(Out::Unit)));
    assert_eq!(endpoint.receive(&stray), None, "stray reply");
    assert_eq!(endpoint.receive(b"junk"), None, "undecodable");
    assert_eq!(endpoint.unanswered().len(), 2);

    let reply = encode(first, &Message::Reply(Reply::ok(Out::Rd(Rd(2)))));
    assert_eq!(
        endpoint.receive(&reply),
        Some((first, Message::Reply(Reply::ok(Out::Rd(Rd(2))))))
    );
    let waiting: Vec<u64> =
        endpoint.unanswered().iter().map(|(s, _)| *s).collect();
    assert_eq!(waiting, vec![second]);
    assert_eq!(endpoint.receive(&reply), None, "answered twice");
}

fn field(name: &str, ty: Ty) -> Field {
    Field { name: name.to_string(), ty }
}

/// A declaration using every type once, nested where a type can nest.
fn wide() -> Decl {
    let record = Ty::Record(vec![
        field("unit", Ty::Unit),
        field("bool", Ty::Bool),
        field("u8", Ty::U8),
        field("u32", Ty::U32),
        field("u64", Ty::U64),
        field("i64", Ty::I64),
        field("f64", Ty::F64),
        field("bytes", Ty::Bytes(16)),
        field("utf8", Ty::Utf8(32)),
        field("rd", Ty::Rd),
    ]);
    let choice = Ty::Choice(vec![
        field("none", Ty::Unit),
        field("some", Ty::Sequence(4, Box::new(record.clone()))),
        field("fixed", Ty::Array(2, Box::new(Ty::Rd))),
    ]);
    let m = |number, name: &str, input, output, right| Method {
        number,
        name: name.to_string(),
        input,
        output,
        right,
    };
    Decl {
        api: "wide".to_string(),
        ver: 3,
        methods: vec![
            m(2, "later", Ty::Unit, choice, Rights::READ),
            m(0, "get", Ty::Unit, record, Rights::READ),
            m(1, "set", Ty::Bytes(16), Ty::Unit, Rights::WRITE),
        ],
    }
}

#[test]
fn a_declaration_parses_back_from_its_canonical_bytes() {
    let decl = wide();
    let parsed = Decl::parse(&decl.canonical()).unwrap();
    assert_eq!(parsed.hash(), decl.hash());
    assert_eq!(parsed.api, decl.api);
    assert_eq!(parsed.ver, decl.ver);
    // Canonical order is by number.
    let numbers: Vec<u16> = parsed.methods.iter().map(|m| m.number).collect();
    assert_eq!(numbers, vec![0, 1, 2]);
    for method in &decl.methods {
        assert_eq!(parsed.method(method.number), Some(method));
    }
    assert_eq!(parsed.canonical(), decl.canonical());

    let bytes = decl.canonical();
    assert_eq!(Decl::parse(&bytes[..bytes.len() - 1]), Err(Error::Invalid));
    let mut long = bytes;
    long.push(0);
    assert_eq!(Decl::parse(&long), Err(Error::Invalid));
    assert_eq!(Decl::parse(&[]), Err(Error::Invalid));

    let mut twice = decl;
    twice.methods.push(twice.methods[0].clone());
    assert_eq!(Decl::parse(&twice.canonical()), Err(Error::Invalid), "check");
}
