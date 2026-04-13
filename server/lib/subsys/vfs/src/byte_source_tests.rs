use std::{
    borrow::Cow,
    ops::Range,
    path::{Path, PathBuf},
    process,
    sync::Arc,
    thread,
    time::SystemTime,
};

use crate::{
    ByteSource, ByteSourceCapabilities, HeapByteSource, MappedByteSource, MappedFile, StandardVfs,
    VfsDriver,
};

const ONE_MB: usize = 1024 * 1024;

fn temp_file_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock valid")
        .as_nanos();
    std::env::temp_dir().join(format!("reovim-byte-source-{prefix}-{}-{}", process::id(), nanos))
}

fn assert_empty(cow: &[u8]) {
    assert!(cow.is_empty());
}

#[test]
fn heap_empty_cases() {
    let source = HeapByteSource::new(Vec::<u8>::new());

    assert_eq!(source.len(), 0);
    assert!(source.is_empty());
    assert!(source.as_slice().is_some_and(<[u8]>::is_empty));
    assert_empty(source.read(0..0).as_ref());
    assert_empty(source.read(0..10).as_ref());
    let start = 5;
    let end = 1;
    assert_empty(source.read(start..end).as_ref());

    let mut out = Vec::new();
    assert_eq!(source.write_to(&mut out).unwrap(), 0);
    assert!(out.is_empty());
}

#[test]
fn heap_one_byte_cases() {
    let source = HeapByteSource::new(vec![b'x']);

    assert_eq!(source.len(), 1);
    assert!(!source.is_empty());
    assert_eq!(source.read(0..1), Cow::Borrowed(&[b'x'][..]));
    assert_empty(source.read(1..1).as_ref());
    assert_empty(source.read(2..2).as_ref());
    let start = 2;
    let end = 1;
    assert_empty(source.read(start..end).as_ref());
    assert_empty(source.read(1..2).as_ref());

    let mut out = Vec::new();
    source
        .write_to(&mut out)
        .expect("heap write_to must succeed");
    assert_eq!(out, b"x");
}

#[test]
fn mapped_empty_cases() {
    let source =
        MappedByteSource::new(MappedFile::from_vec(b"", Path::new("/reovim-mapped-empty")));

    assert_eq!(source.len(), 0);
    assert!(source.is_empty());
    assert!(source.as_slice().is_some_and(<[u8]>::is_empty));
    assert_empty(source.read(0..0).as_ref());
    assert_empty(source.read(0..10).as_ref());
    let start = 5;
    let end = 1;
    assert_empty(source.read(start..end).as_ref());

    let mut out = Vec::new();
    assert_eq!(source.write_to(&mut out).unwrap(), 0);
    assert!(out.is_empty());
}

#[test]
fn mapped_one_byte_cases() {
    let source =
        MappedByteSource::new(MappedFile::from_vec(b"x", Path::new("/reovim-mapped-one-byte")));

    assert_eq!(source.len(), 1);
    assert!(!source.is_empty());
    assert_eq!(source.read(0..1), Cow::Borrowed(&[b'x'][..]));
    assert_empty(source.read(1..1).as_ref());
    assert_empty(source.read(2..2).as_ref());
    let start = 2;
    let end = 1;
    assert_empty(source.read(start..end).as_ref());
    assert_empty(source.read(1..2).as_ref());

    let mut out = Vec::new();
    source
        .write_to(&mut out)
        .expect("mapped write_to must succeed");
    assert_eq!(out, b"x");
}

#[test]
fn mapped_exact_64mb_round_trip() {
    let mut bytes = vec![0u8; 64 * ONE_MB];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::try_from(idx % 251).expect("byte index fits into u8");
    }

    let mapping = MappedFile::from_vec(&bytes, Path::new("/reovim-mapped-exact-64mb"));
    let source = MappedByteSource::new(mapping);

    let mut out = Vec::new();
    assert_eq!(source.write_to(&mut out).unwrap(), bytes.len() as u64);
    assert_eq!(out, bytes);
}

