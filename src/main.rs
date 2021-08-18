extern crate getopts;
extern crate spinner;

use getopts::Options;
use spinner::{SpinnerBuilder, SpinnerHandle};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn print_usage(opts: Options) {
    let args: Vec<String> = env::args().collect();
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("  {}\n", env!("CARGO_PKG_DESCRIPTION"));
    println!("Usage:");
    print!("  {} -s SOURCE -t TARGET{}", args[0], opts.usage(""));
}

fn main() {
    let mut opts = Options::new();
    opts.optopt("t", "target", "target directory", "TARGET");
    opts.optopt("s", "source", "source directory (for deletion!)", "SOURCE");
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
    );
    sp.update("Discovering target files... (0)".into());
    let target_count = count_files(
        target,
        0,
        &sp,
        "Discovering target files...",
        &mut files_by_size,
    );
    sp.update(" ".into());
    sp.close();
    println!(
        "source: {}, target: {}",
        source_count.unwrap(),
        target_count.unwrap()
    );
    println!("Files by size:");
    for key in files_by_size.keys() {
        let count = files_by_size.get(key).unwrap().len();
        if count > 1 {
            println!("{}: {}", key, count);
        }
    }
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
        }
    }
    sp.update(format!("{} ({})", prefix, count));
    Ok(count)
}
