use super::*;
use reovim_kernel_interface::{
    driver::{DriverOps, Lifecycle, Progress},
    module::{ModuleImplementation, ModuleScope},
    provider::Registrar,
};
use reovim_kernel_runtime::{Activation, PublicationGrant};

type State = (Lifecycle, Registrar, Option<ResourceProvider>);
fn publish(state: &mut State, _: &mut Context<'_>) -> Result<Progress, Error> {
    if state.0 == Lifecycle::Constructed {
        state.0 = Lifecycle::Starting;
        return Ok(Progress::More);
    }
    if state.0 == Lifecycle::Starting {
        state
            .1
            .stage(b"directory", state.2.take().unwrap())
            .map_err(|(error, _)| error)?;
        state.0 = Lifecycle::Ready;
    }
    Ok(Progress::Idle)
}
#[allow(clippy::result_large_err, reason = "inert rejection returns imports")]
fn installed() -> (Runtime, Rc<Library>, Caller, ResourceId) {
    installed_source("namespace.c")
}
#[allow(clippy::result_large_err, reason = "inert rejection returns imports")]
fn installed_source(
    source: &str,
) -> (Runtime, Rc<Library>, Caller, ResourceId) {
    let mut runtime =
        Runtime::new([0x84; 16], Limits { handles: 3, ..Limits::default() })
            .unwrap();
    let mount = runtime.register_namespace(b"/mount").unwrap();
    let binding = runtime.bind(&[]).unwrap();
    runtime
        .grant_publication(
            &binding,
            PublicationGrant {
                namespace: mount,
                scope: ModuleScope::Global,
                access: 1,
                create_access: 0,
                retain_loans: false,
            },
        )
        .unwrap();
    runtime.start().unwrap();
    let library = Library::compile_source(source);
    let raw = directory(&library);
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let instance = runtime
        .activate(
            Activation {
                namespace: mount,
                scope: ModuleScope::Global,
                required: &[b"directory"],
                exports: 1,
            },
            ModuleImplementation {
                construct: |context| {
                    Ok((
                        Lifecycle::Constructed,
                        context.exports,
                        Some(context.imports),
                    ))
                },
                driver: DriverOps {
                    status: |state: &State| state.0,
                    advance: publish,
                    close: |state| {
                        state.0 = Lifecycle::Closed;
                        Ok(())
                    },
                },
            },
            provider,
            &binding,
            Rc::new(()),
        )
        .unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let directory = runtime.module_exports(instance).unwrap()[0];
    (runtime, library, binding.caller(), directory)
}

#[path = "opened.rs"]
mod opened;
fn subjects(
    runtime: &mut Runtime,
    caller: &Caller,
    directory: ResourceId,
) -> ResourceId {
    let access = open(runtime, caller, directory);
    query(runtime, caller, Operation::SetAccess, access, &1_u32.to_le_bytes())
        .unwrap();
    let rows = ids(&entries(runtime, caller, access, 96).unwrap());
    assert_eq!(rows[0].1, rows[1].1);
    rows[0].1
}
fn acquisition(caller: &Caller, id: ResourceId) -> Call {
    let mut input = 1_u32.to_le_bytes().to_vec();
    input.extend_from_slice(&id.epoch);
    input.extend_from_slice(&id.serial.to_le_bytes());
    Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Acquire,
        subject: AccessId(0),
        command: 0,
        expected: None,
        input: Value::copy(&input, &[]).unwrap(),
    }
}
#[test]
fn concurrent_alias_acquisition_materializes_once_with_separate_access() {
    let (mut runtime, library, caller, directory) = installed();
    let id = subjects(&mut runtime, &caller, directory);
    let first = acquisition(&caller, id);
    let second = acquisition(&caller, id);
    caller.submit(first.clone()).unwrap();
    caller.submit(second.clone()).unwrap();
    assert_eq!(library.count(c"fixture_materializations"), 0);
    let a =
        completed(&mut runtime, &caller, first.request).unwrap().references[0];
    let b =
        completed(&mut runtime, &caller, second.request).unwrap().references[0];
    assert_ne!(a, b);
    assert_eq!(library.count(c"fixture_materializations"), 1);
    let info_a = query(&mut runtime, &caller, Operation::Info, a, &[]).unwrap();
    assert_eq!(&info_a.data[..16], &id.epoch);
    assert_eq!(
        u64::from_le_bytes(info_a.data[16..24].try_into().unwrap()),
        id.serial
    );
    assert_eq!(u32::from_le_bytes(info_a.data[24..28].try_into().unwrap()), 0);
    query(&mut runtime, &caller, Operation::SetAccess, a, &1_u32.to_le_bytes())
        .unwrap();
    let info_b = query(&mut runtime, &caller, Operation::Info, b, &[]).unwrap();
    assert_eq!(u32::from_le_bytes(info_b.data[24..28].try_into().unwrap()), 0);
    query(&mut runtime, &caller, Operation::Close, a, &[]).unwrap();
    query(&mut runtime, &caller, Operation::Info, b, &[]).unwrap();
    assert_eq!(read(&mut runtime, &caller, b), 41);
    query(&mut runtime, &caller, Operation::Close, b, &[]).unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}
