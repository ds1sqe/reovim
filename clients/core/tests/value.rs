//! The value decoder and printer, on the history Driver's `Tree`.

use reovim_client::value::{Value, decode, show};
use reovim_client::{Fault, Rd, Ty};
use reovim_kernel_interface::Field;
use reovim_module_text::history;

fn field(name: &str, ty: Ty) -> Field {
    Field { name: name.to_string(), ty }
}

/// A `Tree` with one node.
fn tree() -> Vec<u8> {
    let mut out = 1u32.to_le_bytes().to_vec();
    out.extend_from_slice(&1u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(b"open");
    out.extend_from_slice(&1u64.to_le_bytes());
    out.extend_from_slice(&1u64.to_le_bytes());
    out
}

#[test]
fn a_tree_decodes_by_name_and_prints_on_one_line() {
    let ty = history::decl().method(0).unwrap().output.clone();
    let value = decode(&ty, &tree()).unwrap();
    assert_eq!(value.field("head").and_then(Value::number), Some(1));
    let nodes = value.field("nodes").unwrap();
    let Value::Sequence(nodes) = nodes else { panic!("{nodes:?}") };
    assert_eq!(nodes[0].field("label"), Some(&Value::Utf8("open".into())));
    assert_eq!(
        show(&ty, &tree()).unwrap(),
        b"{nodes=[{id=1 parent=0 label=open}] head=1 published=1}\n"
    );
    assert_eq!(decode(&ty, &tree()[..10]), Err(Fault::Wire));
    let mut long = tree();
    long.push(0);
    assert_eq!(decode(&ty, &long), Err(Fault::Wire));
}

#[test]
fn every_scalar_a_choice_and_an_array_print() {
    let choice =
        Ty::Choice(vec![field("none", Ty::Unit), field("some", Ty::Rd)]);
    let mut some = vec![1u8];
    some.extend_from_slice(&7u64.to_le_bytes());
    assert_eq!(show(&choice, &some).unwrap(), b"some(7)\n");
    assert_eq!(show(&choice, &[0]).unwrap(), b"none()\n");
    assert_eq!(decode(&choice, &[2]), Err(Fault::Wire));

    let pair = Ty::Array(2, Box::new(Ty::U8));
    assert_eq!(show(&pair, &[3, 4]).unwrap(), b"[3 4]\n");
    assert_eq!(show(&Ty::Bool, &[1]).unwrap(), b"true\n");
    assert_eq!(show(&Ty::I64, &(-2i64).to_le_bytes()).unwrap(), b"-2\n");
    assert_eq!(show(&Ty::F64, &1.5f64.to_le_bytes()).unwrap(), b"1.5\n");
    assert_eq!(decode(&Ty::Rd, &3u64.to_le_bytes()), Ok(Value::Rd(Rd(3))));
    let mut bytes = 2u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"hi");
    assert_eq!(show(&Ty::Bytes(9), &bytes).unwrap(), b"hi");
    assert_eq!(show(&Ty::Utf8(9), &bytes).unwrap(), b"hi\n");
    assert_eq!(show(&Ty::Unit, &[]).unwrap(), b"");
}
