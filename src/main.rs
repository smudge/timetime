extern crate data_encoding;
extern crate getopts;
extern crate ring;
extern crate spinner;

use data_encoding::HEXLOWER;
use getopts::Options;
use ring::digest::{Context, Digest, SHA256};
use spinner::{SpinnerBuilder, SpinnerHandle};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

fn print_usage(opts: Options) {
    let args: Vec<String> = env::args().collect();
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("  {}\n", env!("CARGO_PKG_DESCRIPTION"));
    println!("Usage:");
    print!("  {} [opts] file-1 file-2{}", args[0], opts.usage(""));
}

fn main() {
    let mut opts = Options::new();
    opts.optopt("s", "strategy", "oldest (default) or newest", "VAL");
    opts.optflag("t", "tz-safety", "halt if timezones match");
    opts.optflag("m", "mtime", "only compare 'modified' timestamp");
    opts.optflag("h", "help", "this help message");

    let args: Vec<String> = env::args().collect();
    let matches = opts.parse(&args[1..]).unwrap();

    match (
        matches.opt_str("s"),
        matches.opt_str("t"),
        matches.opt_present("h"),
    ) {
        (Some(s), Some(t), false) if Path::new(&s).is_dir() && Path::new(&t).is_dir() => {
            run(Path::new(&s), Path::new(&t))
        }
        (Some(s), Some(_), false) if !Path::new(&s).is_dir() => {
            eprintln!("Unable to find source dir: {}", s)
        }
        (Some(_), Some(t), false) if !Path::new(&t).is_dir() => {
            eprintln!("Unable to find target dir: {}", t)
        }
        (_, _, _) => print_usage(opts),
    }
}

fn run(source: &Path, target: &Path) {
    let mut files_by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let sp = SpinnerBuilder::new("Discovering source files... (0)".into()).start();
    let source_count = count_files(
        source,
        0,
        &sp,
        "Discovering source files...",
        &mut files_by_size,
    )
    .unwrap();
    sp.update("Discovering target files... (0)".into());
    let target_count = count_files(
        target,
        0,
        &sp,
        "Discovering target files...",
        &mut files_by_size,
    )
    .unwrap();
    sp.message(format!(
        "Discovered {} files ({} source, {} target)",
        source_count + target_count,
        source_count,
        target_count
    ));
    let mut files_by_sum: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let count = files_by_size.keys().count();
    for (i, key) in files_by_size.keys().enumerate() {
        let files = files_by_size.get(key).unwrap();
        if files.len() > 1 {
            for path in files {
                sp.update(format!("Hashing files ({}/{})", i, count));
                let file = File::open(path).unwrap();
                let reader = BufReader::new(file);
                let digest = sha256_digest(reader).unwrap();
                let sum = HEXLOWER.encode(digest.as_ref());
                if !files_by_sum.contains_key(&sum) {
                    files_by_sum.insert(sum.clone(), Vec::new());
                }
                let list = files_by_sum.get_mut(&sum).unwrap();
                list.push(path.clone());
            }
        }
    }
    sp.close();
    for key in files_by_sum.keys() {
        let files = files_by_sum.get(key).unwrap();
        if files.len() > 1 {
            println!("{}: {}", key, files.len());
            for file in files {
                println!("↳{}", file.to_string_lossy());
            }
        }
    }
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

fn count_files(
    dir: &Path,
    mut count: u64,
    sp: &SpinnerHandle,
    prefix: &str,
    files_by_size: &mut HashMap<u64, Vec<PathBuf>>,
) -> io::Result<u64> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count = count_files(&path, count, sp, prefix, files_by_size)?;
        } else if path.is_file() {
            let size = fs::metadata(path.clone())?.len();
            if !files_by_size.contains_key(&size) {
                files_by_size.insert(size, Vec::new());
            }
            let list = files_by_size.get_mut(&size).unwrap();
            list.push(path.clone());
            count += 1;
            sp.update(format!("{} ({})", prefix, count));
        }
    }
    Ok(count)
}
