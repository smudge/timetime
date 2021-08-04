extern crate getopts;

use getopts::Options;
use std::env;
use std::fs;
use std::io;
use std::path::Path;

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
    let source_count = count_files(source, 0);
    let target_count = count_files(target, 0);
    println!("source: {}", source_count.unwrap());
    println!("target: {}", target_count.unwrap());
}

fn count_files(dir: &Path, mut count: u64) -> io::Result<u64> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count = count_files(&path, count)?;
        } else if path.is_file() {
            count += 1;
        }
    }
    Ok(count)
}
