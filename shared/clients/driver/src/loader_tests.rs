use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::{
    ClientModule, ClientModuleError, ModuleContext, OptionValue, ProbeResult, Style, Version,
    loader::{ClientModuleFactory, ClientModuleLoader, ClientModuleLoaderError, ClientModuleState},
    traits::{PlatformCapabilities, ServerHandle, ThemeProvider},
    types::{ColorDepth, Insets, RenderingModel},
};

// =============================================================================
// Mock infrastructure
// =============================================================================

/// Shared call log for verifying call order across modules.
type CallLog = Arc<Mutex<Vec<String>>>;

/// Configurable mock module for loader tests (stateful, uses `from_modules_for_test`).
struct MockClientModule {
    kind: &'static str,
    deps: &'static [&'static str],
    optional_deps: &'static [&'static str],
    exit_ok: bool,
    log: CallLog,
}

impl MockClientModule {
    fn new(kind: &'static str, log: CallLog) -> Self {
        Self {
            kind,
            deps: &[],
            optional_deps: &[],
            exit_ok: true,
            log,
        }
    }

    fn with_deps(mut self, deps: &'static [&'static str]) -> Self {
        self.deps = deps;
        self
    }

    fn with_exit_fail(mut self) -> Self {
        self.exit_ok = false;
        self
    }
}

impl ClientModule for MockClientModule {
    fn id(&self) -> &'static str {
        self.kind
    }
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn name(&self) -> &'static str {
        self.kind
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn dependencies(&self) -> &[&str] {
        self.deps
    }
    fn optional_dependencies(&self) -> &[&str] {
        self.optional_deps
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        self.log.lock().unwrap().push(format!("init:{}", self.kind));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        self.log.lock().unwrap().push(format!("exit:{}", self.kind));
        if self.exit_ok {
            Ok(())
        } else {
            Err(ClientModuleError {
                message: "exit failed".to_string(),
            })
        }
    }

    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        self.log
            .lock()
            .unwrap()
            .push(format!("on_all_loaded:{}", self.kind));
    }
}

// =============================================================================
// Simple helper module types (usable with fn() factories)
// =============================================================================

/// Minimal module that always succeeds init/exit.
struct SimpleModule(&'static str);

impl ClientModule for SimpleModule {
    fn id(&self) -> &'static str {
        self.0
    }
    fn kind(&self) -> &'static str {
        self.0
    }
    fn name(&self) -> &'static str {
        self.0
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }
}

/// Module with configurable dependencies.
struct DepModule {
    kind: &'static str,
    deps: &'static [&'static str],
    optional: &'static [&'static str],
}

impl ClientModule for DepModule {
    fn id(&self) -> &'static str {
        self.kind
    }
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn name(&self) -> &'static str {
        self.kind
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn dependencies(&self) -> &[&str] {
        self.deps
    }
    fn optional_dependencies(&self) -> &[&str] {
        self.optional
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }
}

/// Module that always fails init.
struct FailModule(&'static str);

impl ClientModule for FailModule {
    fn id(&self) -> &'static str {
        self.0
    }
    fn kind(&self) -> &'static str {
        self.0
    }
    fn name(&self) -> &'static str {
        self.0
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Failed(ClientModuleError {
            message: "always fails".to_string(),
        })
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }
}

/// Module that defers once then succeeds.
struct DeferOnceModule {
    kind: &'static str,
    init_count: usize,
}

impl ClientModule for DeferOnceModule {
    fn id(&self) -> &'static str {
        self.kind
    }
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn name(&self) -> &'static str {
        self.kind
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        self.init_count += 1;
        if self.init_count <= 1 {
            ProbeResult::Defer("not ready".to_string())
        } else {
            ProbeResult::Success
        }
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }
}

/// Module that always defers.
struct AlwaysDeferModule(&'static str);

impl ClientModule for AlwaysDeferModule {
    fn id(&self) -> &'static str {
        self.0
    }
    fn kind(&self) -> &'static str {
        self.0
    }
    fn name(&self) -> &'static str {
        self.0
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Defer("always defers".to_string())
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }
}

// =============================================================================
// Test helpers
// =============================================================================

struct MockCaps;
impl PlatformCapabilities for MockCaps {
    fn rendering_model(&self) -> RenderingModel {
        RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }
    fn color_depth(&self) -> ColorDepth {
        ColorDepth::TrueColor
    }
    fn pixel_size(&self) -> Option<(u32, u32)> {
        None
    }
    fn reliable_unicode_width(&self) -> bool {
        true
    }
    fn dark_mode(&self) -> bool {
        true
    }
    fn smooth_scroll(&self) -> bool {
        false
    }
    fn pointer_events(&self) -> bool {
        false
    }
    fn touch_input(&self) -> bool {
        false
    }
    fn haptic(&self) -> bool {
        false
    }
    fn safe_area(&self) -> Insets {
        Insets::ZERO
    }
    fn has_focus(&self) -> bool {
        true
    }
    fn clipboard_available(&self) -> bool {
        false
    }
    fn screen_reader_active(&self) -> bool {
        false
    }
}