#[test]
fn mapped_larger_than_64mb_read_is_borrowed() {
    let size = 64 * ONE_MB + 1;
    let mut bytes = vec![0u8; size];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::try_from(idx % 256).expect("byte index fits into u8");
    }

    let mapping = MappedFile::from_vec(&bytes, Path::new("/reovim-mapped-large"));
    let source = MappedByteSource::new(mapping);

    assert!(source.as_slice().is_some_and(|slice| slice.len() == size));

    let range = 1024u64..(1024u64 + 4);
    let read = source.read(range);

    match read {
        Cow::Borrowed(slice) => {
            assert_eq!(slice.len(), 4);
            assert_eq!(slice[0], 0);
            assert_eq!(slice[3], 3);
        }
        Cow::Owned(_) => panic!("mapped reads must be borrowed"),
    }
}

#[test]
fn oob_and_reversed_range_behavior_is_empty_and_safe() {
    let source = HeapByteSource::new(vec![b'a'; 100]);

    assert_empty(source.read(100..200).as_ref());
    let start = 50;
    let end = 40;
    assert_empty(source.read(start..end).as_ref());
    assert_empty(source.read(100..100).as_ref());
    assert_empty(source.read(101..200).as_ref());
    assert_eq!(source.read(0..100), Cow::Borrowed(&[b'a'; 100]));
}

#[test]
fn mapped_oob_and_reversed_range_behavior_is_empty_and_safe() {
    let source =
        MappedByteSource::new(MappedFile::from_vec(&[b'a'; 100], Path::new("/reovim-mapped-oob")));

    assert_empty(source.read(100..200).as_ref());
    let start = 50;
    let end = 40;
    assert_empty(source.read(start..end).as_ref());
    assert_empty(source.read(100..100).as_ref());
    assert_empty(source.read(101..200).as_ref());
    assert_eq!(source.read(0..100), Cow::Borrowed(&[b'a'; 100]));
}

