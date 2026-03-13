use super::*;

#[test]
fn test_parse_source_git_https() {
    let source = parse_source("https://github.com/user/repo.git", None);
    assert!(source.is_git());
    assert!(!source.is_path());
}

#[test]
fn test_parse_source_git_ssh() {
    let source = parse_source("git@github.com:user/repo.git", None);
    assert!(source.is_git());
}

#[test]
fn test_parse_source_git_with_rev() {
    let source = parse_source("https://github.com/user/repo.git", Some("v1.0.0"));
    match source {
        ModuleSource::Git { url, rev } => {
            assert_eq!(url, "https://github.com/user/repo.git");
            assert_eq!(rev, Some("v1.0.0".to_string()));
        }
        ModuleSource::Path { .. } => panic!("expected git source"),
    }
}

#[test]
fn test_parse_source_local_path() {
    let source = parse_source("/home/user/my-module", None);
    assert!(source.is_path());
    assert!(!source.is_git());
}

#[test]
fn test_parse_source_relative_path() {
    let source = parse_source("./my-module", None);
    assert!(source.is_path());
}

#[test]
fn test_parse_source_http() {
    let source = parse_source("http://example.com/repo.git", None);
    assert!(source.is_git());
}
