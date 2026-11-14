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

#[test]
fn invalid_strategy_fails_before_accessing_files() {
    let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
        .args(["--mtime", "--strategy", "newset", "missing-a", "missing-b"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("Invalid strategy 'newset'"));
}

#[test]
fn explicit_strategies_are_accepted() {
    for (strategy, expected) in [("oldest", 1000), ("newest", 2000)] {
        let first = NamedTempFile::new().unwrap();
        let second = NamedTempFile::new().unwrap();
        set_file_mtime(first.path(), FileTime::from_unix_time(1000, 0)).unwrap();
        set_file_mtime(second.path(), FileTime::from_unix_time(2000, 0)).unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
            .args(["--mtime", "--strategy", strategy])
            .args([first.path(), second.path()])
            .output()
            .unwrap();

        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let target = std::time::UNIX_EPOCH + std::time::Duration::from_secs(expected);
        assert_eq!(fs::metadata(first.path()).unwrap().modified().unwrap(), target);
        assert_eq!(fs::metadata(second.path()).unwrap().modified().unwrap(), target);
    }
}

#[test]
fn invalid_strategy_never_modifies_existing_files_even_with_force() {
    let first = NamedTempFile::new().unwrap();
    let second = NamedTempFile::new().unwrap();
    set_file_mtime(first.path(), FileTime::from_unix_time(1000, 0)).unwrap();
    set_file_mtime(second.path(), FileTime::from_unix_time(2000, 0)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
        .args(["--mtime", "--force", "--strategy", "newset"])
        .args([first.path(), second.path()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid strategy 'newset'"));
    assert_eq!(
        fs::metadata(first.path()).unwrap().modified().unwrap(),
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000)
    );
    assert_eq!(
        fs::metadata(second.path()).unwrap().modified().unwrap(),
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(2000)
    );
}
