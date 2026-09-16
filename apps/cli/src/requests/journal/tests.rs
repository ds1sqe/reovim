use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Transcript(std::path::PathBuf);
impl Transcript {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "reovim-cli-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn open(&self) -> Journal {
        Journal::open(&self.0, "127.0.0.1:1234", [7; 16]).unwrap()
    }
}
impl Drop for Transcript {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}
fn reserve(sequence: u64) -> Frame {
    Frame { caller: [7; 16], action: Action::Reserve(sequence) }
}
#[test]
fn restart_keeps_exact_pending_allocation_and_acknowledged_sequence() {
    let file = Transcript::new();
    let mut journal = file.open();
    journal.begin(reserve(1)).unwrap();
    drop(journal);
    let mut journal = file.open();
    assert_eq!(journal.pending().unwrap(), Some(&reserve(1)));
    assert!(journal.begin(reserve(2)).is_err());
    journal.complete(&Reply::Reserved(81)).unwrap();
    drop(journal);
    let mut journal = file.open();
    assert_eq!(journal.receipts().copied().collect::<Vec<_>>(), vec![81]);
    assert_eq!(journal.next().unwrap(), 2);
    journal
        .begin(Frame { caller: [7; 16], action: Action::Release(81) })
        .unwrap();
    journal.complete(&Reply::Accepted).unwrap();
    drop(journal);
    assert_eq!(file.open().receipts().next(), None);
}
#[test]
fn wrong_reply_and_sequence_preserve_original_custody() {
    let file = Transcript::new();
    let mut journal = file.open();
    assert!(journal.begin(reserve(2)).is_err());
    journal.begin(reserve(1)).unwrap();
    assert!(journal.complete(&Reply::Accepted).is_err());
    assert!(journal.complete(&Reply::Reserved(0)).is_err());
    assert_eq!(journal.pending().unwrap(), Some(&reserve(1)));
    drop(journal);
    assert_eq!(file.open().pending().unwrap(), Some(&reserve(1)));
}
#[test]
fn torn_reply_replays_original_frame() {
    let file = Transcript::new();
    let mut journal = file.open();
    journal.begin(reserve(1)).unwrap();
    journal.file.write_all(b"<partial").unwrap();
    drop(journal);
    let mut journal = file.open();
    assert_eq!(journal.pending().unwrap(), Some(&reserve(1)));
    journal.complete(&Reply::Reserved(91)).unwrap();
    drop(journal);
    assert_eq!(file.open().next().unwrap(), 2);
}
#[test]
fn exclusive_owner_and_credential_are_required() {
    let file = Transcript::new();
    let journal = file.open();
    assert!(Journal::open(&file.0, "127.0.0.1:1234", [7; 16]).is_err());
    drop(journal);
    assert!(Journal::open(&file.0, "127.0.0.1:1234", [8; 16]).is_err());
    assert!(Journal::open(&file.0, "127.0.0.1:1235", [7; 16]).is_err());
    let journal = file.open();
    journal.file.set_len(journal.file.metadata().unwrap().len() - 1).unwrap();
    drop(journal);
    assert!(Journal::open(&file.0, "127.0.0.1:1234", [7; 16]).is_err());
}
#[test]
fn failed_append_fences_memory_until_reopen() {
    let file = Transcript::new();
    let mut journal = file.open();
    // A read-only descriptor makes the real write fail without a mock writer.
    journal.file = File::open(&file.0).unwrap();
    assert!(journal.begin(reserve(1)).is_err());
    assert!(journal.pending().is_err());
    assert!(journal.next().is_err());
    assert!(journal.begin(reserve(1)).is_err());
    drop(journal);
    assert_eq!(file.open().next().unwrap(), 1);
}
