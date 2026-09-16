use super::*;

#[test]
fn native_interfaces_bind_distinct_contexts_to_the_same_callback() {
    let library = Library::compile();
    let create: unsafe extern "C" fn(*mut abi::Provider) -> u32 =
        unsafe { library.symbol(c"fixture_pair") };
    let mut raw = core::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { create(raw.as_mut_ptr()) }, 0);
    let raw = unsafe { raw.assume_init() };
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    assert_ne!(provider.interfaces()[0].api(), provider.interfaces()[1].api());
    let cx = &mut Context::from_waker(Waker::noop());
    for (interface, expected) in [(0, 41), (1, 83)] {
        let mut pending = provider
            .begin(interface, Operation::Ioctl(1), &Value::unit())
            .unwrap();
        assert_eq!(pending.poll(cx), Ok(Poll::Ready(())));
        let completion = pending.take().unwrap();
        let Outcome::Ok(value) = &completion.outcome else {
            panic!("no value")
        };
        assert_eq!(number(value), expected);
        drop(completion);
        pending.finish().unwrap();
    }
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
