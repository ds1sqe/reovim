//! A path as text, before any tree is asked anything.
//!
//! Where [`crate::walk`] turns a path into an rd by asking, this turns
//! one path into another by spelling alone: what a caller that holds a
//! directory of its own does with a name a person typed.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// `path` under `dir` unless it is absolute, `.` and `..` resolved
/// lexically, without the leading slash: its name under the root.
/// `dir` is a name under the root already.
#[must_use]
pub fn under(dir: &str, path: &str) -> String {
    let joined = if path.starts_with('/') {
        String::from(path)
    } else {
        format!("{dir}/{path}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::under;

    #[test]
    fn under_names_the_file_under_the_root() {
        assert_eq!(under("home/me", "note"), "home/me/note");
        assert_eq!(under("home/me", "./a/../b/"), "home/me/b");
        assert_eq!(under("home/me", "/tmp/x"), "tmp/x");
        assert_eq!(under("", "../../etc/hosts"), "etc/hosts");
        assert_eq!(under("", "/home/me"), "home/me");
        assert_eq!(under("home/me", "/"), "");
    }
}
