//! Committed declarations compile with the published language.

use fipe::decl::Declarations;
use fipe::embed::Outcome;
use fipe::session::Session;

#[test]
fn committed_progress_matches() {
    assert_eq!(
        include_str!("../sdk.progress.fipe"),
        reovim_sdk::progress::progress::text("sdk")
    );
}

fn declarations() -> Declarations {
    let mut decls = Declarations::new();
    decls.commands(include_str!("../commands.fipe")).unwrap();
    let modules = [
        ("text.open", include_str!("../../../modules/text/api/text.open.fipe")),
        ("text.text", include_str!("../../../modules/text/api/text.text.fipe")),
        (
            "text.history",
            include_str!("../../../modules/text/api/text.history.fipe"),
        ),
        (
            "text.publish",
            include_str!("../../../modules/text/api/text.publish.fipe"),
        ),
        (
            "search.search",
            include_str!("../../../modules/search/api/search.search.fipe"),
        ),
        ("fs.fs", include_str!("../../../modules/fs/fs.fs.fipe")),
        (
            "client.keyboard",
            include_str!("../../../clients/core/api/client.keyboard.fipe"),
        ),
        (
            "client.window",
            include_str!("../../../clients/core/api/client.window.fipe"),
        ),
        (
            "client.display",
            include_str!("../../../clients/core/api/client.display.fipe"),
        ),
        (
            "client.looks",
            include_str!("../../../clients/core/api/client.looks.fipe"),
        ),
        (
            "client.open",
            include_str!("../../../clients/core/api/client.open.fipe"),
        ),
        (
            "client.buffers",
            include_str!("../../../clients/core/api/client.buffers.fipe"),
        ),
        (
            "client.draft",
            include_str!("../../../clients/core/api/client.draft.fipe"),
        ),
        (
            "client.resolver",
            include_str!("../../../clients/core/api/client.resolver.fipe"),
        ),
        ("sdk.progress", include_str!("../../../sdk/rust/sdk.progress.fipe")),
    ];
    for (name, source) in modules {
        decls.module(name, source).unwrap_or_else(|errors| {
            panic!("{name}: {errors:?}");
        });
    }
    decls
}

#[test]
fn every_rendering_loads_and_checks_a_call() {
    let decls = declarations();
    let open = decls.get_module("text.open").unwrap();
    assert_eq!(
        open.module.doc.as_deref(),
        Some(concat!(
            "`/module/text/open`: a Buffer over any rd, or over nothing:\n",
            "`new` makes one that is empty and cannot save.\n",
            "open api, version 1.",
        ))
    );
    let fipe::ast::Entry::Item(item) = &open.module.entries[0] else {
        panic!("the first declaration is not a signature");
    };
    assert_eq!(item.doc.as_deref(), Some("Needs WRITE."));
    let mut session = Session::new(decls);
    let Outcome::Done(report) = session.line("use text.open") else {
        panic!("host module resolved as a file");
    };
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let Outcome::Done(report) =
        session.line("call = h source => text.open.open h -source source")
    else {
        panic!("call resolved as a file")
    };
    assert!(report.diagnostics.is_empty(), "{report:?}");
    assert!(report.accepted.is_some());
    let Outcome::Done(report) =
        session.line("bad = h source => text.open.open h -wrong source")
    else {
        panic!("bad call resolved as a file")
    };
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(diagnostic, fipe::session::Diagnostic::Type(error)
            if matches!(&error.reason,
                fipe::infer::Reason::UnknownSpot { name } if name == "wrong"))
        }),
        "{report:?}"
    );
    assert!(report.accepted.is_none());
}

#[test]
fn every_wire_type_renders_as_a_checked_host_type() {
    use reovim_kernel_interface::{Decl, Field, Method, Rights, Ty};

    let field = |name: &str, ty| Field { name: name.into(), ty };
    let fields = vec![
        field("unit", Ty::Unit),
        field("boolean", Ty::Bool),
        field("byte", Ty::U8),
        field("word", Ty::U32),
        field("long", Ty::U64),
        field("signed", Ty::I64),
        field("float", Ty::F64),
        field("bytes", Ty::Bytes(16)),
        field("text", Ty::Utf8(16)),
        field("rd", Ty::Rd),
        field("sequence", Ty::Sequence(4, Box::new(Ty::U8))),
        field("array", Ty::Array(2, Box::new(Ty::U32))),
        field("tail", Ty::Rest(32)),
    ];
    let record = Ty::Record("sample.Record".into(), fields);
    let choice = Ty::Choice(
        "sample.Choice".into(),
        vec![field("Empty", Ty::Unit), field("Item", record.clone())],
    );
    let decl = Decl {
        api: "sample".into(),
        ver: 2,
        records: vec![],
        methods: vec![Method {
            number: 0,
            name: "choose".into(),
            input_name: Some("record".into()),
            input: record,
            output: choice,
            right: Rights::READ,
        }],
    };
    decl.check().unwrap();
    let mut decls = Declarations::new();
    decls.commands(include_str!("../commands.fipe")).unwrap();
    decls.module("sample", &decl.render()).unwrap();
    let mut session = Session::new(decls);
    let Outcome::Done(report) =
        session.line("use sample\nchoose = h => sample.choose h")
    else {
        panic!("host declaration requested a file")
    };
    assert!(report.diagnostics.is_empty(), "{report:?}");
    assert!(report.accepted.is_some());
}

#[test]
fn nested_host_namespaces_keep_open_exports_distinct() {
    let mut session = Session::new(declarations());
    let lines = [
        "use text.open",
        "make_text = h => open.new h",
        "make_full = h => text.open.new h",
        "use client.open",
        concat!(
            "show_client = h target => open.open h -target target ",
            "-plugin \"\" -name \"\" -line 0 -locator \"\""
        ),
        "open_text = h source => text.open.open h -source source",
        "use text.open.new",
        "make_imported = h => new h",
        "use client.resolver",
        "input = press => client.resolver.Input press 0 -released=False",
    ];
    for line in lines {
        let Outcome::Done(report) = session.line(line) else {
            panic!("host namespace requested a file: {line}");
        };
        assert!(report.diagnostics.is_empty(), "{line}: {report:?}");
        assert!(report.accepted.is_some(), "{line}: {report:?}");
    }
}
