use {super::*, reovim_kernel::api::v1::ModuleId};

// Test mode implementation
struct TestMode {
    id: ModeId,
    is_entry: bool,
    enter_fails: bool,
}

impl TestMode {
    fn new(name: &'static str) -> Self {
        Self {
            id: ModeId::new(ModuleId::new("test"), name),
            is_entry: false,
            enter_fails: false,
        }
    }

    fn with_entry(mut self) -> Self {
        self.is_entry = true;
        self
    }

    fn fails_on_enter(mut self) -> Self {
        self.enter_fails = true;
        self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SessionMode for TestMode {
    fn id(&self) -> ModeId {
        self.id.clone()
    }

    fn is_entry(&self) -> bool {
        self.is_entry
    }

    fn on_enter(&self) -> Result<(), ModeError> {
        if self.enter_fails {
            Err(ModeError::new("test failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn test_mode_error_display() {
    let err = ModeError::new("something went wrong");
    assert_eq!(err.to_string(), "mode error: something went wrong");
}

#[test]
fn test_session_mode_default_is_entry() {
    let mode = TestMode::new("normal");
    assert!(!mode.is_entry());
}

#[test]
fn test_session_mode_is_entry() {
    let mode = TestMode::new("normal").with_entry();
    assert!(mode.is_entry());
}

#[test]
fn test_session_mode_on_enter_success() {
    let mode = TestMode::new("normal");
    assert!(mode.on_enter().is_ok());
}

#[test]
fn test_session_mode_on_enter_failure() {
    let mode = TestMode::new("normal").fails_on_enter();
    let result = mode.on_enter();
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("test failure"));
}

#[test]
fn test_session_mode_on_exit_default() {
    let mode = TestMode::new("normal");
    mode.on_exit(); // Should not panic
}

#[test]
fn trait_is_object_safe() {
    let mode = TestMode::new("normal");
    let _: Box<dyn SessionMode> = Box::new(mode);
}

#[test]
fn test_mode_error_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(ModeError::new("test error"));
    assert_eq!(err.to_string(), "mode error: test error");
}

#[test]
fn test_mode_error_clone() {
    let err = ModeError::new("test");
    let cloned = err.clone();
    assert_eq!(err.message, cloned.message);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_error_debug() {
    let err = ModeError::new("debug test");
    let debug = format!("{err:?}");
    assert!(debug.contains("debug test"));
}

#[test]
fn test_mode_error_empty_message() {
    let err = ModeError::new("");
    assert_eq!(err.to_string(), "mode error: ");
}

#[test]
fn test_mode_error_from_string() {
    let err = ModeError::new(String::from("owned string"));
    assert_eq!(err.message, "owned string");
}

#[test]
fn test_session_mode_id() {
    let mode = TestMode::new("insert");
    let id = mode.id();
    assert_eq!(id.name(), "insert");
}

#[test]
fn test_session_mode_default_on_exit() {
    // Default on_exit should not panic
    struct MinimalMode;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SessionMode for MinimalMode {
        fn id(&self) -> ModeId {
            ModeId::new(ModuleId::new("test"), "minimal")
        }
    }

    let mode = MinimalMode;
    mode.on_exit(); // Default impl
    assert!(mode.on_enter().is_ok()); // Default impl
    assert!(!mode.is_entry()); // Default impl
}

#[test]
fn test_session_mode_with_entry_and_fails() {
    let mode = TestMode::new("special").with_entry().fails_on_enter();
    assert!(mode.is_entry());
    assert!(mode.on_enter().is_err());
}

#[test]
fn test_dyn_session_mode_methods() {
    let mode: Box<dyn SessionMode> = Box::new(TestMode::new("normal").with_entry());
    assert_eq!(mode.id().name(), "normal");
    assert!(mode.is_entry());
    assert!(mode.on_enter().is_ok());
    mode.on_exit();
}
