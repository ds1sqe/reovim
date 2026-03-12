use super::*;

#[test]
fn test_display_driver_is_object_safe() {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _accepts_ref(_: &dyn DisplayDriver) {}
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _accepts_box(_: Box<dyn DisplayDriver>) {}
}

#[test]
fn test_window_manager_is_object_safe() {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _accepts_ref(_: &dyn WindowManager) {}
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _accepts_box(_: Box<dyn WindowManager>) {}
}
