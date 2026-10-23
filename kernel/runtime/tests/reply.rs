//! Ready and deferred calls obey the same output contract.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{Rig, written};
use reovim_kernel_interface::{
    Calls, Decl, Driver, DriverId, Error, Flags, Hash, Method, Module, Op, Out,
    Rd, Reply, Rights, Token, Ty,
};
use reovim_kernel_runtime::{Limits, SYS};

#[derive(Default)]
struct Answer {
    rd: Cell<Option<Rd>>,
    later: Cell<bool>,
    valid: Cell<bool>,
}

fn decl() -> Decl {
    Decl {
        api: "answer".into(),
        ver: 1,
        methods: vec![Method {
            number: 0,
            name: "get".into(),
            input_name: None,
            input: Ty::Unit,
            output: Ty::U64,
            right: Rights::READ,
        }],
        records: vec![],
    }
}

impl Module for Answer {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let rd = ctx.register(DriverId(0), home, Some("value"))?;
        ctx.report(rd, Flags::READ, &[decl()])?;
        self.rd.set(Some(rd));
        Ok(())
    }
}

impl Driver for Answer {
    fn open(&self, _: &dyn Calls, _: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(0)
    }

    fn call(
        &self,
        _: &dyn Calls,
        _: u64,
        _: Hash,
        _: u16,
        _: &[u8],
    ) -> Reply<Vec<u8>> {
        if self.later.get() {
            return Reply::Pending(Token(1));
        }
        Reply::ok(if self.valid.get() {
            42_u64.to_le_bytes().to_vec()
        } else {
            vec![1, 2, 3]
        })
    }
}

fn fresh() -> (Rig, Rc<Answer>) {
    let driver = Rc::new(Answer::default());
    let rig = Rig::over(
        || 1,
        Limits::default(),
        "answer",
        driver.clone(),
        vec![driver.clone()],
    );
    (rig, driver)
}

#[test]
fn ready_output_is_checked_and_a_misfit_is_logged() {
    let (rig, driver) = fresh();
    let level = rig.lookup(SYS, "loglevel");
    let log = rig.open(level, Rights::WRITE);
    rig.ok(Op::Write { h: log, at: 0, bytes: b"debug".to_vec() });
    let h = rig.open(driver.rd.get().unwrap(), Rights::READ);
    let call = || Op::Call { h, api: decl().hash(), method: 0, input: vec![] };
    assert_eq!(rig.err(call()), Error::Invalid);
    let expected = format!(
        "call api={:016x} method=0 invalid reply len=3",
        decl().hash().0
    );
    assert!(
        written(&rig.kernel)
            .iter()
            .any(|(_, level, text)| { *level == 7 && text == &expected })
    );
    driver.valid.set(true);
    assert_eq!(rig.ok(call()), Out::Bytes(42_u64.to_le_bytes().to_vec()));
}

#[test]
fn deferred_output_is_checked_when_the_driver_finishes() {
    let (rig, driver) = fresh();
    driver.later.set(true);
    let h = rig.open(driver.rd.get().unwrap(), Rights::READ);
    let call = || Op::Call { h, api: decl().hash(), method: 0, input: vec![] };
    let token = rig.waits(call());
    rig.calls().done(Token(1), Ok(Out::Bytes(vec![1, 2, 3]))).unwrap();
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid);
    let token = rig.waits(call());
    let answer = Out::Bytes(42_u64.to_le_bytes().to_vec());
    rig.calls().done(Token(1), Ok(answer.clone())).unwrap();
    assert_eq!(rig.ok(Op::Ask { token }), answer);
    let token = rig.waits(call());
    rig.calls().done(Token(1), Ok(Out::Unit)).unwrap();
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid);
    let token = rig.waits(call());
    rig.calls().done(Token(1), Err(Error::Gone)).unwrap();
    assert_eq!(rig.err(Op::Ask { token }), Error::Gone);
}

fn read(rig: &Rig, rd: Rd) -> Vec<u8> {
    let h = rig.open(rd, Rights::READ);
    let Out::Bytes(bytes) = rig.ok(Op::Read { h, at: 0, len: 65536 }) else {
        panic!("read declaration");
    };
    rig.ok(Op::Close { h });
    bytes
}

#[test]
fn declarations_are_published_by_name_and_version_without_conflicts() {
    let (rig, driver) = fresh();
    let apis = rig.lookup(SYS, "apis");
    let api = rig.lookup(apis, "answer");
    let version = rig.lookup(api, "1");
    assert_eq!(rig.ok(Op::Children { rd: api }), Out::Rds(vec![version]));
    let decls = rig.lookup(SYS, "decls");
    let hash = rig.lookup(decls, &format!("{:016x}", decl().hash().0));
    assert_eq!(read(&rig, version), decl().canonical());
    assert_eq!(read(&rig, version), read(&rig, hash));
    let rd = driver.rd.get().unwrap();
    rig.calls().report(rd, Flags::READ, &[decl()]).unwrap();
    let mut changed = decl();
    changed.methods[0].output = Ty::Bool;
    let mut other = decl();
    other.api = "other".into();
    let before = rig.ok(Op::Children { rd: decls });
    let cursor = rig.since(0).1;
    let report = rig.calls().report(rd, Flags::WRITE, &[other, changed]);
    assert_eq!(report, Err(Error::Invalid));
    assert_eq!(rig.ok(Op::Children { rd: decls }), before);
    assert_eq!(read(&rig, version), decl().canonical());
    let Out::Info(info) = rig.ok(Op::Info { rd }) else { panic!("info") };
    assert_eq!(info.flags, Flags::READ);
    assert_eq!(info.apis, vec![decl().hash()]);
    assert_eq!(rig.since(cursor).0, Vec::new());
    assert_eq!(
        rig.err(Op::Lookup { rd: apis, name: "other".into() }),
        Error::Invalid
    );
}

#[test]
fn a_report_cannot_publish_two_declarations_for_one_name_and_version() {
    let (rig, driver) = fresh();
    let rd = driver.rd.get().unwrap();
    let mut first = decl();
    first.api = "new".into();
    let mut second = first.clone();
    second.methods[0].output = Ty::Bool;
    let report = rig.calls().report(rd, Flags::READ, &[first, second]);
    assert_eq!(report, Err(Error::Invalid));
    let apis = rig.lookup(SYS, "apis");
    assert_eq!(
        rig.err(Op::Lookup { rd: apis, name: "new".into() }),
        Error::Invalid
    );
}

#[test]
fn a_wait_keeps_its_output_contract_when_the_resource_reports_again() {
    let (rig, driver) = fresh();
    driver.later.set(true);
    let rd = driver.rd.get().unwrap();
    let h = rig.open(rd, Rights::READ);
    let call = Op::Call { h, api: decl().hash(), method: 0, input: vec![] };
    let token = rig.waits(call);
    let mut next = decl();
    next.ver = 2;
    next.methods[0].output = Ty::Bool;
    rig.calls().report(rd, Flags::READ, &[next.clone()]).unwrap();
    let answer = Out::Bytes(42_u64.to_le_bytes().to_vec());
    rig.calls().done(Token(1), Ok(answer.clone())).unwrap();
    assert_eq!(rig.ok(Op::Ask { token }), answer);
    let apis = rig.lookup(SYS, "apis");
    let api = rig.lookup(apis, "answer");
    let version = rig.lookup(api, "2");
    assert_eq!(read(&rig, version), next.canonical());
}
