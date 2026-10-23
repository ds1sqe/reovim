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

/// Every kind an event and an `Op::Changed` spell with the one tag.
const KINDS: [Kind; 7] = [
    Kind::Modify,
    Kind::Attrib,
    Kind::Create,
    Kind::Delete,
    Kind::Move,
    Kind::Gone,
    Kind::Overflow,
];

fn every_op() -> Vec<Op> {
    let h = Handle(7);
    // `Op::Changed` carries `Modify` and `Attrib` alone, which is what
    // `Calls::changed` takes; the codec spells all seven all the same,
    // the kind being one tag wherever it stands.
    let changed = KINDS.map(|kind| Op::Changed { rd: Rd(21), kind });
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
        Op::Done { token: Token(18), outcome: Ok(Out::Rd(Rd(19))) },
        Op::Done { token: Token(20), outcome: Err(Error::Gone) },
    ]
    .into_iter()
    .chain(changed)
    .collect()
}

/// What a `serve` list carries: the token the answering `Op::Done`
/// names again, and one op in the Client's own rd and handle space.
fn served() -> Vec<(Token, Op)> {
    vec![
        (Token(30), Op::Lookup { rd: Rd(31), name: "dev".to_string() }),
        (Token(32), Op::Read { h: Handle(33), at: 34, len: 35 }),
    ]
}

