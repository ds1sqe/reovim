use super::*;

#[test]
fn test_from_uri_scheme_file() {
    assert_eq!(VfsScheme::from_uri_scheme("file"), Some(VfsScheme::File));
    assert_eq!(VfsScheme::from_uri_scheme(""), Some(VfsScheme::File));
}

#[test]
fn test_from_uri_scheme_memory() {
    assert_eq!(VfsScheme::from_uri_scheme("mem"), Some(VfsScheme::Memory));
    assert_eq!(VfsScheme::from_uri_scheme("memory"), Some(VfsScheme::Memory));
}

#[test]
fn test_from_uri_scheme_ssh() {
    assert_eq!(VfsScheme::from_uri_scheme("ssh"), Some(VfsScheme::Ssh));
    assert_eq!(VfsScheme::from_uri_scheme("sftp"), Some(VfsScheme::Ssh));
}

#[test]
fn test_from_uri_scheme_unknown() {
    assert_eq!(VfsScheme::from_uri_scheme("http"), None);
    assert_eq!(VfsScheme::from_uri_scheme("ftp"), None);
}

#[test]
fn test_as_str() {
    assert_eq!(VfsScheme::File.as_str(), "file");
    assert_eq!(VfsScheme::Memory.as_str(), "mem");
    assert_eq!(VfsScheme::Ssh.as_str(), "ssh");
}

#[test]
fn test_service_key_impl() {
    assert_eq!(VfsScheme::service_name(), "VFS");
}

#[test]
fn test_vfs_scheme_debug() {
    let debug = format!("{:?}", VfsScheme::File);
    assert!(debug.contains("File"));
}

#[test]
fn test_vfs_scheme_clone() {
    let a = VfsScheme::Memory;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn test_vfs_scheme_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(VfsScheme::File);
    set.insert(VfsScheme::Memory);
    set.insert(VfsScheme::Ssh);
    set.insert(VfsScheme::File); // Duplicate
    assert_eq!(set.len(), 3);
}

#[test]
fn test_vfs_scheme_equality() {
    assert_eq!(VfsScheme::File, VfsScheme::File);
    assert_ne!(VfsScheme::File, VfsScheme::Memory);
    assert_ne!(VfsScheme::Memory, VfsScheme::Ssh);
}

#[test]
fn test_roundtrip_scheme_str() {
    // File
    let scheme = VfsScheme::from_uri_scheme(VfsScheme::File.as_str());
    assert_eq!(scheme, Some(VfsScheme::File));

    // Memory
    let scheme = VfsScheme::from_uri_scheme(VfsScheme::Memory.as_str());
    assert_eq!(scheme, Some(VfsScheme::Memory));

    // SSH
    let scheme = VfsScheme::from_uri_scheme(VfsScheme::Ssh.as_str());
    assert_eq!(scheme, Some(VfsScheme::Ssh));
}
