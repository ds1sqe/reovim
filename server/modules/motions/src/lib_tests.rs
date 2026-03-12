use super::*;

#[test]
fn test_module_trait() {
    let module = MotionsModule;
    assert_eq!(module.id().as_str(), "motions");
    assert_eq!(module.name(), "Vim Motions");
}

#[test]
fn test_module_version() {
    let module = MotionsModule;
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_init() {
    let mut module = MotionsModule::new();
    let ctx = ModuleContext::default();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}

#[test]
fn test_module_exit() {
    let mut module = MotionsModule::new();
    let result = module.exit();
    assert!(result.is_ok());
}

#[test]
fn test_module_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let module: MotionsModule = create_default();
    assert_eq!(module.name(), "Vim Motions");
}

#[test]
fn test_module_new() {
    let _module = MotionsModule::new();
}

#[test]
fn test_command_handlers_total() {
    let module = MotionsModule;
    let handlers = module.command_handlers();
    // 8 word + 6 line + 7 find_char + 7 search = 28
    assert_eq!(handlers.len(), 28);
}

#[test]
fn test_word_commands_count() {
    let cmds = word::all_commands();
    assert_eq!(cmds.len(), 8); // w, b, e, W, B, E, ge, gE
}

#[test]
fn test_line_commands_count() {
    let cmds = line::all_commands();
    assert_eq!(cmds.len(), 6); // 0, $, ^, gg, G, whole-line
}

#[test]
fn test_find_char_commands_count() {
    let cmds = find_char::all_commands();
    assert_eq!(cmds.len(), 7); // dispatch, f, F, t, T, ;, ,
}

#[test]
fn test_search_commands_count() {
    let cmds = search::all_commands();
    assert_eq!(cmds.len(), 7); // /, ?, n, N, *, #, :noh
}

#[test]
fn test_motions_module_id_constant() {
    assert_eq!(MOTIONS_MODULE.as_str(), "motions");
}