fn every_out() -> Vec<Out> {
    let events = KINDS
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
        Out::Events { events, cursor: 22, serve: Vec::new() },
        Out::Events { events: Vec::new(), cursor: 23, serve: served() },
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

/// Every strict prefix of one encoded message is refused. A frame the
/// transport cut short is no frame: the codec says so rather than
/// reading past what it was given or taking a part for the whole.
fn every_prefix_is_refused(message: &Message) {
    let bytes = encode(9, message);
    for len in 0..bytes.len() {
        assert!(decode(&bytes[..len]).is_none(), "{len} of {message:?}");
    }
}

#[test]
fn a_frame_cut_inside_a_nested_body_is_refused_at_every_length() {
    // The `Out` shapes that carry a length of their own — a byte
    // string, a count of rds, a name and a list of apis, a list of
    // events and a list of served ops — each inside the `Op::Done`
    // that hands it back, where the length stands two bodies deep.
    let nested = every_out().into_iter().filter(|out| {
        matches!(
            out,
            Out::Bytes(_) | Out::Rds(_) | Out::Info(_) | Out::Events { .. }
        )
    });
    for out in nested {
        let done = Op::Done { token: Token(18), outcome: Ok(out) };
        every_prefix_is_refused(&Message::Request(done));
    }
    for kind in KINDS {
        let changed = Op::Changed { rd: Rd(21), kind };
        every_prefix_is_refused(&Message::Request(changed));
    }
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
    let record = Ty::Record(
        "test.Record".into(),
        vec![
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
        ],
    );
    let choice = Ty::Choice(
        "test.Choice".into(),
        vec![
            field("none", Ty::Unit),
            field("some", Ty::Sequence(4, Box::new(record.clone()))),
            field("fixed", Ty::Array(2, Box::new(Ty::Rd))),
        ],
    );
    let m = |number, name: &str, input, output, right| Method {
        number,
        name: name.to_string(),
        input_name: if input == Ty::Unit { None } else { Some("input".into()) },
        input,
        output,
        right,
    };
    Decl {
        api: "wide".to_string(),
        ver: 3,
        methods: vec![
            m(2, "later", Ty::Unit, choice, Rights::READ),
            m(0, "get", Ty::Unit, record.clone(), Rights::READ),
            m(1, "set", Ty::Bytes(16), Ty::Unit, Rights::WRITE),
        ],
        records: vec![
            ("Wide".to_string(), fields_of(record)),
            ("Empty".to_string(), Vec::new()),
        ],
    }
}

/// The fields of a `Ty::Record`, which is the only shape a declared
/// record has.
fn fields_of(ty: Ty) -> Vec<Field> {
    match ty {
        Ty::Record(_, fields) => fields,
        other => panic!("no record: {other:?}"),
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
    assert_eq!(parsed.records, decl.records, "the records came back");
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

/// A declaration's records are part of what it is: a record-only api
/// — `keyboard`, `resolver` — declares no method at all, so its name
/// and version are the whole of its identity unless the records it
/// names are hashed too, and a reshaped record passes the Kernel's
/// check as the shape both sides already agreed on.
#[test]
fn a_records_field_list_changes_the_hash() {
    let only = |fields: Vec<Field>| Decl {
        api: "only".to_string(),
        ver: 1,
        methods: Vec::new(),
        records: vec![("Input".to_string(), fields)],
    };
    let one = vec![field("press", Ty::U64)];
    let decl = only(one.clone());

    assert_eq!(decl.hash(), only(one.clone()).hash(), "two builds agree");
    assert_ne!(
        decl.hash(),
        only(vec![field("pressed", Ty::U64)]).hash(),
        "a field renamed"
    );
    assert_ne!(
        decl.hash(),
        only(vec![field("press", Ty::U32)]).hash(),
        "a field retyped"
    );
    let two = vec![field("press", Ty::U64), field("origin", Ty::U64)];
    assert_ne!(decl.hash(), only(two).hash(), "a field added");
    assert_ne!(
        decl.hash(),
        Decl { records: Vec::new(), ..only(one) }.hash(),
        "the record dropped"
    );

    // The name is the record's own, and the order is the declared one.
    let mut renamed = decl.clone();
    renamed.records[0].0 = "Output".to_string();
    assert_ne!(decl.hash(), renamed.hash(), "a record renamed");

    let parsed = Decl::parse(&decl.canonical()).unwrap();
    assert_eq!(parsed, decl, "and every byte of it parses back");
}

/// One record whose one field is `ty`, as one method's input: the
/// smallest declaration a shape can be held in.
fn holding(ty: Ty) -> Decl {
    let method = Method {
        number: 0,
        name: "run".to_string(),
        input_name: Some("input".into()),
        input: Ty::Record("test.Record".into(), vec![field("spec", ty)]),
        output: Ty::Rd,
        right: Rights::READ,
    };
    Decl {
        api: "one".to_string(),
        ver: 1,
        methods: vec![method],
        records: Vec::new(),
    }
}

/// `rest(max)` is `utf8(max)`'s bytes under a tag of its own: one
/// value of either is the same on the wire and bounded the same way,
/// and a declaration holding one differs from the same declaration
/// holding the other in that one byte — so the two are never one hash
/// and a shell can never read them alike.
#[test]
fn a_rest_encodes_as_utf8_with_its_own_tag() {
    use reovim_kernel_interface::value::fits;

    // The value: a u32 length and that many bytes of UTF-8, `Utf8`'s
    // rule to the byte, the bound and the UTF-8 check included.
    let value = |text: &str| {
        let len = u32::try_from(text.len()).unwrap();
        let mut out = len.to_le_bytes().to_vec();
        out.extend_from_slice(text.as_bytes());
        out
    };
    for text in ["", "a | b > c", "héllo"] {
        let bytes = value(text);
        assert!(fits(&bytes, &Ty::Rest(32)), "{text}");
        assert_eq!(fits(&bytes, &Ty::Rest(32)), fits(&bytes, &Ty::Utf8(32)));
    }
    let long = value("0123456789");
    assert!(!fits(&long, &Ty::Rest(4)), "past its own bound");
    let mut bad = 1u32.to_le_bytes().to_vec();
    bad.push(0xff);
    assert!(!fits(&bad, &Ty::Rest(32)), "no UTF-8");

    // The canonical form: one byte apart, and that byte is the tag.
    let rest = holding(Ty::Rest(32)).canonical();
    let utf8 = holding(Ty::Utf8(32)).canonical();
    assert_eq!(rest.len(), utf8.len());
    let apart: Vec<usize> = rest
        .iter()
        .zip(&utf8)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(at, _)| at)
        .collect();
    assert_eq!(apart.len(), 1, "the tag and nothing else");
    assert_eq!((rest[apart[0]], utf8[apart[0]]), (14, 8));
    assert_ne!(holding(Ty::Rest(32)).hash(), holding(Ty::Utf8(32)).hash());

    // And it parses back to itself.
    assert_eq!(Decl::parse(&rest), Ok(holding(Ty::Rest(32))));
}

/// A `rest` is the rest of a line: a line has one, and it ends with
/// it. `check` refuses a declaration that puts one anywhere else, so
/// a Driver that spells it wrong never reports at all.
#[test]
fn a_rest_declaration_must_be_last_and_alone() {
    let two = |first: Ty, second: Ty| {
        let method = Method {
            number: 0,
            name: "run".to_string(),
            input_name: Some("input".into()),
            input: Ty::Record(
                "test.Record".into(),
                vec![field("a", first), field("b", second)],
            ),
            output: Ty::Rd,
            right: Rights::READ,
        };
        Decl {
            api: "two".to_string(),
            ver: 1,
            methods: vec![method],
            records: Vec::new(),
        }
    };
    // Last is where it stands.
    let ok = two(Ty::Utf8(8), Ty::Rest(64));
    assert_eq!(Decl::parse(&ok.canonical()), Ok(ok));
    // Before another field, twice over, inside a sequence, and as a
    // whole input of its own: none of them.
    for decl in [
        two(Ty::Rest(64), Ty::Utf8(8)),
        two(Ty::Rest(8), Ty::Rest(8)),
        holding(Ty::Sequence(4, Box::new(Ty::Rest(8)))),
        holding(Ty::Choice(
            "test.Choice".into(),
            vec![field("one", Ty::Rest(8))],
        )),
    ] {
        assert_eq!(Decl::parse(&decl.canonical()), Err(Error::Invalid));
    }
    let bare = Decl {
        api: "bare".to_string(),
        ver: 1,
        methods: vec![Method {
            number: 0,
            name: "run".to_string(),
            input_name: Some("input".into()),
            input: Ty::Rest(8),
            output: Ty::Rd,
            right: Rights::READ,
        }],
        records: Vec::new(),
    };
    assert_eq!(Decl::parse(&bare.canonical()), Err(Error::Invalid));
}

#[test]
fn named_types_of_one_shape_have_distinct_identities() {
    for named in [Ty::Record, Ty::Choice] {
        let fields = vec![field("value", Ty::U64)];
        let mut first = wide();
        first.methods.sort_by_key(|method| method.number);
        first.methods[0].output = named("wide.First".into(), fields.clone());
        let mut second = first.clone();
        second.methods[0].output = named("wide.Second".into(), fields);
        assert_ne!(first.hash(), second.hash());
        assert_eq!(Decl::parse(&first.canonical()), Ok(first));
        assert_eq!(Decl::parse(&second.canonical()), Ok(second));
    }
}
