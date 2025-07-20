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
        (Some(_), Some(_), s, m, t, f, false) => run(matches.free, s, m, f, t),
        (_, _, _, _, _, _, _) => print_usage(opts),
    }
}

fn run(files: Vec<String>, strategy: Option<String>, mtime: bool, force: bool, tzsafety: bool) {
    let paths: Vec<PathBuf> = files.iter().map(std::path::PathBuf::from).collect();
    let mut checksums: Vec<String> = paths.iter().map(|path| checksum(path.as_path())).collect();
    checksums.dedup();
    if checksums.len() != 1 && !force {
        eprintln!("File checksums do not match! Cancelling operation...");
    } else {
        let metadata: Vec<Metadata> = paths
            .iter()
            .map(|path| fs::metadata(path).unwrap())
            .collect();
        let modified = get_canonical_time(&metadata, tzsafety, strategy.clone(), |m| m.modified());
        if !mtime {
            let created = get_canonical_time(&metadata, tzsafety, strategy, |m| m.created());
            if created > modified {
                panic!("Canonical creation time cannot be after modified time");
            }
            for path in paths.iter() {
                filetime::set_file_mtime(&path, created).unwrap(); // Setting mtime to created will update btime so that btime <= mtime
            }
        }

        for path in paths.iter() {
            filetime::set_file_mtime(&path, modified).unwrap();
        }
    }
}

fn get_canonical_time<F: Fn(&Metadata) -> io::Result<SystemTime>>(
    metadata: &Vec<Metadata>,
    tzsafety: bool,
    strategy: Option<String>,
    f: F,
) -> FileTime {
    let times = metadata
        .iter()
        .map(|m| f(m).unwrap())
        .collect::<Vec<SystemTime>>();
    if tz_check(&times) {
        if tzsafety {
            panic!("Timezone Safety Check Failed! Files may have matching timestamps from different timezones.")
        } else {
            eprintln!("Warning: files may have matching timestamps from different timezones");
        }
    }
    match strategy {
        Some(w) if w == "newest".to_string() => {
            FileTime::from_system_time(times.iter().max().unwrap().clone())
        }
        _ => FileTime::from_system_time(times.iter().min().unwrap().clone()),
    }
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

fn checksum(path: &Path) -> String {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let digest = sha256_digest(reader).unwrap();
    HEXLOWER.encode(digest.as_ref())
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
            checksum(file.path()),
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
    fn tz_check_non_hour_delta() {
        let base = SystemTime::UNIX_EPOCH;
        let times = vec![base, base + Duration::from_secs(3599)];
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

        let oldest = get_canonical_time(&meta, false, None, |m| m.modified());
        assert_eq!(oldest.unix_seconds(), 1000);

        let newest = get_canonical_time(&meta, false, Some("newest".into()), |m| m.modified());
        assert_eq!(newest.unix_seconds(), 2000);
    }

    #[test]
    #[should_panic]
    fn canonical_time_tz_safety_panics() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(4600); // exactly one hour later
        let meta = vec![
            fs::metadata(f1.path()).unwrap(),
            fs::metadata(f2.path()).unwrap(),
        ];
        get_canonical_time(&meta, true, None, |m| m.modified());
    }

    #[test]
    fn run_updates_to_oldest() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(2000);
        let p1 = f1.path().to_str().unwrap().to_string();
        let p2 = f2.path().to_str().unwrap().to_string();

        run(vec![p1.clone(), p2.clone()], None, true, false, false);

        let m1 = fs::metadata(&p1).unwrap().modified().unwrap();
        let m2 = fs::metadata(&p2).unwrap().modified().unwrap();
        assert_eq!(secs(m1), 1000);
        assert_eq!(secs(m1), secs(m2));
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
        let before = fs::metadata(&p1).unwrap().modified().unwrap();

        run(vec![p1.clone(), p2.clone()], None, true, false, false);

        let after = fs::metadata(&p1).unwrap().modified().unwrap();
        assert_eq!(secs(before), secs(after));
    }

    #[test]
    fn run_force_updates_with_mismatch() {
        let f1 = create_file_with_mtime(1000);
        let f2 = create_file_with_mtime(2000);
        fs::write(f2.path(), b"other").unwrap();

        let p1 = f1.path().to_str().unwrap().to_string();
        let p2 = f2.path().to_str().unwrap().to_string();

        run(
            vec![p1.clone(), p2.clone()],
            Some("newest".into()),
            true,
            true,
            false,
        );

        let m1 = fs::metadata(&p1).unwrap().modified().unwrap();
        let m2 = fs::metadata(&p2).unwrap().modified().unwrap();
        assert_eq!(secs(m1), secs(m2));
        assert!(secs(m1) >= 2000);
    }
}
