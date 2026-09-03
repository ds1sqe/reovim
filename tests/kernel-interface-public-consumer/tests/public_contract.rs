#![forbid(unsafe_code)]
//! Executed public-boundary journey for Kernel Interface contracts.

#[test]
fn complete_public_contract_journey() {
    assert!(reovim_kernel_interface_public_consumer::exercise_public_api());
}
