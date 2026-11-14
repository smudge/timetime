use filetime::{set_file_mtime, FileTime};
use std::fs;
use std::time::{Duration, SystemTime};
use tempfile::NamedTempFile;

#[test]
fn creation_time_workaround_is_detected_not_assumed() {
    let file = NamedTempFile::new().unwrap();
    let first = fs::metadata(file.path()).unwrap().created();
    let desired = SystemTime::now() - Duration::from_secs(24 * 3600);
    set_file_mtime(file.path(), FileTime::from_system_time(desired)).unwrap();
    let observed = fs::metadata(file.path()).unwrap();
    let birth = observed.created();
    eprintln!("birth-time behavior: before={first:?}, after backdating mtime={birth:?}");
    // No assertion about btime: this test documents the runner's filesystem.
    assert_eq!(
        FileTime::from_system_time(observed.modified().unwrap()).unix_seconds(),
        FileTime::from_system_time(desired).unix_seconds()
    );
}
