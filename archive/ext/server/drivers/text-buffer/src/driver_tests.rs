//! In-process tests for [`crate::TextBufferDriverImpl`].

use {
    crate::TextBufferDriverImpl,
    reovim_kernel::api::v1::ByteEdit,
    reovim_subsys_buffer::{BufferDriver, BufferError},
};

#[test]
fn kind_is_buffer() {
    let drv = TextBufferDriverImpl::new();
    assert_eq!(drv.kind(), "buffer");
}

#[test]
fn probe_reports_kind_and_name() {
    let probe = TextBufferDriverImpl::probe();
    let kind_nul = probe
        .kind
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(probe.kind.len());
    let name_nul = probe
        .name
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(probe.name.len());
    let kind = std::str::from_utf8(&probe.kind[..kind_nul]).unwrap();
    let name = std::str::from_utf8(&probe.name[..name_nul]).unwrap();
    assert_eq!(kind, "buffer");
    assert_eq!(name, "Reovim text buffer driver");
}

#[test]
fn construct_succeeds() {
    let drv = TextBufferDriverImpl::construct().expect("construct ok");
    assert_eq!(drv.kind(), "buffer");
    assert!(drv.list_buffers().is_empty());
}

#[test]
fn create_register_get_close_lifecycle() {
    let drv = TextBufferDriverImpl::new();
    let buf_a = drv
        .create_buffer(b"alpha", None)
        .expect("create_buffer alpha");
    let buf_b = drv
        .create_buffer(b"beta", Some("/tmp/beta".into()))
        .expect("create_buffer beta");

    let id_a = buf_a.id();
    let id_b = buf_b.id();
    assert_ne!(id_a, id_b);

    let mut listed = drv.list_buffers();
    listed.sort_by_key(|id| id.as_usize());
    let mut expected = vec![id_a, id_b];
    expected.sort_by_key(|id| id.as_usize());
    assert_eq!(listed, expected);

    let again = drv.get_buffer(id_a).expect("get_buffer alpha");
    assert_eq!(again.id(), id_a);

    drv.close_buffer(id_a).expect("close alpha");
    assert!(drv.get_buffer(id_a).is_none());
    assert_eq!(drv.list_buffers(), vec![id_b]);
}

#[test]
fn create_buffer_rejects_non_utf8() {
    let drv = TextBufferDriverImpl::new();
    let bytes = [0xff, 0xfe, 0xfd];
    let result = drv.create_buffer(&bytes, None);
    assert!(matches!(result, Err(BufferError::InvalidEdit(_))));
}

#[test]
fn close_buffer_unknown_errors() {
    let drv = TextBufferDriverImpl::new();
    let buf = drv.create_buffer(b"x", None).unwrap();
    let id = buf.id();
    drv.close_buffer(id).unwrap();
    let err = drv.close_buffer(id).unwrap_err();
    assert!(matches!(err, BufferError::NotFound(_)));
}

#[test]
fn open_buffer_reads_file() {
    use std::{
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    path.push(format!("reovim-text-buffer-driver-test-{}-{stamp}.txt", std::process::id()));
    {
        let mut f = std::fs::File::create(&path).expect("create tmpfile");
        f.write_all(b"file contents").unwrap();
    }
    let path_str = path.to_str().unwrap().to_string();

    let drv = TextBufferDriverImpl::new();
    let buf = drv.open_buffer(&path_str).expect("open ok");
    let _ = std::fs::remove_file(&path);
    assert_eq!(buf.size(), b"file contents".len());
    assert_eq!(buf.file_path().as_deref(), Some(path_str.as_str()));
}

#[test]
fn open_buffer_missing_file_errors() {
    let drv = TextBufferDriverImpl::new();
    let result = drv.open_buffer("/nonexistent/path/that/should/not/exist");
    assert!(matches!(result, Err(BufferError::Io(_))));
}

#[test]
fn create_then_apply_edit_via_buffer_handle() {
    let drv = TextBufferDriverImpl::new();
    let buf = drv.create_buffer(b"hi", None).unwrap();
    buf.apply_edit(ByteEdit::insert(2, b"!")).unwrap();
    assert_eq!(buf.size(), 3);
    assert_eq!(buf.read_bytes(0..3).unwrap(), b"hi!".to_vec());
}

#[test]
fn registry_shares_storage_with_buffer_handle() {
    let drv = TextBufferDriverImpl::new();
    let buf = drv.create_buffer(b"shared", None).unwrap();
    let id = buf.id();
    // The registry must hold the same underlying storage so legacy
    // text consumers see edits made through the cdylib path.
    let reg_handle = drv.registry().get(id).expect("registry has buffer");
    assert_eq!(reg_handle.read().byte_len(), 6);
    buf.apply_edit(ByteEdit::insert(6, b"!")).unwrap();
    assert_eq!(reg_handle.read().byte_len(), 7);
}
