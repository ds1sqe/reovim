use super::*;

#[test]
fn std_env_matches_std_env_var_for_path() {
    // PATH is reliably set in CI and dev shells. Don't mutate the
    // environment — concurrent tests would race, and 2024-edition
    // set_var/remove_var are unsafe.
    assert_eq!(StdEnv.get("PATH"), std::env::var("PATH").ok());
}

#[test]
fn std_env_returns_none_for_unset_key() {
    let key = "REOVIM_DYLIB_LOADER_DEFINITELY_UNSET_XYZ_12345";
    assert_eq!(StdEnv.get(key), None);
}
