extern crate getopts;

use getopts::Options;
use std::env;

fn print_usage(opts: Options) {
    let args: Vec<String> = env::args().collect();
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("  {}\n", env!("CARGO_PKG_DESCRIPTION"));
    println!("Usage:");
    print!("  {} -s SOURCE -t TARGET{}", args[0], opts.usage(""));
}

fn main() {
    let mut opts = Options::new();
    opts.optopt(
        "t",
        "target",
        "target directory (where files will go)",
        "TARGET",
    );
    opts.optopt("s", "source", "source directory (for deletion!)", "SOURCE");
    opts.optflag("h", "help", "this help message");

    let args: Vec<String> = env::args().collect();
    let matches = opts.parse(&args[1..]).unwrap();

    match (
        matches.opt_str("s"),
        matches.opt_str("t"),
        matches.opt_present("h"),
    ) {
        (Some(source), Some(target), false) => run(source, target),
        (_, _, _) => print_usage(opts),
    }
}

fn run(source: String, target: String) {
    println!("source: {}, target: {}", source, target);
}
