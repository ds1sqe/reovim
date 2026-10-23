use super::*;
use reovim_sdk::dynamic::{self, Value};

fn load(source: &str, stem: &str) -> (crate::exec::Executor, Vec<Export>) {
    let rig = reovim_rig::Rig::new(&format!("fipe-module-{stem}"));
    rig.file(&format!("{stem}.fipe"), source.as_bytes());
    let path = format!("/module/fs/root/{stem}.fipe");
    let host = rig.host();
    let ctx = host.own();
    let mut executor = crate::exec::Executor::new().unwrap();
    let (report, output) = executor.load(&ctx, &path, source, 1000).unwrap();
    assert!(report.unit.is_some(), "{output}");
    let exports = exports(&executor.session, &path, stem, source).unwrap();
    (executor, exports)
}

#[test]
fn written_integer_widths_are_kept_in_exported_fields() {
    let (_, exports) = load("pub echo: U8 -> U8 = byte => byte", "width");
    let Ty::Record(_, fields) = &exports[0].decl.methods[0].input else {
        panic!("input record")
    };
    assert_eq!(fields[0].name, "byte");
    assert_eq!(fields[0].ty, Ty::U8);
    assert_eq!(exports[0].result, Ty::U8);
}

#[test]
fn file_record_fields_keep_their_types_and_meanings() {
    let (executor, exports) = load(
        concat!(
            "pub type Point { pub x: U8 }\n",
            "pub echo: Point -> Point = point => point\n",
            "pub again: Point -> Point = point => point",
        ),
        "record",
    );
    let Ty::Record(name, fields) = &exports[0].result else { panic!("record") };
    assert_eq!(name, "record.Point");
    assert_eq!(exports[0].result, exports[1].result);
    assert_eq!(fields[0].ty, Ty::U8);
    let rig = reovim_rig::Rig::new("fipe-record");
    let host = rig.host();
    let ctx = host.own();
    let value = Value::Record(alloc::vec![(
        "point".into(),
        Value::Record(alloc::vec![("x".into(), Value::U8(7))])
    )]);
    let input =
        dynamic::encode(&exports[0].decl.methods[0].input, &value).unwrap();
    let output = executor.invoke(&ctx, &exports[0], &input, 1000).unwrap();
    assert_eq!(output, b"{x=7}\n");
}

#[test]
fn file_choices_map_the_constructor_and_its_payload() {
    let (executor, exports) = load(
        concat!(
            "pub type Maybe { Empty, Item(value: U8) }\n",
            "pub echo: Maybe -> Maybe = value => value",
        ),
        "choice",
    );
    let Ty::Choice(_, fields) = &exports[0].result else { panic!("choice") };
    assert_eq!(fields[1].ty, Ty::U8);
    let rig = reovim_rig::Rig::new("fipe-choice");
    let host = rig.host();
    let ctx = host.own();
    for (label, value, shown) in [
        ("Empty", Value::Unit, "Empty()\n"),
        ("Item", Value::U8(7), "Item(7)\n"),
    ] {
        let choice = Value::Choice(label.into(), Box::new(value));
        let value = Value::Record(alloc::vec![("value".into(), choice)]);
        let input =
            dynamic::encode(&exports[0].decl.methods[0].input, &value).unwrap();
        let output = executor.invoke(&ctx, &exports[0], &input, 1000).unwrap();
        assert_eq!(output, shown.as_bytes());
    }
}

#[test]
fn only_the_final_binding_can_export_a_name() {
    let (_, exports) = load(
        concat!(
            "pub greet: String -> String = name => name\n",
            "greet: String -> String = name => name",
        ),
        "private",
    );
    assert!(exports.is_empty());
}
