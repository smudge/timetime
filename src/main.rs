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
    ) {
        (_, _, _, _, _, _, true) => print_usage(opts),
        (_, None, _, _, _, _, false) => {
            eprintln!("At least two file paths are required");
            std::process::exit(2);
        }
        (Some(_), Some(_), Some(ref strategy), _, _, _, false)
            if !matches!(strategy.as_str(), "oldest" | "newest") =>
        {
            eprintln!("Invalid strategy '{}': expected oldest or newest", strategy);
            std::process::exit(2);
        }
        (Some(_), Some(_), s, m, t, f, false) => {
            if let Err(error) = run(matches.free, s, m, f, t) {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        },
        (_, _, _, _, _, _, _) => print_usage(opts),
    }
}

fn run(files: Vec<String>, strategy: Option<String>, mtime: bool, force: bool, tzsafety: bool) -> Result<(), String> {
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
    let modified = get_canonical_time(&metadata, &paths, tzsafety, strategy.as_deref(), |m| m.modified())?;
    if !mtime {
        let created = get_canonical_time(&metadata, &paths, tzsafety, strategy.as_deref(), |m| m.created())?;
        if created > modified {
            return Err("Canonical creation time cannot be after modified time".into());
        }
        for path in &paths {
            filetime::set_file_mtime(path, created)
                .map_err(|e| format!("Cannot set timestamp for '{}': {}", path.display(), e))?;
        }
    }

    for path in &paths {
        filetime::set_file_mtime(path, modified)
            .map_err(|e| format!("Cannot set timestamp for '{}': {}", path.display(), e))?;
    }
    Ok(())
}

fn get_canonical_time<F: Fn(&Metadata) -> io::Result<SystemTime>>(
    metadata: &[Metadata],
    paths: &[PathBuf],
    tzsafety: bool,
    strategy: Option<&str>,
    f: F,
) -> Result<FileTime, String> {
    let times = metadata
        .iter()
        .zip(paths)
        .map(|(m, path)| {
            f(m).map_err(|e| format!("Cannot read timestamp for '{}': {}", path.display(), e))
        })
        .collect::<Result<Vec<SystemTime>, _>>()?;

    if tz_check(&times) {
        if tzsafety {
            return Err("Timezone Safety Check Failed! Files may have matching timestamps from different timezones.".into());
        }
        eprintln!("Warning: files may have matching timestamps from different timezones");
    }

    match strategy {
        Some("newest") => times.iter().max(),
        _ => times.iter().min(),
    }
    .copied()
    .map(FileTime::from_system_time)
    .ok_or_else(|| "No file timestamps were available".to_string())
}

fn tz_check(times: &Vec<SystemTime>) -> bool {
    let mut times = times.clone();
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
    fn canonical_time_oldest_and_newest() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(2000);
        let meta = vec![
            fs::metadata(f1.path()).unwrap(),
            fs::metadata(f2.path()).unwrap(),
        ];

        let paths = vec![f1.path().to_path_buf(), f2.path().to_path_buf()];
        let oldest = get_canonical_time(&meta, &paths, false, None, |m| m.modified()).unwrap();
        assert_eq!(oldest.unix_seconds(), 1000);

        let newest = get_canonical_time(&meta, &paths, false, Some("newest"), |m| m.modified()).unwrap();
        assert_eq!(newest.unix_seconds(), 2000);
    }

    #[test]
    fn canonical_time_tz_safety_returns_error() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(4600); // exactly one hour later
        let meta = vec![
            fs::metadata(f1.path()).unwrap(),
            fs::metadata(f2.path()).unwrap(),
        ];
        let paths = vec![f1.path().to_path_buf(), f2.path().to_path_buf()];
        let error = get_canonical_time(&meta, &paths, true, None, |m| m.modified()).unwrap_err();
        assert!(error.contains("Timezone Safety Check Failed"));
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