struct MockServer;
impl ServerHandle for MockServer {
    fn get_options(&self, _names: &[&str]) -> Vec<(String, OptionValue)> {
        Vec::new()
    }
    fn execute_command(&self, _command: &str) {}
}

struct MockTheme;
impl ThemeProvider for MockTheme {
    fn highlight(&self, _group: &str) -> Style {
        Style::default()
    }
    fn highlight_with_fallback(&self, _groups: &[&str]) -> Style {
        Style::default()
    }
    fn foreground(&self) -> Style {
        Style::default()
    }
    fn background(&self) -> Style {
        Style::default()
    }
    fn is_dark(&self) -> bool {
        true
    }
}

fn make_ctx() -> ModuleContext<'static> {
    let caps: &'static dyn PlatformCapabilities = Box::leak(Box::new(MockCaps));
    let theme: &'static dyn ThemeProvider = Box::leak(Box::new(MockTheme));
    ModuleContext {
        capabilities: caps,
        server: Arc::new(MockServer),
        theme,
    }
}

// Factory functions for use with HashMap<&str, ClientModuleFactory>.
// Each creates a specific module type with a unique kind string.

// =============================================================================
// Tests
// =============================================================================

#[test]
fn loader_empty_map() {
    let factories: HashMap<&str, ClientModuleFactory> = HashMap::new();
    let disabled = HashSet::new();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    assert_eq!(loader.module_count(), 0);
    assert_eq!(loader.running_count(), 0);
    assert!(loader.as_module_slice().is_empty());
}

#[test]
fn loader_single_module() {
    fn factory() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("alpha"))
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("alpha", factory as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    assert_eq!(loader.module_count(), 1);
    assert_eq!(loader.as_module_slice()[0].kind(), "alpha");
}

#[test]
fn loader_no_deps_multiple() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("b"))
    }
    fn fc() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("c"))
    }
    let factories: HashMap<&str, ClientModuleFactory> = [
        ("a", fa as ClientModuleFactory),
        ("b", fb as ClientModuleFactory),
        ("c", fc as ClientModuleFactory),
    ]
    .into();
    let disabled = HashSet::new();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    assert_eq!(loader.module_count(), 3);
}

#[test]
fn loader_with_required_deps() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("dep_a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(DepModule {
            kind: "dep_b",
            deps: &["dep_a"],
            optional: &[],
        })
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("dep_a", fa as ClientModuleFactory), ("dep_b", fb as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let kinds: Vec<&str> = loader.as_module_slice().iter().map(|m| m.kind()).collect();
    let a_pos = kinds.iter().position(|k| *k == "dep_a").unwrap();
    let b_pos = kinds.iter().position(|k| *k == "dep_b").unwrap();
    assert!(a_pos < b_pos, "dep_a must come before dep_b");
}

#[test]
fn loader_with_optional_deps() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("opt_a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(DepModule {
            kind: "opt_b",
            deps: &[],
            optional: &["opt_a"],
        })
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("opt_a", fa as ClientModuleFactory), ("opt_b", fb as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let kinds: Vec<&str> = loader.as_module_slice().iter().map(|m| m.kind()).collect();
    let a_pos = kinds.iter().position(|k| *k == "opt_a").unwrap();
    let b_pos = kinds.iter().position(|k| *k == "opt_b").unwrap();
    assert!(a_pos < b_pos, "opt_a must come before opt_b when present");
}

#[test]
fn loader_cycle_detection() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(DepModule {
            kind: "cyc_a",
            deps: &["cyc_b"],
            optional: &[],
        })
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(DepModule {
            kind: "cyc_b",
            deps: &["cyc_a"],
            optional: &[],
        })
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("cyc_a", fa as ClientModuleFactory), ("cyc_b", fb as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let result = ClientModuleLoader::new(factories, &disabled);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ClientModuleLoaderError::DependencyResolution(_)));
}

#[test]
fn loader_missing_required_dep() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(DepModule {
            kind: "miss_a",
            deps: &["nonexistent"],
            optional: &[],
        })
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("miss_a", fa as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let result = ClientModuleLoader::new(factories, &disabled);
    assert!(result.is_err());
}

#[test]
fn loader_disabled_modules_excluded() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("dis_a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("dis_b"))
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("dis_a", fa as ClientModuleFactory), ("dis_b", fb as ClientModuleFactory)].into();
    let disabled: HashSet<String> = std::iter::once("dis_b".to_string()).collect();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    assert_eq!(loader.module_count(), 1);
    assert_eq!(loader.as_module_slice()[0].kind(), "dis_a");
}

