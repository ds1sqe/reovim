use reovim_kernel_interface::abi;
use reovim_module::{Connection, Contract, Message, ResourceId, rv_module};

struct State;
impl State {
    fn new(_: Connection) -> Self {
        panic!("passive metadata must not construct state")
    }

    #[allow(clippy::unused_self)]
    fn read(
        &self,
        _: ResourceId,
        input: Message,
    ) -> core::future::Ready<Result<Message, reovim_module::Error>> {
        core::future::ready(Ok(input))
    }
}
const BYTES: Contract = Contract::new("resource.bytes", 1);
rv_module! {
    pub MODULE {
        contract: ("example.feature", 1),
        requires: [("example.storage", 1), ("example.index", 2)],
        state: State::new,
        resources: { "value" => { read BYTES => State::read } },
    }
}

#[test]
#[allow(unsafe_code)]
fn contract_metadata_is_passive_and_has_the_declared_c_layout() {
    let table = MODULE.table();
    assert_eq!(table.version, abi::VERSION);
    assert_eq!(table.size, abi::Module::ABI_SIZE);
    // These views are generated exclusively from the static declaration above.
    let identity = unsafe { name(table.identity) };
    assert_eq!(identity, "example.feature");
    assert_eq!(table.identity.version, 1);
    let dependencies = unsafe {
        std::slice::from_raw_parts(table.dependencies, table.dependency_count)
    };
    let contracts: Vec<_> = dependencies
        .iter()
        .map(|value| (unsafe { name(*value) }, value.version))
        .collect();
    assert_eq!(contracts, [("example.storage", 1), ("example.index", 2)]);
}

#[allow(unsafe_code)]
unsafe fn name(value: abi::Contract) -> &'static str {
    let bytes =
        unsafe { std::slice::from_raw_parts(value.name.data, value.name.len) };
    std::str::from_utf8(bytes).unwrap()
}
