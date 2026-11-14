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

#[test]
fn missing_files_fail_with_nonzero_status() {
    for paths in [&[][..], &["only-one-file"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
            .args(["--mtime"])
            .args(paths)
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "{paths:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("At least two file paths"),
            "{paths:?}"
        );
    }
}

#[test]
fn help_succeeds_without_files() {
    for args in [&["--help"][..], &["--help", "only-one-file"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    }
}

#[test]
fn unreadable_or_missing_file_reports_path_without_panicking() {
    let first = NamedTempFile::new().unwrap();
    let missing = first.path().with_extension("missing");
    let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
        .arg("--mtime")
        .args([first.path(), missing.as_path()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(missing.to_str().unwrap()), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn force_still_reports_missing_file_metadata() {
    let first = NamedTempFile::new().unwrap();
    let missing = first.path().with_extension("missing");
    let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
        .args(["--force", "--mtime"])
        .args([first.path(), missing.as_path()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cannot read metadata"), "{stderr}");
    assert!(stderr.contains(missing.to_str().unwrap()), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn dry_run_prints_plan_without_modifying_files() {
    let first = NamedTempFile::new().unwrap();
    let second = NamedTempFile::new().unwrap();
    fs::write(first.path(), b"same").unwrap();
    fs::write(second.path(), b"same").unwrap();
    set_file_mtime(first.path(), FileTime::from_unix_time(1000, 0)).unwrap();
    set_file_mtime(second.path(), FileTime::from_unix_time(2000, 0)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
        .args(["--mtime", "--dry-run"])
        .args([first.path(), second.path()])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(first.path().to_str().unwrap()));
    assert!(stdout.contains(second.path().to_str().unwrap()));
    assert!(stdout.contains("modified=1000"));
    assert_eq!(fs::metadata(first.path()).unwrap().modified().unwrap(), std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000));
    assert_eq!(fs::metadata(second.path()).unwrap().modified().unwrap(), std::time::UNIX_EPOCH + std::time::Duration::from_secs(2000));
}

#[test]
fn dry_run_still_rejects_mismatched_contents() {
    let first = NamedTempFile::new().unwrap();
    let second = NamedTempFile::new().unwrap();
    fs::write(first.path(), b"first").unwrap();
    fs::write(second.path(), b"second").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_timetime"))
        .args(["--mtime", "-n"])
        .args([first.path(), second.path()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("File checksums do not match"));
    assert!(output.stdout.is_empty());
}
