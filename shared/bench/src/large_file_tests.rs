#![allow(clippy::cast_possible_truncation)] // Test constants are small.

use super::*;

#[test]
fn test_utf8_log_empty() {
    let mut buf = Vec::new();
    let written = write_utf8_log(&mut buf, 0).unwrap();
    assert_eq!(written, 0);
    assert!(buf.is_empty());
}

#[test]
fn test_utf8_log_small() {
    let mut buf = Vec::new();
    let target = 1024;
    let written = write_utf8_log(&mut buf, target).unwrap();
    assert!(written <= target);
    assert_eq!(written, buf.len() as u64);

    // Every byte must be valid UTF-8.
    let text = std::str::from_utf8(&buf).expect("output must be valid UTF-8");

    // Every line ends with \n.
    assert!(text.ends_with('\n'));

    // Lines are non-empty.
    for line in text.lines() {
        assert!(!line.is_empty());
    }
}

#[test]
fn test_utf8_log_exact_target_not_exceeded() {
    let mut buf = Vec::new();
    let target = 500;
    let written = write_utf8_log(&mut buf, target).unwrap();
    assert!(written <= target);
    assert_eq!(buf.len() as u64, written);
}

#[test]
fn test_utf8_log_lines_cycle_templates() {
    let mut buf = Vec::new();
    // Large enough to cycle through all 6 templates multiple times.
    let target = 10_000;
    let written = write_utf8_log(&mut buf, target).unwrap();
    assert!(written > 0);

    let text = std::str::from_utf8(&buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();

    // Must have exercised multiple templates.
    assert!(lines.len() > 6);

    // Spot-check: first line should contain "INFO" (template 0).
    assert!(lines[0].contains("INFO"));
    // Second line should contain "DEBUG" (template 1).
    assert!(lines[1].contains("DEBUG"));
}

#[test]
fn test_utf8_log_target_too_small_for_one_line() {
    let mut buf = Vec::new();
    // Smallest template is ~80 bytes; target 10 is too small.
    let written = write_utf8_log(&mut buf, 10).unwrap();
    assert_eq!(written, 0);
    assert!(buf.is_empty());
}

#[test]
fn test_elf_binary_minimum_size() {
    let mut buf = Vec::new();
    let written = write_elf_binary(&mut buf, 0).unwrap();
    // Minimum 64 bytes (ELF header).
    assert_eq!(written, 64);
    assert_eq!(buf.len(), 64);
}

#[test]
fn test_elf_binary_exact_64() {
    let mut buf = Vec::new();
    let written = write_elf_binary(&mut buf, 64).unwrap();
    assert_eq!(written, 64);
    assert_eq!(buf.len(), 64);
}

#[test]
fn test_elf_binary_magic() {
    let mut buf = Vec::new();
    write_elf_binary(&mut buf, 128).unwrap();

    // ELF magic: \x7fELF
    assert_eq!(&buf[0..4], b"\x7fELF");
}

#[test]
fn test_elf_binary_class_and_endian() {
    let mut buf = Vec::new();
    write_elf_binary(&mut buf, 128).unwrap();

    // ELFCLASS64
    assert_eq!(buf[4], 2);
    // ELFDATA2LSB (little-endian)
    assert_eq!(buf[5], 1);
    // EV_CURRENT
    assert_eq!(buf[6], 1);
}

#[test]
fn test_elf_binary_type_and_machine() {
    let mut buf = Vec::new();
    write_elf_binary(&mut buf, 128).unwrap();

    // e_type: ET_EXEC (2) at offset 16, little-endian u16
    assert_eq!(u16::from_le_bytes([buf[16], buf[17]]), 2);
    // e_machine: EM_X86_64 (0x3E) at offset 18
    assert_eq!(u16::from_le_bytes([buf[18], buf[19]]), 0x3E);
}

#[test]
fn test_elf_binary_ehsize() {
    let mut buf = Vec::new();
    write_elf_binary(&mut buf, 128).unwrap();

    // e_ehsize at offset 52, little-endian u16
    assert_eq!(u16::from_le_bytes([buf[52], buf[53]]), 64);
}

#[test]
fn test_elf_binary_zero_fill() {
    let mut buf = Vec::new();
    let target = 1024;
    let written = write_elf_binary(&mut buf, target).unwrap();
    assert_eq!(written, target);
    assert_eq!(buf.len(), target as usize);

    // Everything after the 64-byte header should be zero.
    assert!(buf[64..].iter().all(|&b| b == 0));
}

#[test]
fn test_elf_binary_large() {
    let mut buf = Vec::new();
    let target = 100_000;
    let written = write_elf_binary(&mut buf, target).unwrap();
    assert_eq!(written, target);
    assert_eq!(buf.len(), target as usize);
    assert_eq!(&buf[0..4], b"\x7fELF");
}

#[test]
fn test_write_zeros_exact_chunk() {
    let mut buf = Vec::new();
    let count = 64 * 1024; // Exactly one chunk.
    write_zeros(&mut buf, count).unwrap();
    assert_eq!(buf.len(), count as usize);
    assert!(buf.iter().all(|&b| b == 0));
}

#[test]
fn test_write_zeros_partial_chunk() {
    let mut buf = Vec::new();
    let count = 100; // Well under one chunk.
    write_zeros(&mut buf, count).unwrap();
    assert_eq!(buf.len(), count as usize);
    assert!(buf.iter().all(|&b| b == 0));
}

#[test]
fn test_write_zeros_zero() {
    let mut buf = Vec::new();
    write_zeros(&mut buf, 0).unwrap();
    assert!(buf.is_empty());
}