#[test]
fn namespace_read_grant_does_not_authorize_materializing_its_children() {
    let (mut runtime, library, caller, directory) = installed();
    let id = subjects(&mut runtime, &caller, directory);
    let other = runtime.bind(&[grant(directory, 1)]).unwrap().caller();
    let request = acquisition(&other, id);
    assert_eq!(other.submit(request.clone()), Err(Error::DENIED));
    other.release(request.request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(library.count(c"fixture_materializations"), 0);
    runtime.close().unwrap();
}

fn read(runtime: &mut Runtime, caller: &Caller, access: AccessId) -> u64 {
    query(runtime, caller, Operation::SetAccess, access, &1_u32.to_le_bytes())
        .unwrap();
    let mut page = 0_u64.to_le_bytes().to_vec();
    page.extend_from_slice(&4096_u64.to_le_bytes());
    let offered =
        query(runtime, caller, Operation::Interfaces, access, &page).unwrap();
    let length =
        u32::from_le_bytes(offered.data[4..8].try_into().unwrap()) as usize;
    let api = reovim_kernel_interface::declaration::Declaration::decode(
        &offered.data[8..8 + length],
    )
    .unwrap();
    let mut input =
        u32::try_from(api.bytes().len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(api.bytes());
    let bound = query(runtime, caller, Operation::Bind, access, &input)
        .unwrap()
        .references[0];
    let command = api
        .methods()
        .iter()
        .find_map(|method| match method.operation() {
            reovim_kernel_interface::declaration::Operation::Ioctl(command) => {
                Some(command)
            }
            reovim_kernel_interface::declaration::Operation::Exec => None,
        })
        .unwrap();
    let request = caller.reserve().unwrap();
    caller
        .submit(Call {
            request,
            operation: Operation::Ioctl,
            subject: bound,
            command,
            expected: None,
            input: Value::unit(),
        })
        .unwrap();
    number(&completed(runtime, caller, request).unwrap())
}
#[test]
fn cancelling_one_joiner_releases_its_handle_reservation() {
    let (mut runtime, library, caller, directory) = installed();
    let id = subjects(&mut runtime, &caller, directory);
    let first = acquisition(&caller, id);
    let second = acquisition(&caller, id);
    caller.submit(first.clone()).unwrap();
    caller.submit(second.clone()).unwrap();
    assert_eq!(
        caller.cancel(first.request).unwrap(),
        reovim_kernel_interface::call::Cancellation::Accepted
    );
    assert_eq!(
        completed(&mut runtime, &caller, first.request),
        Err(Error::CANCELLED)
    );
    let b =
        completed(&mut runtime, &caller, second.request).unwrap().references[0];
    let c = open(&mut runtime, &caller, id);
    assert_ne!(b, c);
    assert_eq!(library.count(c"fixture_materializations"), 1);
    query(&mut runtime, &caller, Operation::Close, b, &[]).unwrap();
    query(&mut runtime, &caller, Operation::Close, c, &[]).unwrap();
    runtime.close().unwrap();
}
#[test]
fn uncertain_materialization_reconciles_one_shared_native_attempt() {
    let (mut runtime, library, caller, directory) = installed();
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_materialize_unknown") };
    unsafe { uncertain() };
    let id = subjects(&mut runtime, &caller, directory);
    let first = acquisition(&caller, id);
    let second = acquisition(&caller, id);
    caller.submit(first.clone()).unwrap();
    caller.submit(second.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_unknown(caller.snapshot(first.request).unwrap());
    assert_unknown(caller.snapshot(second.request).unwrap());
    caller.reconcile(first.request).unwrap();
    caller.reconcile(second.request).unwrap();
    let a =
        completed(&mut runtime, &caller, first.request).unwrap().references[0];
    let b =
        completed(&mut runtime, &caller, second.request).unwrap().references[0];
    assert_eq!(library.count(c"fixture_materializations"), 1);
    query(&mut runtime, &caller, Operation::Close, a, &[]).unwrap();
    assert_eq!(read(&mut runtime, &caller, b), 41);
    query(&mut runtime, &caller, Operation::Close, b, &[]).unwrap();
    runtime.close().unwrap();
}

#[test]
fn replaced_key_fails_without_retargeting_the_old_identity() {
    let (mut runtime, library, caller, directory) = installed();
    let old = subjects(&mut runtime, &caller, directory);
    let replace: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_replace") };
    unsafe { replace() };
    let request = acquisition(&caller, old);
    caller.submit(request.clone()).unwrap();
    assert_eq!(
        completed(&mut runtime, &caller, request.request),
        Err(Error::CONFLICT)
    );
    let new = subjects(&mut runtime, &caller, directory);
    assert_ne!(old, new);
    let opened = open(&mut runtime, &caller, new);
    let info =
        query(&mut runtime, &caller, Operation::Info, opened, &[]).unwrap();
    assert_eq!(
        u64::from_le_bytes(info.data[16..24].try_into().unwrap()),
        new.serial
    );
    assert_eq!(library.count(c"fixture_materializations"), 2);
    query(&mut runtime, &caller, Operation::Close, opened, &[]).unwrap();
    runtime.close().unwrap();
}

fn assert_unknown(status: Status) {
    let Status::Outcome(outcome) = status else { panic!("not settled") };
    assert!(matches!(outcome.as_ref(), Outcome::Unknown { .. }));
}

#[path = "traversal.rs"]
mod traversal;

#[path = "removal.rs"]
mod removal;

#[path = "link.rs"]
mod link;

#[path = "rename.rs"]
mod rename;

#[path = "create.rs"]
mod create;
