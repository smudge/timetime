# bakmerge

> A CLI for merging multiple backups into a single, consolidated folder.

If you find yourself with a bunch of disorganized backups, with files
duplicated many times over, `bakmerge` can help you merge them all into one
place.

**WARNING: This is an experimental tool that _will_ delete files.** Use at your
own risk!

### Why?

I made `bakmerge` to help me merge many years of external hard drive backups
onto a single, network-attached storage device with a newly-organized folder
structure. In other words, I had folder after folder with names like
`mac_backup_3/` and `desktop 1-1-2015/`, and I wanted to de-duplicate and
re-organize all files contained within, so that I could delete the backups and
get rid of the hard drives. (My NAS manages its own backups to cloud storage!)

In order to do this, I needed to be sure that I wasn't deleting anything that
wasn't already on my storage server. So I made sure that `bakmerge` is able
to:

- Detect similar/duplicate files/folders, recursively.
- Manage file timestamps sensibly and uncover the most truthful values.
- Compare folder structures and detect missing or corrupt files.
- Store alternate versions of files as smaller, delta/patch files.
- Surface interactive prompts whenever a choice is ambiguous.

## How it Works

Bakmerge assumes that you have one or more source folders, and that your aim is
to merge them all into a single target folder hierarchy.

It is recommended that you first set up your "target" folder by moving one copy
of as many files as you can find into your desired organization system. An
empty "target" is of little use, but a fully organized "target" will allow you
to quickly clear out your source folders! Your "target" could even be your most
recent backup!

The goal will be to detect and delete files from the source folders that are
known to exist in the target. You'll also have an opportunity to consolidate
alternate versions of files into an adjacent `.versions` folder. Plus, if
duplicate files have inconsistent timestamps but are otherwise 100% identical,
you'll have the option to apply the earliest "created at" timestamp to the copy
in your target folder.

In the end, your "source" folders should be left containing only files that
do not (yet) exist in "target" and can be manually incorporated if desired.

## Usage

Running the `bakmerge` command will begin an interactive session:

```bash
$ bakmerge -t /path/to/target_folder -s /path/to/source_folders
```

Initially, `bakmerge` will build a database of files and checksums. **This may
take a long time**, but is fully resume-able and need only be incrementally
updated once performed once. By default, the database is stored in your
home/user directory under `.bakmerge/`.

After all files are processed, `backmerge` will prompt you with choices.

- When a duplicate file (or _an entire folder consisting of duplicate files_)
  is detected, you may:
  - Press `D` or `K` to either (D)elete or (K)eep the source file(s).
  - Press `E` to (E)nter the folder and choose on a per-file basis.
- When a duplicate source file has an older "created at" timestamp than the
  target copy, you may:
  - Press `W` or `K` to either (W)rite the older timestamp to the
    target, or (K)eep the existing timestamp.
- When a similar file is detected (similar content and/or similar containing
  folder and name):
  - Press `W` or `S` to either (W)rite a patch file to a `.versions` folder in
    the target, or (S)kip the file.

## Installing

### via Homebrew

```
brew install smudge/smudge/bakmerge
```

### via Cargo

[Set up Rust/Cargo](https://doc.rust-lang.org/book/ch01-01-installation.html)
and install from crates.io by running:

```
cargo install bakmerge
```

## Thanks To:

* Carol Nichols and Steve Klabnik for the [official book](https://doc.rust-lang.org/book/) on Rust

## Contributing

* Check the issue tracker and consider creating a new issue.
* Fork the project and create a new branch for your contribution.
* Write, commit, and push your contribution to your branch.
* Make sure the project builds (`cargo build`) and functionality still works as expected.
* Submit a pull request.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