#[test]
fn heap_concurrent_reads() {
    let size = 1024 * ONE_MB;
    let mut bytes = vec![0u8; size];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::try_from(idx % 251).expect("byte index fits into u8");
    }
    let bytes = Arc::new(bytes);

    let source = Arc::new(HeapByteSource::new(bytes.as_slice().to_vec()));

    let handles: Vec<_> = (0u8..8)
        .map(|_| {
            let source = Arc::clone(&source);
            let bytes = Arc::clone(&bytes);
            thread::spawn(move || {
                let mut cursor = 0usize;
                while cursor < 1024 {
                    let end = cursor.saturating_add(16);
                    let read = source.read(cursor as u64..end as u64);
                    assert_eq!(&*read, &bytes[cursor..end]);

                    cursor = (cursor + 16) % (bytes.len().saturating_sub(16));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread join");
    }
}

#[test]
fn concurrent_reads() {
    let size = 1024 * ONE_MB;
    let mut bytes = vec![0u8; size];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::try_from(idx % 251).expect("byte index fits into u8");
    }
    let bytes = Arc::new(bytes);

    let source = Arc::new(MappedByteSource::new(MappedFile::from_vec(
        bytes.as_slice(),
        Path::new("/reovim-concurrent"),
    )));

    let handles: Vec<_> = (0u8..8)
        .map(|_| {
            let source = Arc::clone(&source);
            let bytes = Arc::clone(&bytes);
            thread::spawn(move || {
                let mut cursor = 0usize;
                while cursor < 1024 {
                    let end = cursor.saturating_add(16);
                    let read = source.read(cursor as u64..end as u64);
                    assert_eq!(&*read, &bytes[cursor..end]);

                    cursor = (cursor + 16) % (bytes.len().saturating_sub(16));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread join");
    }
}

#[test]
fn mapped_byte_source_capabilities_for_real_and_fallback_sources() {
    let from_vec = MappedByteSource::new(MappedFile::from_vec(
        b"fallback mmap source",
        Path::new("/reovim-mapped-fallback"),
    ));
    let expected = ByteSourceCapabilities::RANDOM_ACCESS | ByteSourceCapabilities::MMAP_BACKED;
    assert_eq!(from_vec.capabilities(), expected);

    let path = temp_file_path("capabilities-real");
    let vfs = StandardVfs::new();
    vfs.write(&path, b"real mmap source")
        .expect("write test file");
    let source = MappedByteSource::new(vfs.mmap_read(&path).expect("mmap file"));
    assert_eq!(source.capabilities(), expected);

    vfs.delete(&path).expect("delete test file");
}

#[test]
fn as_any_downcasts() {
    let heap = HeapByteSource::new(vec![b'h'; 3]);
    assert!(heap.as_any().downcast_ref::<HeapByteSource>().is_some());
    assert!(heap.as_any().downcast_ref::<MappedByteSource>().is_none());

    let mapped =
        MappedByteSource::new(MappedFile::from_vec(b"mapped", Path::new("/reovim-any-mapped")));
    assert!(mapped.as_any().downcast_ref::<MappedByteSource>().is_some());
    assert!(mapped.as_any().downcast_ref::<HeapByteSource>().is_none());
}

#[test]
fn heap_capabilities() {
    let source = HeapByteSource::new(vec![1, 2, 3]);
    let capabilities = source.capabilities();
    assert!(capabilities.contains(ByteSourceCapabilities::RANDOM_ACCESS));
    assert!(!capabilities.contains(ByteSourceCapabilities::MMAP_BACKED));
    assert_eq!(capabilities, ByteSourceCapabilities::RANDOM_ACCESS);
}

#[test]
fn capability_bit_values_are_stable() {
    assert_eq!(ByteSourceCapabilities::RANDOM_ACCESS.bits(), 1);
    assert_eq!(ByteSourceCapabilities::WRITABLE.bits(), 1 << 1);
    assert_eq!(ByteSourceCapabilities::STREAMING.bits(), 1 << 2);
    assert_eq!(ByteSourceCapabilities::MMAP_BACKED.bits(), 1 << 3);

    let combined = ByteSourceCapabilities::RANDOM_ACCESS | ByteSourceCapabilities::MMAP_BACKED;
    assert!(combined.contains(ByteSourceCapabilities::RANDOM_ACCESS));
    assert!(combined.contains(ByteSourceCapabilities::MMAP_BACKED));
    assert!(!combined.contains(ByteSourceCapabilities::WRITABLE));

    let bits = combined.bits();
    let restored = ByteSourceCapabilities::from_bits_truncate(bits);
    assert_eq!(restored, combined);
    assert!(ByteSourceCapabilities::empty().is_empty());
}

#[test]
fn default_write_to_for_non_sliceable_source() {
    struct NonSliceSource;

    impl NonSliceSource {
        const fn new() -> Self {
            Self
        }
    }

    impl ByteSource for NonSliceSource {
        fn len(&self) -> u64 {
            0
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_slice(&self) -> Option<&[u8]> {
            None
        }

        fn read(&self, _range: Range<u64>) -> Cow<'_, [u8]> {
            Cow::Borrowed(&[])
        }

        fn capabilities(&self) -> ByteSourceCapabilities {
            ByteSourceCapabilities::STREAMING
        }
    }

    let source = NonSliceSource::new();
    let mut sink = Vec::new();
    let error = source
        .write_to(&mut sink)
        .expect_err("write_to should fail");
    assert_eq!(
        error.to_string(),
        "ByteSource is not sliceable and has no streaming write_to impl",
    );
    assert!(sink.is_empty());
}

#[test]
fn heap_write_to_round_trip() {
    let source = HeapByteSource::new(b"hello".to_vec());
    let mut out = Vec::new();
    source
        .write_to(&mut out)
        .expect("heap write_to should succeed");
    assert_eq!(out, b"hello");
    assert_eq!(out.len() as u64, source.len());
    assert_eq!(out.len(), 5);
}
