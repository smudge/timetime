extern crate getopts;

use getopts::Options;
use std::env;
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
        (Some(s), Some(t), false) if Path::new(&s).exists() && Path::new(&t).exists() => {
            run(Path::new(&s), Path::new(&t))
        }
        (Some(s), Some(_), false) if !Path::new(&s).exists() => {
            eprintln!("Unable to find source dir: {}", s)
        }
        (Some(_), Some(t), false) if !Path::new(&t).exists() => {
            eprintln!("Unable to find target dir: {}", t)
        }
        (_, _, _) => print_usage(opts),
    }
}

fn run(source: &Path, target: &Path) {
    println!(
        "source: {}, target: {}",
        source.to_str().unwrap(),
        target.to_str().unwrap()
    );
}