#[test]
fn loader_disabled_dep_makes_optional_skip() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("dopt_a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(DepModule {
            kind: "dopt_b",
            deps: &[],
            optional: &["dopt_a"],
        })
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("dopt_a", fa as ClientModuleFactory), ("dopt_b", fb as ClientModuleFactory)].into();
    let disabled: HashSet<String> = std::iter::once("dopt_a".to_string()).collect();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    assert_eq!(loader.module_count(), 1);
    assert_eq!(loader.as_module_slice()[0].kind(), "dopt_b");
}

#[test]
fn loader_disabled_required_dep_error() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("dreq_a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(DepModule {
            kind: "dreq_b",
            deps: &["dreq_a"],
            optional: &[],
        })
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("dreq_a", fa as ClientModuleFactory), ("dreq_b", fb as ClientModuleFactory)].into();
    let disabled: HashSet<String> = std::iter::once("dreq_a".to_string()).collect();
    let result = ClientModuleLoader::new(factories, &disabled);
    assert!(result.is_err());
}

#[test]
fn loader_init_all_success() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("init_a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("init_b"))
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("init_a", fa as ClientModuleFactory), ("init_b", fb as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let ctx = make_ctx();
    let count = loader.init_all(&ctx);
    assert_eq!(count, 2);
    assert_eq!(loader.running_count(), 2);
}

#[test]
fn loader_init_failure_continues() {
    fn fg() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("ifail_good"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(FailModule("ifail_bad"))
    }
    let factories: HashMap<&str, ClientModuleFactory> = [
        ("ifail_good", fg as ClientModuleFactory),
        ("ifail_bad", fb as ClientModuleFactory),
    ]
    .into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let ctx = make_ctx();
    let count = loader.init_all(&ctx);
    assert_eq!(count, 1);
    assert_eq!(loader.running_count(), 1);
    assert!(matches!(
        loader.state("ifail_bad"),
        Some(ClientModuleState::Failed(_))
    ));
}

#[test]
fn loader_init_deferred_retry() {
    fn fd() -> Box<dyn ClientModule> {
        Box::new(DeferOnceModule {
            kind: "defer1",
            init_count: 0,
        })
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("defer1", fd as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let ctx = make_ctx();
    let count = loader.init_all(&ctx);
    assert_eq!(count, 1);
    assert_eq!(loader.state("defer1"), Some(&ClientModuleState::Running));
}

#[test]
fn loader_init_permanently_deferred() {
    fn fd() -> Box<dyn ClientModule> {
        Box::new(AlwaysDeferModule("perma_defer"))
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("perma_defer", fd as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let ctx = make_ctx();
    let count = loader.init_all(&ctx);
    assert_eq!(count, 0);
    assert!(matches!(
        loader.state("perma_defer"),
        Some(ClientModuleState::Failed(_))
    ));
}

#[test]
fn loader_on_all_loaded_only_running() {
    fn fg() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("oal_good"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(FailModule("oal_bad"))
    }
    let factories: HashMap<&str, ClientModuleFactory> = [
        ("oal_good", fg as ClientModuleFactory),
        ("oal_bad", fb as ClientModuleFactory),
    ]
    .into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let ctx = make_ctx();
    loader.init_all(&ctx);
    loader.on_all_loaded(&ctx);
    assert_eq!(loader.running_count(), 1);
}

#[test]
fn loader_exit_reverse_order() {
    let log: CallLog = Arc::new(Mutex::new(Vec::new()));
    let a = MockClientModule::new("rev_a", Arc::clone(&log)).with_deps(&["rev_b"]);
    let b = MockClientModule::new("rev_b", Arc::clone(&log));

    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(b), Box::new(a)];
    let mut loader = ClientModuleLoader::from_modules_for_test(modules).unwrap();
    let ctx = make_ctx();
    loader.init_all(&ctx);
    assert_eq!(loader.running_count(), 2);

    log.lock().unwrap().clear();
    loader.exit_all();

    let calls = log.lock().unwrap().clone();
    let exit_calls: Vec<&String> = calls.iter().filter(|s| s.starts_with("exit:")).collect();
    assert_eq!(exit_calls.len(), 2);
    assert_eq!(exit_calls[0], "exit:rev_a");
    assert_eq!(exit_calls[1], "exit:rev_b");
}

#[test]
fn loader_exit_skips_non_running() {
    fn fg() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("skip_good"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(FailModule("skip_bad"))
    }
    let factories: HashMap<&str, ClientModuleFactory> = [
        ("skip_good", fg as ClientModuleFactory),
        ("skip_bad", fb as ClientModuleFactory),
    ]
    .into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let ctx = make_ctx();
    loader.init_all(&ctx);
    loader.exit_all();
    assert!(matches!(
        loader.state("skip_bad"),
        Some(ClientModuleState::Failed(_))
    ));
}

#[test]
fn loader_into_modules_consumes() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("into_a"))
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("into_a", fa as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let modules = loader.into_modules();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].kind(), "into_a");
}

#[test]
fn loader_state_query() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("sq_a"))
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("sq_a", fa as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();

    assert_eq!(loader.state("sq_a"), Some(&ClientModuleState::Loaded));
    assert_eq!(loader.state("nonexistent"), None);

    let ctx = make_ctx();
    loader.init_all(&ctx);
    assert_eq!(loader.state("sq_a"), Some(&ClientModuleState::Running));
}

