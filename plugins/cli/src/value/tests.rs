use super::*;
use core::cell::RefCell;
use fipe::ir::{Constructor, Meaning};
use reovim_kernel_interface::{
    Decl, DriverId, Flags, Kind, Op, Out, Outcome, Reply, Token,
};

#[derive(Default)]
struct Counter {
    open: RefCell<Vec<Handle>>,
}

impl Calls for Counter {
    fn register(
        &self,
        _: DriverId,
        _: Rd,
        _: Option<&str>,
    ) -> Result<Rd, Error> {
        Err(Error::Unsupported)
    }
    fn name(&self, _: Rd, _: Option<&str>) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn report(&self, _: Rd, _: Flags, _: &[Decl]) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn unregister(&self, _: Rd) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn changed(&self, _: Rd, _: Kind) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn done(&self, _: Token, _: Outcome) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn open(&self, rd: Rd, _: Rights) -> Result<Handle, Error> {
        let h = Handle(rd.0);
        self.open.borrow_mut().push(h);
        Ok(h)
    }
    fn submit(&self, op: Op) -> Reply<Out> {
        let Op::Close { h } = op else { return Reply::err(Error::Unsupported) };
        self.open.borrow_mut().retain(|open| *open != h);
        Reply::ok(Out::Unit)
    }
}

fn tables() -> Tables {
    let meaning = |namespace, label: &str, labels: &[&str]| Meaning {
        namespace,
        label: label.into(),
        parameters: Vec::new(),
        constructors: labels
            .iter()
            .map(|label| Constructor {
                label: (*label).into(),
                fields: Vec::new(),
            })
            .collect(),
    };
    Tables {
        namespaces: alloc::vec!["prelude".into(), "text.other".into()],
        meanings: alloc::vec![
            meaning(0, "Bool", &["True", "False"]),
            meaning(1, "Point", &["Point"]),
            meaning(1, "Maybe", &["Empty", "Item"]),
        ],
        ..Tables::default()
    }
}

#[test]
fn every_wire_type_round_trips_with_values_and_bounds() {
    let ctx = Counter::default();
    let tables = tables();
    let mut resources = Resources::new(&ctx);
    let mut mapping =
        Mapping { tables: &tables, home: "text", resources: &mut resources };
    let field = |name: &str, ty| reovim_kernel_interface::Field {
        name: name.into(),
        ty,
    };
    let cases = alloc::vec![
        (Ty::Unit, Wire::Unit),
        (Ty::Bool, Wire::Bool(true)),
        (Ty::Bool, Wire::Bool(false)),
        (Ty::U8, Wire::U8(255)),
        (Ty::U32, Wire::U32(u32::MAX)),
        (Ty::U64, Wire::U64(u64::MAX)),
        (Ty::I64, Wire::I64(i64::MIN)),
        (Ty::F64, Wire::F64(1.25)),
        (Ty::Utf8(8), Wire::Utf8("hello".into())),
        (Ty::Rest(8), Wire::Utf8("tail".into())),
        (Ty::Bytes(8), Wire::Bytes(alloc::vec![0, 255])),
        (
            Ty::Sequence(3, Box::new(Ty::U32)),
            Wire::Sequence(alloc::vec![Wire::U32(42)])
        ),
        (
            Ty::Array(2, Box::new(Ty::U8)),
            Wire::Sequence(alloc::vec![Wire::U8(1), Wire::U8(2)])
        ),
        (
            Ty::Record("other.Point".into(), alloc::vec![field("x", Ty::U32)]),
            Wire::Record(alloc::vec![("x".into(), Wire::U32(9))])
        ),
        (
            Ty::Choice(
                "other.Maybe".into(),
                alloc::vec![field("Empty", Ty::Unit), field("Item", Ty::U8)]
            ),
            Wire::Choice("Item".into(), Box::new(Wire::U8(17)))
        ),
        (Ty::Rd, Wire::Rd(Rd(27))),
    ];
    for (ty, wire) in cases {
        let value = mapping.lift(&ty, wire.clone()).unwrap();
        assert_eq!(mapping.to_wire(&ty, &value).unwrap(), wire);
    }
    assert_eq!(mapping.to_wire(&Ty::U8, &Value::Int(256)), Err(Error::Invalid));
    assert_eq!(mapping.to_wire(&Ty::U32, &Value::Int(-1)), Err(Error::Invalid));
    assert_eq!(mapping.to_wire(&Ty::Rd, &Value::Int(9)), Err(Error::Invalid));
    assert_eq!(
        mapping.to_wire(&Ty::Utf8(2), &Value::text("long")),
        Err(Error::Invalid)
    );
    drop(resources);
    assert!(ctx.open.borrow().is_empty());
}

#[test]
fn resources_close_when_a_run_returns_an_error() {
    let ctx = Counter::default();
    let run = || -> Result<(), Error> {
        let mut resources = Resources::new(&ctx);
        resources.open(Rd(9))?;
        resources.open(Rd(10))?;
        Err(Error::Invalid)
    };
    assert_eq!(run(), Err(Error::Invalid));
    assert!(ctx.open.borrow().is_empty());
}
