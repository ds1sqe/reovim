//! The std backend over a fresh directory.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use reovim_host::{Error, Kind, clock, fs};

/// A directory of its own for one test, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let name = format!("reovim-host-{}-{test}", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_string()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn list_and_stat_see_kinds() {
    let dir = Dir::new("list");
    std::fs::create_dir(dir.path("sub")).unwrap();
    std::fs::write(dir.path("a.txt"), b"abc").unwrap();

    let mut entries = fs::list(&dir.path("")).unwrap();
    entries.sort_by(|x, y| x.name.cmp(&y.name));
    let names: Vec<(&str, Kind)> =
        entries.iter().map(|e| (e.name.as_str(), e.kind)).collect();
    assert_eq!(names, vec![("a.txt", Kind::File), ("sub", Kind::Dir)]);

    let stat = fs::stat(&dir.path("a.txt")).unwrap();
    assert_eq!((stat.kind, stat.len), (Kind::File, 3));
    assert_eq!(fs::stat(&dir.path("sub")).unwrap().kind, Kind::Dir);
    assert_eq!(fs::stat(&dir.path("none")), Err(Error::NotFound));
    assert_eq!(fs::list(&dir.path("a.txt")), Err(Error::Io));
}

#[test]
fn read_write_create_round_trip() {
    let dir = Dir::new("rw");
    let file = dir.path("f");
    assert_eq!(fs::create(&file), Ok(()));
    assert_eq!(fs::create(&file), Err(Error::Exists));
    assert_eq!(fs::read(&dir.path("none"), 0, 8), Err(Error::NotFound));

    assert_eq!(fs::write(&file, 0, b"hello world"), Ok(11));
    assert_eq!(fs::read(&file, 6, 3).unwrap(), b"wor");
    assert_eq!(fs::read(&file, 6, 100).unwrap(), b"world");
    assert_eq!(fs::read(&file, 11, 8).unwrap(), b"");

    // A write ends the file where its bytes end.
    assert_eq!(fs::write(&file, 6, b"you"), Ok(3));
    assert_eq!(fs::read(&file, 0, 100).unwrap(), b"hello you");
    assert_eq!(fs::stat(&file).unwrap().len, 9);

    // A write makes the file it does not find.
    assert_eq!(fs::write(&dir.path("made"), 0, b"new"), Ok(3));
    assert_eq!(fs::read(&dir.path("made"), 0, 8).unwrap(), b"new");
}

#[test]
fn mtime_moves_with_writes() {
    let dir = Dir::new("mtime");
    let file = dir.path("f");
    fs::create(&file).unwrap();
    fs::write(&file, 0, b"x").unwrap();
    let first = fs::stat(&file).unwrap().mtime;
    // The host stamps files with a coarse clock; give it a tick.
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&file, 0, b"y").unwrap();
    let second = fs::stat(&file).unwrap().mtime;
    assert!(first < second && second <= clock::now());
}

#[test]
fn replace_makes_keeps_the_mode_and_writes_through_a_link() {
    let dir = Dir::new("replace");
    let file = dir.path("f");
    assert_eq!(fs::replace(&file, b"one"), Ok(()));
    assert_eq!(fs::read(&file, 0, 8).unwrap(), b"one");

    let mode = std::fs::Permissions::from_mode(0o640);
    std::fs::set_permissions(&file, mode).unwrap();
    assert_eq!(fs::replace(&file, b"two"), Ok(()));
    assert_eq!(fs::read(&file, 0, 8).unwrap(), b"two");
    let mode = std::fs::metadata(&file).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o640, "the mode survives the rename");

    let link = dir.path("l");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert_eq!(fs::replace(&link, b"three"), Ok(()));
    assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(fs::read(&file, 0, 8).unwrap(), b"three");

    let names: Vec<String> =
        fs::list(&dir.path("")).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names.len(), 2, "no temporary left: {names:?}");
}
