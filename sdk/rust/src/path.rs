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

/// `PATH:LOCATOR` as a name and the rest after it, at the first `:`
/// with something on either side of it; a name with none is itself
/// and no locator.
///
/// The rest is nobody's to read here: a locator belongs to whatever
/// view opens the path, whose own grammar says what it means. Text's
/// holds colons of its own (`12:5`), which is why the path ends at
/// the first `:` and not the last, and no locator of any domain holds
/// a `/`, which is what tells a directory whose name holds a colon
/// from a locator after one.
///
/// A name a file of the whole spelling holds is not this function's
/// to know: the caller asks the tree first, and `a:b` on disk opens
/// `a:b`.
#[must_use]
pub fn split(name: &str) -> (&str, Option<&str>) {
    match name.split_once(':') {
        Some((path, rest))
            if !path.is_empty() && !rest.is_empty() && !rest.contains('/') =>
        {
            (path, Some(rest))
        }
        _ => (name, None),
    }
}

#[cfg(test)]
mod tests {
    use super::{split, under};

    #[test]
    fn under_names_the_file_under_the_root() {
        assert_eq!(under("home/me", "note"), "home/me/note");
        assert_eq!(under("home/me", "./a/../b/"), "home/me/b");
        assert_eq!(under("home/me", "/tmp/x"), "tmp/x");
        assert_eq!(under("", "../../etc/hosts"), "etc/hosts");
        assert_eq!(under("", "/home/me"), "home/me");
        assert_eq!(under("home/me", "/"), "");
    }

    #[test]
    fn a_name_splits_at_the_locator_after_it() {
        assert_eq!(split("README.md:12"), ("README.md", Some("12")));
        // The locator holds colons of its own, so the path ends at
        // the first one and not at the last.
        assert_eq!(split("a/b.md:12:5-14:3"), ("a/b.md", Some("12:5-14:3")));
        assert_eq!(split("README.md"), ("README.md", None));
        assert_eq!(split("x:"), ("x:", None), "an empty rest is no locator");
        assert_eq!(split(":12"), (":12", None), "an empty path is no path");
        assert_eq!(split("a:b"), ("a", Some("b")), "text alone cannot tell");
        // A directory whose own name holds a colon: no locator of any
        // domain holds a slash, so the name is whole.
        assert_eq!(split("d:x/f.md"), ("d:x/f.md", None));
    }
}
