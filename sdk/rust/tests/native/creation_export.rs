use super::*;
use reovim_kernel_interface::abi;

fn exported_factory(probe: &Rc<Probe>, reject: bool) -> abi::Provider {
    let provider = ResourceProvider::new(
        (probe.clone(), reject),
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration(true),
            begin: None,
            create: Some(|(probe, reject), _, _, _, staging| {
                *probe.creation.borrow_mut() = Some(staging.clone());
                if *reject {
                    return Err(Error::DENIED);
                }
                staging.resource(child(probe, 91)).map_err(
                    |(error, mut child)| {
                        child.stop().unwrap();
                        error
                    },
                )?;
                let token = staging.auxiliary(child(probe, 92)).ok().unwrap();
                assert_eq!(token, AccessId(0x44));
                Ok(completed(Outcome::Ok(Value::copy(
                    &17_u64.to_le_bytes(),
                    &[],
                )?)))
            }),
        }]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    reovim_sdk::ExportedProvider::new(provider, LIMITS).ok().unwrap().into_raw()
}

#[test]
fn retained_rust_creation_clones_fence_before_native_context_expires() {
    for mode in [0, 1, 2] {
        check_lifetime(mode);
    }
}

fn check_lifetime(mode: u32) {
    let library = Library::compile_source("creation_export.c");
    let create: unsafe extern "C" fn(*mut abi::Provider, u32) =
        unsafe { library.symbol(c"fixture_factory") };
    let probe = Rc::new(Probe {
        mode: Mode::Create,
        calls: Cell::new(0),
        stopped: Cell::new(false),
        destroyed: Cell::new(false),
        stop_ready: Cell::new(true),
        creation: RefCell::new(None),
    });
    let mut raw = exported_factory(&probe, mode == 1);
    unsafe { create(&raw mut raw, mode) };
    // C's Creation/context were stack borrows that no longer exist here.
    let staging = probe.creation.borrow().as_ref().unwrap().clone();
    let before = library.count(c"fixture_staging_calls");
    let (error, mut returned) =
        staging.resource(child(&probe, 93)).err().unwrap();
    assert_eq!(error, Error::CLOSED);
    returned.stop().unwrap();
    let (error, mut returned) =
        staging.auxiliary(child(&probe, 94)).err().unwrap();
    assert_eq!(error, Error::CLOSED);
    returned.stop().unwrap();
    assert_eq!(library.count(c"fixture_staging_calls"), before);
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
}
