use reovim_call::{arguments, empty, optional, path};
use reovim_kernel_interface::{Error, Value, resource::AccessId};

#[test]
fn paths_produce_absolute_locators_without_escaping_root() {
    assert_eq!(path("/project/src", "../README"), "/project/README");
    assert_eq!(path("/project", "/other/./file"), "/other/file");
    assert_eq!(path("/", "../../"), "/");
}

#[test]
fn optional_arguments_preserve_explicit_empty_values() {
    assert_eq!(arguments(b"a\0\0b").unwrap(), ["a", "", "b"]);
    assert_eq!(optional(b"", ".").unwrap(), ".");
    assert_eq!(optional(b"folder", ".").unwrap(), "folder");
    assert_eq!(optional(b"a\0b", "."), Err(Error::INVALID));
    assert_eq!(arguments(&[0xff]), Err(Error::INVALID));
}

#[test]
fn unit_rejects_unused_access_slots() {
    assert_eq!(empty(&Value::unit()), Ok(()));
    let input = Value::copy(&[], &[AccessId(1)]).unwrap();
    assert_eq!(empty(&input), Err(Error::INVALID));
}
