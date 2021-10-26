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
        if !mtime {
            let min_created = get_min_time(&metadata, tzsafety, |m| m.created());
            for path in paths.iter() {
                filetime::set_file_mtime(&path, min_created).unwrap(); // Setting mtime to created will update btime so that btime <= mtime
            }
        }

        let min_modified = get_min_time(&metadata, tzsafety, |m| m.modified());
        for path in paths.iter() {
            filetime::set_file_mtime(&path, min_modified).unwrap();
        }
    }
}

fn get_min_time<F: Fn(&Metadata) -> io::Result<SystemTime>>(
    metadata: &Vec<Metadata>,
    tzsafety: bool,
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
    FileTime::from_system_time(times.iter().min().unwrap().clone())
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
