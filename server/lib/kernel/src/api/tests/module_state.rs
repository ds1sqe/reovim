use super::*;

use crate::api::{context::ModuleContext, version::API_VERSION};

#[test]
fn test_module_state_default() {
    let state = ModuleState::default();
    assert_eq!(state, ModuleState::Loaded);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_state_display() {
    assert_eq!(format!("{}", ModuleState::Loaded), "loaded");
    assert_eq!(format!("{}", ModuleState::Initializing), "initializing");
    assert_eq!(format!("{}", ModuleState::Running), "running");
    assert_eq!(format!("{}", ModuleState::Unloading), "unloading");
    assert_eq!(format!("{}", ModuleState::Failed("error".into())), "failed: error");
}

#[test]
fn test_module_state_valid_transitions() {
    // Loaded -> Initializing
    assert!(ModuleState::Loaded.can_transition_to(&ModuleState::Initializing));

    // Initializing -> Running
    assert!(ModuleState::Initializing.can_transition_to(&ModuleState::Running));

    // Running -> Unloading
    assert!(ModuleState::Running.can_transition_to(&ModuleState::Unloading));

    // Any state -> Failed
    assert!(ModuleState::Loaded.can_transition_to(&ModuleState::Failed("err".into())));
    assert!(ModuleState::Initializing.can_transition_to(&ModuleState::Failed("err".into())));
    assert!(ModuleState::Running.can_transition_to(&ModuleState::Failed("err".into())));
    assert!(ModuleState::Unloading.can_transition_to(&ModuleState::Failed("err".into())));
}

#[test]
fn test_module_state_invalid_transitions() {
    // Can't skip states
    assert!(!ModuleState::Loaded.can_transition_to(&ModuleState::Running));
    assert!(!ModuleState::Loaded.can_transition_to(&ModuleState::Unloading));

    // Can't go backwards
    assert!(!ModuleState::Running.can_transition_to(&ModuleState::Initializing));
    assert!(!ModuleState::Running.can_transition_to(&ModuleState::Loaded));
    assert!(!ModuleState::Initializing.can_transition_to(&ModuleState::Loaded));
}

#[test]
fn test_module_state_edge_cases() {
    // Test Failed state transitions
    let failed = ModuleState::Failed("error".into());

    // Failed can only transition to Failed (another error)
    assert!(!failed.can_transition_to(&ModuleState::Loaded));
    assert!(!failed.can_transition_to(&ModuleState::Initializing));
    assert!(!failed.can_transition_to(&ModuleState::Running));
    assert!(!failed.can_transition_to(&ModuleState::Unloading));
    assert!(failed.can_transition_to(&ModuleState::Failed("new error".into())));

    // Test Unloading state transitions
    let unloading = ModuleState::Unloading;

    // Unloading should only allow transition to Failed
    assert!(!unloading.can_transition_to(&ModuleState::Loaded));
    assert!(!unloading.can_transition_to(&ModuleState::Initializing));
    assert!(!unloading.can_transition_to(&ModuleState::Running));
    assert!(unloading.can_transition_to(&ModuleState::Failed("error".into())));
}

#[test]
fn test_module_info_fields() {
    let info = ModuleInfo {
        id: ModuleId::new("lang-rust"),
        name: "Rust Language Support",
        version: Version::new(1, 2, 3),
        api_version: API_VERSION,
        description: "Rust syntax and LSP",
        authors: &["Author 1", "Author 2"],
        license: "MIT",
    };

    assert_eq!(info.id.as_str(), "lang-rust");
    assert_eq!(info.name, "Rust Language Support");
    assert_eq!(info.version.major, 1);
    assert_eq!(info.version.minor, 2);
    assert_eq!(info.version.patch, 3);
    assert_eq!(info.description, "Rust syntax and LSP");
    assert_eq!(info.authors.len(), 2);
    assert_eq!(info.license, "MIT");
}

#[test]
fn test_module_info_from_module() {
    // Create a test module implementation
    struct TestModule;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Module for TestModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("test-module")
        }
        fn name(&self) -> &'static str {
            "Test Module"
        }
        fn version(&self) -> Version {
            Version::new(2, 1, 0)
        }
        fn api_version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let module = TestModule;
    let info = ModuleInfo::from_module(&module);

    assert_eq!(info.id, ModuleId::new("test-module"));
    assert_eq!(info.name, "Test Module");
    assert_eq!(info.version, Version::new(2, 1, 0));
    assert_eq!(info.api_version, Version::new(1, 0, 0));
}
