use super::*;

#[test]
fn test_term_env_new() {
    let env = TermEnv::new((120, 40), ColorMode::Color256);
    assert_eq!(env.size, (120, 40));
    assert_eq!(env.color_mode, ColorMode::Color256);
}

#[test]
fn test_term_env_default() {
    let env = TermEnv::default();
    assert_eq!(env.size, (80, 24));
    assert_eq!(env.color_mode, ColorMode::TrueColor);
}

#[test]
fn test_tui_env_headless() {
    let env = TuiEnv::headless(100, 30);
    match env {
        TuiEnv::Headless(term_env) => {
            assert_eq!(term_env.size, (100, 30));
            assert_eq!(term_env.color_mode, ColorMode::TrueColor);
        }
        TuiEnv::Real => panic!("Expected Headless"),
    }
}
