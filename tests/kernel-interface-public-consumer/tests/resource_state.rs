//! Native preparation witnesses, not an authoritative editing engine.

#[path = "resource_state/faults.rs"]
mod faults;
#[path = "resource_state/history_faults.rs"]
mod history_faults;
#[path = "resource_state/journey.rs"]
mod journey;
#[path = "resource_state/provider.rs"]
mod provider;
#[path = "resource_state/retirement.rs"]
mod retirement;
use reovim_kernel_interface_public_consumer::first_edit::native::{
    models, support,
};
