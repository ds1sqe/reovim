use reovim_host_reference::DeterministicClock;
use reovim_kernel_interface::clock::{
    ClockDescriptor, ClockError, InstantOrdering, MonotonicClock,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::ContractSelection;

#[test]
fn deterministic_time_is_monotonic_and_close_preserves_past_instants() {
    let selected = descriptor(1, 9);
    let clock = DeterministicClock::new(selected);
    assert_eq!(clock.descriptor(), selected);
    let initial = clock.observe().unwrap();
    clock.advance(4).unwrap();
    let after = clock.observe().unwrap();
    assert_eq!(after.duration_since(initial).unwrap().nanoseconds(), 4);
    assert_eq!(after.compare(initial), Ok(InstantOrdering::After));
    assert!(matches!(
        clock.advance(6),
        Err(ClockError::DescriptorExhausted { .. })
    ));
    assert_eq!(
        clock.observe().unwrap().compare(after),
        Ok(InstantOrdering::Equal)
    );
    clock.close();
    clock.close();
    assert!(matches!(clock.observe(), Err(ClockError::Closed)));
    assert_eq!(clock.advance(1), Err(ClockError::Closed));
    assert_eq!(after.duration_since(initial).unwrap().nanoseconds(), 4);
}

#[test]
fn clock_generation_and_arithmetic_failures_do_not_change_time() {
    let clock = DeterministicClock::new(descriptor(1, u64::MAX));
    clock.advance(u64::MAX).unwrap();
    let last = clock.observe().unwrap();
    assert_eq!(
        clock.advance(1),
        Err(ClockError::DeterministicAdvancementOverflow)
    );
    assert_eq!(
        clock.observe().unwrap().compare(last),
        Ok(InstantOrdering::Equal)
    );
    assert_eq!(
        descriptor(2, u64::MAX).validate_stamp(last),
        Err(ClockError::StaleGeneration)
    );
    let other = ClockDescriptor::new(
        clock.descriptor().contract(),
        lifecycle(99),
        lifecycle(98),
        1,
        u64::MAX,
        100,
    )
    .unwrap();
    assert_eq!(other.validate_stamp(last), Err(ClockError::CrossClock));
}

#[cfg(feature = "std")]
#[test]
fn standard_clock_uses_the_same_stamps_and_close_contract() {
    let selected = descriptor(1, u64::MAX);
    let clock = reovim_host_reference::StdClock::new(selected);
    assert_eq!(clock.descriptor(), selected);
    let first = clock.observe().unwrap();
    let second = clock.observe().unwrap();
    selected.validate_stamp(first).unwrap();
    selected.validate_stamp(second).unwrap();
    second.duration_since(first).unwrap();
    clock.close();
    clock.close();
    assert!(matches!(clock.observe(), Err(ClockError::Closed)));
    second.duration_since(first).unwrap();
}

fn descriptor(generation: u64, maximum: u64) -> ClockDescriptor {
    ClockDescriptor::new(
        ContractSelection::new(1, 1, 1, 1).unwrap(),
        LifecycleIdentity::new(2, 3, Generation::new(generation)),
        lifecycle(4),
        1,
        maximum,
        5,
    )
    .unwrap()
}

const fn lifecycle(identity: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(identity, 1, Generation::new(1))
}
