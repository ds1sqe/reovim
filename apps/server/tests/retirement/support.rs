use std::rc::Rc;

use reovim_host_reference::DeterministicClock;
use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootLifecycleRequest,
};
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::{self, HeadingService, life};

pub fn constructed() -> HeadingService {
    let owner = life(40);
    let clock = ClockDescriptor::new(
        profile::schema(80),
        life(41),
        owner,
        1,
        u64::MAX,
        81,
    )
    .unwrap();
    let telemetry = Rc::new(
        Telemetry::new(
            owner,
            life(42),
            Rc::new(DeterministicClock::new(clock)),
            None,
            64,
        )
        .unwrap(),
    );
    assembly::heading(owner, b"# a\n\xff".to_vec(), telemetry).unwrap()
}

pub fn opened() -> HeadingService {
    let mut service = constructed();
    service.open(0).unwrap();
    service.open(1).unwrap();
    service
}

pub fn budget(service: &HeadingService) -> RootLifecycleRequest {
    RootLifecycleRequest {
        root: service.runtime.owner(),
        work: service.runtime.limits().work,
        deadline: DeadlineState::Pending,
    }
}
