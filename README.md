# timetime

> A CLI for repairing timestamp metadata of file copies 🕛🕧🕐🕜🕑🕝🕒

## Basic Usage

When given two or more files, `timetime` will cross-check their
created/modified timestamps and 💅 repair ✨ each file's timestamp metadata to
match whichever values are the earliest:

```bash
timetime my-file.txt backups/my-file.txt
```

By default, **🚨the files must have identical checksums🚨**, because it assumes
that they are copies. If not, a warning will be produced and the operation will
fail. This can be circumvented with the `--force` or `-f` option:

```bash
timetime --force file-1 file-2 file-3
```

⚠️Use with caution⚠️, as this will blindly overwrite any number of timestamps!

#### "Modified" vs "Created"

By default, `timetime` will compare both `mtime` ("modified at") and, on
supported systems, `btime` ("created at").

To compare only the "modified" timestamp, use the `--mtime` or `-m` flag:

```bash
timetime --mtime file-1 file-2
```

_Note that some filesystems will prevent a file's "created at" from being after
its "modified at" by automatically setting `btime` to match `mtime`._

#### Comparison strategy

When supplied with multiple files, `timetime` can be configured to use the
newest timestamps, rather than the oldest:

```bash
timetime --strategy newest file-1 file-2
```

This can be useful in situations where a set of files are known to be in the
wrong timezone, or to have had their timestamp metadata damaged in some other
way.

#### Timezone safety

As a special feature, `timetime` will detect if the files' timestamps are off
by an _exact_ number of hours. If this is the case, it will output a warning:

```
Warning: file1 and file2 may have matching timestamps from different timezones
```

Due to the risk of false positives, this warning will not halt execution,
unless an extra `--tz-safety` parameter is supplied:

```bash
timetime file-1 file-2 --tz-safety
```

### Usage with `rmlint`

The `timetime` command is intended to be used as part of a more complex file
consolidation and/or deletion script, such as one produced by `rmlint`.

For example, prior to removing a duplicate file, it may make sense to "correct"
the timestamps of the files, to preserve the oldest modified/created
timestamps. This can be done by supplying a custom removal command to `rmlint`:

```bash
rmlint -o sh -c sh:cmd='timetime --tz-safety "$1" "$2" && rm "$1"' --keep-all-tagged --must-match-tagged -T df /media/backup // /media/original
```

The above command will detect files in `/media/backup` that are identical to
files in `/media/original`, then "repair" timestamps across both the backup and
the original, _and then_ (only if that succeeds) remove the duplicate file.

**As always, be sure to test operations carefully before removing files en masse!**

## Installing

### via Homebrew

```
brew install smudge/smudge/timetime
```

### via Cargo

[Set up Rust/Cargo](https://doc.rust-lang.org/book/ch01-01-installation.html)
and install from crates.io by running:

```
cargo install timetime
```

## Thanks To:

* The maintainers of `rmlint` for such a reliable and well-documented project
* Carol Nichols and Steve Klabnik for the [official book](https://doc.rust-lang.org/book/)

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
