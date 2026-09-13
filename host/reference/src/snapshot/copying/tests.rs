use super::*;
use std::{fs, sync::mpsc};

#[test]
fn sparse_copy_clears_old_candidate_data_in_holes() {
    let directory = std::env::temp_dir()
        .join(alloc::format!("lease-holes-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("source");
    let writer = File::create(&path).unwrap();
    writer.set_len(12288).unwrap();
    writer.write_all_at(b"head", 0).unwrap();
    writer.write_all_at(b"tail", 12284).unwrap();
    drop(writer);
    let source = File::open(&path).unwrap();
    let snapshot = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(directory.join("candidate"))
        .unwrap();
    snapshot.write_all_at(b"stale", 4096).unwrap();
    let (copied, _) = capture(&source, snapshot).unwrap();
    let mut bytes = [0; 5];
    copied.read_exact_at(&mut bytes, 4096).unwrap();
    assert_eq!(bytes, [0; 5]);
    let mut marker = [0; 4];
    copied.read_exact_at(&mut marker, 0).unwrap();
    assert_eq!(&marker, b"head");
    copied.read_exact_at(&mut marker, 12284).unwrap();
    assert_eq!(&marker, b"tail");
    drop(copied);
    drop(source);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn writer_break_discards_a_partly_copied_candidate_and_releases_lease() {
    let directory = std::env::temp_dir()
        .join(alloc::format!("lease-break-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("source");
    fs::write(&path, b"original contents").unwrap();
    let source = File::open(&path).unwrap();
    let snapshot = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("candidate"))
        .unwrap();
    let (ready, waiting) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let lease = lease(&source).unwrap();
        copy_range(&lease, &snapshot, &mut [0; 4], 0, 4).unwrap();
        ready.send(()).unwrap();
        resumed.recv().unwrap();
        copy_range(&lease, &snapshot, &mut [0; 4], 4, 17)
    });
    waiting.recv().unwrap();
    let rejected = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
        .unwrap_err();
    assert_eq!(rejected.raw_os_error(), Some(libc::EWOULDBLOCK));
    resume.send(()).unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap_err().raw_os_error(),
        Some(libc::ESTALE)
    );
    let writer = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
        .unwrap();
    writer.write_all_at(b"changed!", 0).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"changed! contents");
    drop(writer);
    fs::remove_dir_all(directory).unwrap();
}
