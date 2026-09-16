use super::*;

struct Fixture {
    runtime: Runtime,
    library: Rc<Library>,
    caller: Caller,
    parent: AccessId,
    source: AccessId,
    id: ResourceId,
}
impl Fixture {
    fn new() -> Self {
        let (mut runtime, library, original, directory) = installed();
        let id = subjects(&mut runtime, &original, directory);
        let initial = open(&mut runtime, &original, id);
        let caller = runtime
            .bind(&[grant(directory, 3), grant(id, 0)])
            .unwrap()
            .caller();
        let parent = open(&mut runtime, &caller, directory);
        let source = open(&mut runtime, &caller, id);
        query(
            &mut runtime,
            &caller,
            Operation::SetAccess,
            parent,
            &3_u32.to_le_bytes(),
        )
        .unwrap();
        query(&mut runtime, &original, Operation::Close, initial, &[]).unwrap();
        Self { runtime, library, caller, parent, source, id }
    }
    fn call(&self, name: &[u8]) -> Call {
        let mut data = 0_u32.to_le_bytes().to_vec();
        data.extend_from_slice(
            &u32::try_from(name.len()).unwrap().to_le_bytes(),
        );
        data.extend_from_slice(name);
        Call {
            request: self.caller.reserve().unwrap(),
            operation: Operation::Link,
            subject: self.parent,
            command: 0,
            expected: None,
            input: Value::copy(&data, &[self.source]).unwrap(),
        }
    }
    fn link(&mut self, name: &[u8]) -> Result<Value, Error> {
        let call = self.call(name);
        self.caller.submit(call.clone()).unwrap();
        completed(&mut self.runtime, &self.caller, call.request)
    }
    fn rows(&mut self) -> Vec<(Vec<u8>, ResourceId)> {
        ids(&entries(&mut self.runtime, &self.caller, self.parent, 192)
            .unwrap())
    }
    fn close(mut self) {
        for _ in 0..16 {
            match self.runtime.close() {
                Ok(()) => return,
                Err(error) => assert_eq!(error, Error::BUSY),
            }
            self.runtime
                .advance(&mut Context::from_waker(Waker::noop()))
                .unwrap();
        }
        panic!("fixture cleanup did not drain")
    }
}
#[test]
fn link_retains_identity_without_copying_or_granting_operation_rights() {
    let mut f = Fixture::new();
    f.link(b"joined").unwrap();
    assert_eq!(
        f.rows(),
        vec![
            (b"disk".to_vec(), f.id),
            (b"alias".to_vec(), f.id),
            (b"joined".to_vec(), f.id),
        ]
    );
    assert_eq!(f.library.count(c"fixture_materializations"), 1);
    let info = query(&mut f.runtime, &f.caller, Operation::Info, f.source, &[])
        .unwrap();
    assert_eq!(u32::from_le_bytes(info.data[24..28].try_into().unwrap()), 0);
    assert_eq!(
        query(
            &mut f.runtime,
            &f.caller,
            Operation::SetAccess,
            f.source,
            &1_u32.to_le_bytes()
        ),
        Err(Error::DENIED)
    );
    f.close();
}
#[test]
fn link_rejects_occupied_names_and_stale_source_keys() {
    let mut f = Fixture::new();
    assert_eq!(f.link(b"disk"), Err(Error::CONFLICT));
    let replace: unsafe extern "C" fn() =
        unsafe { f.library.symbol(c"fixture_replace") };
    unsafe { replace() };
    assert_eq!(f.link(b"joined"), Err(Error::CONFLICT));
    assert_eq!(f.library.count(c"fixture_linked"), 0);
    f.close();
}
#[test]
fn link_requires_parent_write_and_one_valid_source_slot() {
    let mut f = Fixture::new();
    query(
        &mut f.runtime,
        &f.caller,
        Operation::SetAccess,
        f.parent,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    let denied = f.call(b"joined");
    assert_eq!(f.caller.submit(denied.clone()), Err(Error::DENIED));
    f.caller.release(denied.request).unwrap();
    query(
        &mut f.runtime,
        &f.caller,
        Operation::SetAccess,
        f.parent,
        &3_u32.to_le_bytes(),
    )
    .unwrap();
    let mut invalid = f.call(b"joined");
    invalid.input.data[..4].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(f.caller.submit(invalid.clone()), Err(Error::INVALID));
    f.caller.release(invalid.request).unwrap();
    assert_eq!(f.library.count(c"fixture_links"), 0);
    f.close();
}
#[test]
fn links_across_mounted_instances_reject_before_provider_entry() {
    let mut f = Fixture::new();
    let other = mounted(&mut f.runtime, &f.library, b"/other");
    let caller =
        f.runtime.bind(&[grant(other, 2), grant(f.id, 0)]).unwrap().caller();
    let parent = open(&mut f.runtime, &caller, other);
    let source = open(&mut f.runtime, &caller, f.id);
    query(
        &mut f.runtime,
        &caller,
        Operation::SetAccess,
        parent,
        &2_u32.to_le_bytes(),
    )
    .unwrap();
    let template = f.call(b"joined");
    f.caller.release(template.request).unwrap();
    let call = Call {
        request: caller.reserve().unwrap(),
        subject: parent,
        input: Value::copy(&template.input.data, &[source]).unwrap(),
        ..template
    };
    assert_eq!(caller.submit(call.clone()), Err(Error::UNSUPPORTED));
    caller.release(call.request).unwrap();
    assert_eq!(f.library.count(c"fixture_links"), 0);
    f.close();
}
#[test]
fn unknown_link_retains_source_close_and_recovery_does_not_republish() {
    let mut f = Fixture::new();
    let uncertain: unsafe extern "C" fn() =
        unsafe { f.library.symbol(c"fixture_link_unknown") };
    unsafe { uncertain() };
    let call = f.call(b"joined");
    f.caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    f.runtime.advance(cx).unwrap();
    f.runtime.advance(cx).unwrap();
    assert_unknown(f.caller.snapshot(call.request).unwrap());
    let closing = f.caller.reserve().unwrap();
    f.caller
        .submit(Call {
            request: closing,
            operation: Operation::Close,
            subject: f.source,
            command: 0,
            expected: None,
            input: Value::unit(),
        })
        .unwrap();
    let mut name = 6_u32.to_le_bytes().to_vec();
    name.extend_from_slice(b"joined");
    query(&mut f.runtime, &f.caller, Operation::Delete, f.parent, &name)
        .unwrap();
    assert_eq!(f.caller.snapshot(closing).unwrap(), Status::Pending);
    f.caller.submit(call.clone()).unwrap();
    f.caller.reconcile(call.request).unwrap();
    completed(&mut f.runtime, &f.caller, call.request).unwrap();
    completed(&mut f.runtime, &f.caller, closing).unwrap();
    assert_eq!(f.library.count(c"fixture_links"), 1);
    assert_eq!(f.library.count(c"fixture_linked"), 1);
    assert_eq!(
        f.rows(),
        vec![(b"disk".to_vec(), f.id), (b"alias".to_vec(), f.id)]
    );
    f.close();
}
#[test]
fn cancelled_link_never_enters_the_provider() {
    let mut f = Fixture::new();
    let call = f.call(b"joined");
    f.caller.submit(call.clone()).unwrap();
    assert_eq!(
        f.caller.cancel(call.request).unwrap(),
        reovim_kernel_interface::call::Cancellation::Accepted
    );
    assert_eq!(
        completed(&mut f.runtime, &f.caller, call.request),
        Err(Error::CANCELLED)
    );
    assert_eq!(f.library.count(c"fixture_links"), 0);
    f.close();
}
