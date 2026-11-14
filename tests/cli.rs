use filetime::{set_file_mtime, FileTime};
use std::fs;
use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn checksum_mismatch_fails_without_changing_timestamps() {
    let first = NamedTempFile::new().unwrap();
    let second = NamedTempFile::new().unwrap();
    fs::write(first.path(), b"one").unwrap();
    fs::write(second.path(), b"two").unwrap();
    set_file_mtime(first.path(), FileTime::from_unix_time(1000, 0)).unwrap();
    set_file_mtime(second.path(), FileTime::from_unix_time(2000, 0)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
        .arg("--mtime")
        .args([first.path(), second.path()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("File checksums do not match"));
    assert_eq!(fs::metadata(first.path()).unwrap().modified().unwrap(),
               std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000));
    assert_eq!(fs::metadata(second.path()).unwrap().modified().unwrap(),
               std::time::UNIX_EPOCH + std::time::Duration::from_secs(2000));
}