#[test]
fn loader_running_count() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("rc_a"))
    }
    fn fb() -> Box<dyn ClientModule> {
        Box::new(FailModule("rc_b"))
    }
    fn fc() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("rc_c"))
    }
    let factories: HashMap<&str, ClientModuleFactory> = [
        ("rc_a", fa as ClientModuleFactory),
        ("rc_b", fb as ClientModuleFactory),
        ("rc_c", fc as ClientModuleFactory),
    ]
    .into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();
    let ctx = make_ctx();
    loader.init_all(&ctx);
    assert_eq!(loader.running_count(), 2);
    assert_eq!(loader.module_count(), 3);
}

#[test]
fn loader_state_transitions() {
    fn fa() -> Box<dyn ClientModule> {
        Box::new(SimpleModule("st_a"))
    }
    let factories: HashMap<&str, ClientModuleFactory> =
        [("st_a", fa as ClientModuleFactory)].into();
    let disabled = HashSet::new();
    let mut loader = ClientModuleLoader::new(factories, &disabled).unwrap();

    assert_eq!(loader.state("st_a"), Some(&ClientModuleState::Loaded));

    let ctx = make_ctx();
    loader.init_all(&ctx);
    assert_eq!(loader.state("st_a"), Some(&ClientModuleState::Running));

    loader.exit_all();
    assert_eq!(loader.state("st_a"), Some(&ClientModuleState::Loaded));
}

#[test]
fn loader_display_error() {
    let err = ClientModuleLoaderError::DependencyResolution("cycle detected".to_string());
    let msg = format!("{err}");
    assert!(msg.contains("cycle detected"));
    assert!(msg.contains("dependency resolution"));
}

#[test]
fn loader_state_can_transition() {
    let loaded = ClientModuleState::Loaded;
    let initializing = ClientModuleState::Initializing;
    let running = ClientModuleState::Running;
    let failed = ClientModuleState::Failed("err".to_string());

    assert!(loaded.can_transition_to(&initializing));
    assert!(initializing.can_transition_to(&running));
    assert!(initializing.can_transition_to(&failed));
    assert!(running.can_transition_to(&loaded));
    assert!(running.can_transition_to(&failed));

    assert!(!loaded.can_transition_to(&running));
    assert!(!loaded.can_transition_to(&failed));
    assert!(!running.can_transition_to(&initializing));
}

#[test]
fn loader_state_display() {
    assert_eq!(format!("{}", ClientModuleState::Loaded), "Loaded");
    assert_eq!(format!("{}", ClientModuleState::Initializing), "Initializing");
    assert_eq!(format!("{}", ClientModuleState::Running), "Running");
    assert_eq!(
        format!("{}", ClientModuleState::Failed("boom".to_string())),
        "Failed(boom)"
    );
}

#[test]
fn loader_from_modules_for_test_basic() {
    let log: CallLog = Arc::new(Mutex::new(Vec::new()));
    let m = MockClientModule::new("fmt_a", Arc::clone(&log));
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(m)];
    let loader = ClientModuleLoader::from_modules_for_test(modules).unwrap();
    assert_eq!(loader.module_count(), 1);
}

#[test]
fn loader_exit_failure_continues() {
    let log: CallLog = Arc::new(Mutex::new(Vec::new()));
    let a = MockClientModule::new("ef_a", Arc::clone(&log));
    let b = MockClientModule::new("ef_b", Arc::clone(&log)).with_exit_fail();
    let c = MockClientModule::new("ef_c", Arc::clone(&log));

    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(a), Box::new(b), Box::new(c)];
    let mut loader = ClientModuleLoader::from_modules_for_test(modules).unwrap();
    let ctx = make_ctx();
    loader.init_all(&ctx);
    assert_eq!(loader.running_count(), 3);

    loader.exit_all();
    assert_eq!(loader.state("ef_a"), Some(&ClientModuleState::Loaded));
    assert!(matches!(
        loader.state("ef_b"),
        Some(ClientModuleState::Failed(_))
    ));
    assert_eq!(loader.state("ef_c"), Some(&ClientModuleState::Loaded));
}
