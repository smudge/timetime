extern crate data_encoding;
extern crate filetime;
extern crate getopts;
extern crate ring;

use data_encoding::HEXLOWER;
use filetime::FileTime;
use getopts::Options;
use ring::digest::{Context, Digest, SHA256};
use std::env;
use std::fs::{self, File, Metadata};
use std::io;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn print_usage(opts: Options) {
    let args: Vec<String> = env::args().collect();
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("  {}\n", env!("CARGO_PKG_DESCRIPTION"));
    println!("Usage:");
    print!("  {} [opts] file file [file...]{}", args[0], opts.usage(""));
}

fn main() {
    let mut opts = Options::new();
    opts.optopt("s", "strategy", "oldest (default) or newest", "VAL");
    opts.optflag("t", "tz-safety", "halt if timezones match");
    opts.optflag("m", "mtime", "only compare 'modified' timestamp");
    opts.optflag("f", "force", "ignore checksum differences");
    opts.optflag("h", "help", "this help message");
    opts.optflag("n", "dry-run", "show planned timestamps without changing files");

    let args: Vec<String> = env::args().collect();
    let matches = opts.parse(&args[1..]).unwrap();

    match (
        matches.free.get(0),
        matches.free.get(1),
        matches.opt_str("s"),
        matches.opt_present("m"),
        matches.opt_present("t"),
        matches.opt_present("f"),
        matches.opt_present("h"),
        matches.opt_present("n"),
    ) {
        (_, _, _, _, _, _, true, _) => print_usage(opts),
        (_, None, _, _, _, _, false, _) => {
            eprintln!("At least two file paths are required");
            std::process::exit(2);
        }
        (Some(_), Some(_), Some(ref strategy), _, _, _, false, _)
            if !matches!(strategy.as_str(), "oldest" | "newest") =>
        {
            eprintln!("Invalid strategy '{}': expected oldest or newest", strategy);
            std::process::exit(2);
        }
        (Some(_), Some(_), s, m, t, f, false, dry_run) => {
            if let Err(error) = run_with_mode(matches.free, s, m, f, t, dry_run) {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        },
        _ => print_usage(opts),
    }
}

fn run(files: Vec<String>, strategy: Option<String>, mtime: bool, force: bool, tzsafety: bool) -> Result<(), String> {
    run_with_mode(files, strategy, mtime, force, tzsafety, false)
}

fn run_with_mode(files: Vec<String>, strategy: Option<String>, mtime: bool, force: bool, tzsafety: bool, dry_run: bool) -> Result<(), String> {
    let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
    if !force {
        let first = checksum(&paths[0])
            .map_err(|e| format!("Cannot read '{}': {}", paths[0].display(), e))?;
        for path in paths.iter().skip(1) {
            let digest = checksum(path)
                .map_err(|e| format!("Cannot read '{}': {}", path.display(), e))?;
            if digest != first {
                return Err("File checksums do not match! Cancelling operation...".into());
            }
        }
    }

    let metadata: Vec<Metadata> = paths
        .iter()
        .map(|path| {
            fs::metadata(path)
                .map_err(|e| format!("Cannot read metadata for '{}': {}", path.display(), e))
        })
        .collect::<Result<_, _>>()?;
    let modified_times = read_times(&metadata, &paths, |m| m.modified())?;
    let created_times = match mtime {
        true => None,
        false => Some(read_times(&metadata, &paths, |m| m.created())?),
    };
    let plan = plan_timestamps(&modified_times, created_times.as_deref(), tzsafety, strategy.as_deref())?;
    if plan.timezone_warning {
        eprintln!("Warning: files may have matching timestamps from different timezones");
    }
    if dry_run {
        for path in &paths {
            match plan.created {
                Some(created) => println!("{}: created={} modified={}", path.display(), created.unix_seconds(), plan.modified.unix_seconds()),
                None => println!("{}: modified={}", path.display(), plan.modified.unix_seconds()),
            }
        }
        return Ok(());
    }
    for path in &paths {
        if let Some(created) = plan.created {
            filetime::set_file_mtime(path, created)
                .map_err(|e| format!("Cannot set timestamp for '{}': {}", path.display(), e))?;
        }
        filetime::set_file_mtime(path, plan.modified)
            .map_err(|e| format!("Cannot set timestamp for '{}': {}", path.display(), e))?;
        // Verify each file before proceeding to the next.
        verify_timestamps(path, &plan)?;
    }
    Ok(())
}

// Compare whole seconds, the precision used by the CLI preview. A filesystem
// with coarser precision fails verification instead of falsely claiming success.
fn verify_timestamp(
    path: &Path,
    field: &str,
    expected: FileTime,
    observed: io::Result<SystemTime>,
) -> Result<(), String> {
    let observed = observed
        .map_err(|e| format!("Cannot verify {} for '{}': {}", field, path.display(), e))?;
    let actual = FileTime::from_system_time(observed);
    match expected.unix_seconds() == actual.unix_seconds() {
        true => Ok(()),
        false => Err(format!(
            "{} not preserved for '{}': expected {}, found {} (filesystem may not support this update)",
            field,
            path.display(),
            expected.unix_seconds(),
            actual.unix_seconds()
        )),
    }
}

fn verify_timestamps(path: &Path, plan: &TimestampPlan) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Cannot verify '{}': {}", path.display(), e))?;
    verify_timestamp(path, "Modification time", plan.modified, metadata.modified())?;
    match plan.created {
        Some(created) => verify_timestamp(path, "Creation time", created, metadata.created()),
        None => Ok(()),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TimestampPlan {
    modified: FileTime,
    created: Option<FileTime>,
    timezone_warning: bool,
}

fn read_times<F: Fn(&Metadata) -> io::Result<SystemTime>>(
    metadata: &[Metadata], paths: &[PathBuf], read: F,
) -> Result<Vec<SystemTime>, String> {
    metadata.iter().zip(paths).map(|(m, path)| {
        read(m).map_err(|e| format!("Cannot read timestamp for '{}': {}", path.display(), e))
    }).collect()
}

fn plan_timestamps(
    modified: &[SystemTime], created: Option<&[SystemTime]>,
    tzsafety: bool, strategy: Option<&str>,
) -> Result<TimestampPlan, String> {
    fn choose(times: &[SystemTime], strategy: Option<&str>) -> Result<FileTime, String> {
        match strategy {
            Some("newest") => times.iter().max(),
            _ => times.iter().min(),
        }.copied()
         .map(FileTime::from_system_time)
         .ok_or_else(|| "No file timestamps were available".to_string())
    }

    let warning = tz_check(modified) || created.is_some_and(tz_check);
    if tzsafety && warning {
        return Err("Timezone Safety Check Failed! Files may have matching timestamps from different timezones.".into());
    }

    let modified_time = choose(modified, strategy)?;
    let created_time = created.map(|times| choose(times, strategy)).transpose()?;
    if created_time.is_some_and(|created| created > modified_time) {
        return Err("Canonical creation time cannot be after modified time".into());
    }

    Ok(TimestampPlan {
        modified: modified_time,
        created: created_time,
        timezone_warning: warning,
    })
}

fn tz_check(times: &[SystemTime]) -> bool {
    match times {
        [] | [_] => return false,
        _ => {}
    }
    let mut times = times.to_vec();
    times.sort();
    let mut left = times.clone();
    let mut right = times.clone();
    left.remove(right.len() - 1);
    right.remove(0);
    let mut deltas = left
        .iter()
        .zip(right.iter())
        .map(|(l, r)| r.duration_since(l.clone()).unwrap());
    deltas.any(|dur| dur.as_secs() > 0 && dur.subsec_nanos() == 0 && dur.as_secs() % 3600 == 0)
}

fn checksum(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let digest = sha256_digest(reader)?;
    Ok(HEXLOWER.encode(digest.as_ref()))
}

fn sha256_digest<R: Read>(mut reader: R) -> io::Result<Digest> {
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    Ok(context.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use std::time::{Duration, SystemTime};
    use tempfile::NamedTempFile;

    fn secs(st: SystemTime) -> u64 {
        st.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
    }

    fn create_file_with_mtime(secs: i64) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "data").unwrap();
        let ft = FileTime::from_unix_time(secs, 0);
        filetime::set_file_mtime(f.path(), ft).unwrap();
        f
    }

    #[test]
    fn sha256_digest_known_value() {
        let digest = sha256_digest(Cursor::new(b"abc")).unwrap();
        assert_eq!(
            HEXLOWER.encode(digest.as_ref()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn checksum_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "hello").unwrap();
        file.flush().unwrap();
        assert_eq!(
            checksum(file.path()).unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn tz_check_detects_exact_hour() {
        let base = SystemTime::UNIX_EPOCH;
        let times = vec![base, base + Duration::from_secs(3600)];
        assert!(tz_check(&times));
    }

    #[test]
    fn tz_check_detects_multiple_hours_but_not_identical_times() {
        let base = SystemTime::UNIX_EPOCH;
        assert!(tz_check(&vec![base, base + Duration::from_secs(7200)]));
        assert!(!tz_check(&vec![base, base]));
    }

    #[test]
    fn tz_check_non_hour_delta() {
        let base = SystemTime::UNIX_EPOCH;
        let times = vec![base, base + Duration::from_secs(3599)];
        assert!(!tz_check(&times));
    }

    #[test]
    fn tz_check_ignores_fractional_hour_offsets() {
        let base = SystemTime::UNIX_EPOCH;
        let times = vec![base, base + Duration::from_secs(3600) + Duration::from_nanos(1)];
        assert!(!tz_check(&times));
    }

    #[test]
    fn plans_oldest_and_newest_without_file_access() {
        let base = SystemTime::UNIX_EPOCH;
        let modified = [base + Duration::from_secs(1000), base + Duration::from_secs(2000)];
        let oldest = plan_timestamps(&modified, None, false, None).unwrap();
        let newest = plan_timestamps(&modified, None, false, Some("newest")).unwrap();
        assert_eq!(oldest.modified.unix_seconds(), 1000);
        assert_eq!(newest.modified.unix_seconds(), 2000);
        assert_eq!(oldest.created, None);
        assert!(!oldest.timezone_warning);
    }

    #[test]
    fn plans_creation_and_modification_together() {
        let base = SystemTime::UNIX_EPOCH;
        let modified = [base + Duration::from_secs(1200), base + Duration::from_secs(2400)];
        let created = [base + Duration::from_secs(900), base + Duration::from_secs(1800)];
        let plan = plan_timestamps(&modified, Some(&created), false, None).unwrap();
        assert_eq!(plan.modified.unix_seconds(), 1200);
        assert_eq!(plan.created.unwrap().unix_seconds(), 900);
    }

    #[test]
    fn rejects_inverted_creation_time() {
        let base = SystemTime::UNIX_EPOCH;
        assert!(plan_timestamps(
            &[base + Duration::from_secs(1000)],
            Some(&[base + Duration::from_secs(2000)]), false, None,
        ).unwrap_err().contains("creation time"));
    }

    #[test]
    fn planning_enforces_timezone_safety() {
        let base = SystemTime::UNIX_EPOCH;
        let modified = [base, base + Duration::from_secs(3600)];
        assert!(plan_timestamps(&modified, None, false, None).unwrap().timezone_warning);
        assert!(plan_timestamps(&modified, None, true, None)
            .unwrap_err().contains("Timezone Safety Check Failed"));
    }

    #[test]
    fn planning_requires_timestamps() {
        assert!(plan_timestamps(&[], None, false, None).is_err());
    }

    #[test]
    fn verification_checks_birth_time_as_well_as_mtime() {
        let path = Path::new("example");
        let expected = FileTime::from_unix_time(1000, 0);
        let actual = SystemTime::UNIX_EPOCH + Duration::from_secs(2000);
        let mismatch = verify_timestamp(path, "Creation time", expected, Ok(actual)).unwrap_err();
        assert!(mismatch.contains("Creation time not preserved"));
        assert!(mismatch.contains("example"));
        assert!(verify_timestamp(path, "Modification time", expected,
            Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(1000))).is_ok());
    }

    #[test]
    fn filesystem_probe_characterizes_indirect_birth_time_changes() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let original_birth = fs::metadata(path).unwrap().created();
        let early = SystemTime::now() - Duration::from_secs(7 * 24 * 3600);
        let later = early + Duration::from_secs(3600);

        filetime::set_file_mtime(path, FileTime::from_system_time(early)).unwrap();
        let after_early = fs::metadata(path).unwrap().created();
        filetime::set_file_mtime(path, FileTime::from_system_time(later)).unwrap();
        let after_restore = fs::metadata(path).unwrap();
        verify_timestamp(path, "Modification time",
            FileTime::from_system_time(later), after_restore.modified()).unwrap();

        eprintln!(
            "filesystem birth-time probe: original={:?}, after mtime backdate={:?}, after restoring mtime={:?}",
            original_birth, after_early, after_restore.created()
        );
        // Some filesystems adjust btime with mtime, others don't. Record the
        // observed behavior without assuming the test runner's filesystem.
    }

    #[test]
    fn run_updates_to_oldest() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(2000);
        let p1 = f1.path().to_str().unwrap().to_string();
        let p2 = f2.path().to_str().unwrap().to_string();

        run(vec![p1.clone(), p2.clone()], None, true, false, false).unwrap();

        let m1 = fs::metadata(&p1).unwrap().modified().unwrap();
        let m2 = fs::metadata(&p2).unwrap().modified().unwrap();
        assert_eq!(secs(m1), 1000);
        assert_eq!(secs(m2), 1000);
    }

    #[test]
    fn run_checksum_mismatch_no_force() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(2000);
        fs::write(f2.path(), b"other").unwrap();
        let ft = FileTime::from_unix_time(2000, 0);
        filetime::set_file_mtime(f2.path(), ft).unwrap();

        let p1 = f1.path().to_str().unwrap().to_string();
        let p2 = f2.path().to_str().unwrap().to_string();
        let before1 = fs::metadata(&p1).unwrap().modified().unwrap();
        let before2 = fs::metadata(&p2).unwrap().modified().unwrap();

        assert_eq!(
            run(vec![p1.clone(), p2.clone()], None, true, false, false).unwrap_err(),
            "File checksums do not match! Cancelling operation..."
        );

        let after1 = fs::metadata(&p1).unwrap().modified().unwrap();
        let after2 = fs::metadata(&p2).unwrap().modified().unwrap();
        assert_eq!(before1, after1);
        assert_eq!(before2, after2);
    }

    #[test]
    fn run_updates_three_identical_files() {
        let files = [
            create_file_with_mtime(1000),
            create_file_with_mtime(2000),
            create_file_with_mtime(3000),
        ];
        let paths: Vec<String> = files.iter()
            .map(|f| f.path().to_str().unwrap().to_owned())
            .collect();

        run(paths.clone(), None, true, false, false).unwrap();

        for path in paths {
            assert_eq!(secs(fs::metadata(path).unwrap().modified().unwrap()), 1000);
        }
    }

    #[test]
    fn run_rejects_three_files_with_a_mismatched_middle_file() {
        let files = [
            create_file_with_mtime(1000),
            create_file_with_mtime(2000),
            create_file_with_mtime(3000),
        ];
        fs::write(files[1].path(), b"different").unwrap();
        filetime::set_file_mtime(files[1].path(), FileTime::from_unix_time(2000, 0)).unwrap();
        let paths: Vec<String> = files.iter()
            .map(|f| f.path().to_str().unwrap().to_owned())
            .collect();

        assert_eq!(
            run(paths.clone(), None, true, false, false).unwrap_err(),
            "File checksums do not match! Cancelling operation..."
        );

        for (path, expected) in paths.iter().zip([1000, 2000, 3000]) {
            assert_eq!(secs(fs::metadata(path).unwrap().modified().unwrap()), expected);
        }
    }

    #[test]
    fn run_force_updates_with_mismatch() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(2000);
        fs::write(f2.path(), b"other").unwrap();
        filetime::set_file_mtime(f2.path(), FileTime::from_unix_time(2000, 0)).unwrap();

        let p1 = f1.path().to_str().unwrap().to_string();
        let p2 = f2.path().to_str().unwrap().to_string();

        run(
            vec![p1.clone(), p2.clone()],
            Some("newest".into()),
            true,
            true,
            false,
        ).unwrap();

        let m1 = fs::metadata(&p1).unwrap().modified().unwrap();
        let m2 = fs::metadata(&p2).unwrap().modified().unwrap();
        assert_eq!(secs(m1), 2000);
        assert_eq!(secs(m2), 2000);
    }
}
