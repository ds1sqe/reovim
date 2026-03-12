use super::*;

#[test]
fn drain_impl_implements_trait() {
    let drain = NotificationDrainImpl;
    let _: &dyn NotificationDrain = &drain;
}
