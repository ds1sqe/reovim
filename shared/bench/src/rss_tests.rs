use super::*;

const TYPICAL_STATUS: &str = "\
Name:\ttest_process
Umask:\t0022
State:\tS (sleeping)
Tgid:\t12345
Ngid:\t0
Pid:\t12345
PPid:\t1
TracerPid:\t0
Uid:\t1000\t1000\t1000\t1000
Gid:\t1000\t1000\t1000\t1000
FDSize:\t256
VmPeak:\t  123456 kB
VmSize:\t  100000 kB
VmLck:\t       0 kB
VmPin:\t       0 kB
VmHWM:\t   98765 kB
VmRSS:\t   45678 kB
RssAnon:\t   30000 kB
RssFile:\t   15678 kB
RssShmem:\t       0 kB
VmData:\t   50000 kB
VmStk:\t     132 kB
VmExe:\t       4 kB
VmLib:\t   12000 kB
VmPTE:\t     200 kB
Threads:\t4
";

#[test]
fn test_parse_vm_rss_typical() {
    let rss = parse_vm_rss(TYPICAL_STATUS).unwrap();
    // 45678 kB * 1024 = 46_774_272 bytes
    assert_eq!(rss, 45_678 * 1024);
}

#[test]
fn test_parse_vm_rss_no_leading_spaces() {
    let status = "VmRSS:\t1234 kB\n";
    let rss = parse_vm_rss(status).unwrap();
    assert_eq!(rss, 1234 * 1024);
}

#[test]
fn test_parse_vm_rss_large_value() {
    let status = "VmRSS:\t 16777216 kB\n";
    let rss = parse_vm_rss(status).unwrap();
    // 16 GiB in kB * 1024
    assert_eq!(rss, 16_777_216 * 1024);
}

#[test]
fn test_parse_vm_rss_missing() {
    let status = "Name:\ttest\nVmSize:\t1000 kB\n";
    let err = parse_vm_rss(status).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn test_parse_vm_rss_empty_value() {
    let status = "VmRSS:\t\n";
    let err = parse_vm_rss(status).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn test_parse_vm_rss_non_numeric() {
    let status = "VmRSS:\tabc kB\n";
    let err = parse_vm_rss(status).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn test_parse_vm_rss_empty_input() {
    let err = parse_vm_rss("").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn test_parse_vm_rss_only_whitespace_after_prefix() {
    // Tab + spaces but no actual number before newline.
    let status = "VmRSS:\t   \n";
    let err = parse_vm_rss(status).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[cfg(target_os = "linux")]
#[test]
fn test_current_rss_bytes_live() {
    let rss = current_rss_bytes().unwrap();
    // A running process should have at least some RSS.
    assert!(rss > 0);
    // And less than 100 GiB (sanity upper bound).
    assert!(rss < 100 * 1024 * 1024 * 1024);
}

#[test]
fn test_measure_rss_returns_closure_result() {
    let (result, _before, _after) = measure_rss(|| 42);
    assert_eq!(result, 42);
}
