# File Deduplicator (dedup)

[![Rust CI](https://github.com/KageRyo/file-deduplicator/actions/workflows/ci.yml/badge.svg)](https://github.com/KageRyo/file-deduplicator/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/file-deduplicator.svg)](https://crates.io/crates/file-deduplicator)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

dedup is a command-line tool for finding duplicate files and either
reviewing, moving, or removing duplicate paths. It groups files by size before
streaming full BLAKE3 hashes, and can emit results as human-readable text or
JSON.

## Features

- Full-content duplicate detection after a size pre-filter.
- File head/tail partial hashing avoids full-file reads for candidates that
  clearly differ, while full BLAKE3 remains required for duplicates.
- Configurable hashing worker count with `--threads` on scan and cleanup
  commands.
- Deterministic first keep policy: the lexicographically smallest path in a
  duplicate group is kept.
- Glob-based exclusions such as target/**, **/.git/**, and *.tmp.
- Persistent partial and full hash cache for repeated scans, with cache
  statistics and an explicit `--no-cache` bypass.
- Configurable traversal depth and same-filesystem scanning.
- Versioned JSON scan output with scan and hashing error details.
- A move --dry-run preview that does not create directories or move files.
- Revalidation of file metadata and full BLAKE3 hashes before moving or
  deleting files.
- A `trash` command that uses the operating system recycle bin/trash.
- Shell completion generation for Bash, Zsh, Fish, PowerShell, and Elvish.
- Tagged GitHub Releases with prebuilt Linux, Windows, and macOS binaries.
- Configurable keep policies for move, trash, and delete operations.
- Hard-link paths to the same physical file are not counted as reclaimable
  duplicate storage.
- Symlinks are not followed or scanned; dedup only processes regular files.

dedup uses ordinary filesystem removal. It does not provide secure erasure,
which cannot be guaranteed by a normal remove_file operation.

## Installation

Install the published crate with Cargo:

~~~bash
cargo install file-deduplicator
~~~

The installed command is dedup:

~~~bash
dedup --version
~~~

Prebuilt archives for Linux x86_64, Windows x86_64, macOS Intel, and macOS
Apple Silicon are published on the
[GitHub Releases page](https://github.com/KageRyo/file-deduplicator/releases).
Each archive includes a SHA-256 checksum file.

## Usage

### Scan

~~~bash
# Scan a directory
dedup scan ./Downloads

# Apply a minimum size and path exclusions
dedup scan ./project \
  --min-size 1MB \
  --exclude 'target/**' \
  --exclude '**/.git/**' \
  --exclude '*.tmp'

# Write machine-readable output
dedup scan ./images --json > report.json

# Limit hashing to four worker threads
dedup scan ./Downloads --threads 4

# Scan only the root and its direct children, without crossing filesystems
dedup scan ./Downloads --max-depth 1 --one-file-system

# Bypass the persistent hash cache for one scan
dedup scan ./Downloads --no-cache
~~~

With --json, standard output contains only the JSON document. Progress and
human-readable diagnostics are not mixed into that output. The JSON summary
includes successfully scanned files, skipped files, scan failures, hash
candidates, and hash failures. The top-level `schema_version` is `1`, and
`application_version` identifies the installed dedup version. The `cache`
object reports partial/full hits and misses plus invalidated and pruned
entries.

Schema version 1 permits additive fields that do not change the meaning or
type of existing fields. Consumers should ignore unknown fields. Renaming,
removing, or changing the type or meaning of an existing field requires a new
schema version. Scan and hash error arrays remain available in every version
that supports the current scan output contract.

Exclusion patterns use / separators on every platform and are matched
relative to the scan root. Patterns containing / match the relative path;
patterns without / also match individual path components, so *.tmp, .git, and
target work at any depth. Exclusions are globs, not substring matches.

Files with the same size are first compared using a BLAKE3 hash of their first
and last 64 KiB. Only files with matching partial hashes are read completely,
and full BLAKE3 hashes are always used before reporting duplicates. Omit
`--threads` to use Rayon’s default worker count; when supplied it must be at
least 1.

The hash cache is enabled by default. On Unix, it is stored at
`$XDG_CACHE_HOME/file-deduplicator/hashes-v1.json`, or
`$HOME/.cache/file-deduplicator/hashes-v1.json` when `XDG_CACHE_HOME` is not
set. On Windows, it is stored below `%LOCALAPPDATA%/file-deduplicator`.
`DEDUP_CACHE_DIR` overrides the parent cache directory on any platform. Cache
entries include the stable file path, physical file identity, size, and
modification time. A changed or replaced file invalidates its entry. Deleted
paths are pruned, and a moved file is treated as a cache miss at its new path;
duplicate correctness still requires a full BLAKE3 hash. A malformed cache is
discarded and rebuilt without preventing the scan from completing.

The scan root is depth 0. `--max-depth 0` therefore scans only a file supplied
as the root, while `--max-depth 1` includes files directly inside a directory
but does not descend into nested directories. Omitting the option keeps the
existing unlimited traversal behavior. `--one-file-system` prevents descent
into another filesystem on Unix and Windows. On platforms where WalkDir cannot
provide a reliable filesystem identity, requesting this option reports a scan
error instead of silently crossing a boundary. These controls compose with
exclusions, minimum-size filtering, JSON output, and cleanup commands.

### Move

Preview moves before changing the filesystem:

~~~bash
dedup move ./Downloads --to ./duplicates_backup --dry-run
~~~

Run the move after reviewing the source-to-destination list:

~~~bash
dedup move ./Downloads --to ./duplicates_backup
~~~

By default, move keeps the lexicographically smallest path. Use the same keep
policies as delete when a different file should remain in place:

~~~bash
dedup move ./Downloads --to ./duplicates_backup --keep newest
dedup move ./Downloads --to ./duplicates_backup --keep oldest
~~~

dedup first tries a filesystem rename. If the destination is on another
filesystem, it copies the file, verifies the copied content, revalidates the
source, and removes the source only after those checks succeed. Destination
collisions preserve the extension (photo.jpg becomes photo_1.jpg).

### Trash

Preview moving duplicates to the operating system trash or recycle bin:

~~~bash
dedup trash ./Downloads --dry-run
~~~

Move them after reviewing the preview:

~~~bash
dedup trash ./Downloads --confirm --keep first
~~~

The command uses the platform trash mechanism instead of permanently removing
files. If the platform does not provide a usable trash implementation, the
command fails before reporting success. Revalidation and keep policies work the
same way as for `move` and `delete`.

### Delete

Preview deletion:

~~~bash
dedup delete ./Downloads --dry-run
~~~

Delete only after reviewing the preview:

~~~bash
dedup delete ./Downloads --confirm --keep first
dedup delete ./Downloads --confirm --keep newest
dedup delete ./Downloads --confirm --keep oldest
~~~

The first policy always keeps the lexicographically smallest path. Before
any destructive operation, every file in the duplicate group is checked
against the original metadata and full BLAKE3 hash. If a file changed or a
scan/hash error made the result incomplete, the affected operation is skipped
or the destructive command refuses to start.

### Filesystem behavior

Only regular files are scanned. Symlinks are not followed, including symlinks
to directories, and symlink paths are not hashed or moved/deleted. A target
file is scanned only when it is reached through a regular path. Hard-link
aliases are detected by physical file identity and are not counted as
reclaimable duplicate storage.

## Exit codes

For valid commands, dedup uses these stable statuses:

- `0`: the scan completed, or cleanup completed without skipped files.
- `1`: the command failed before it could produce a complete result.
- `2`: the scan or hashing result was incomplete. Scan reports are still
  emitted; destructive commands refuse to start.
- `3`: cleanup was partial because one or more files failed revalidation. Those
  files are left unchanged.

When `scan --json` returns `2`, standard output remains a JSON document and
contains the scan and hash error details.

## Shell completions

Generate a completion script to standard output and redirect it to the shell’s
completion directory. The command does not modify user files itself:

~~~bash
dedup completions bash > ~/.local/share/bash-completion/completions/dedup
dedup completions zsh > ~/.zfunc/_dedup
dedup completions fish > ~/.config/fish/completions/dedup.fish
dedup completions powershell > dedup.ps1
dedup completions elvish > dedup.elv
~~~

The generated PowerShell script can be dot-sourced from the PowerShell profile.
Use `dedup completions --help` to see the supported shell names.

## Building from source

The package uses Rust edition 2024 and has an MSRV of Rust 1.85.0. To build
from a checkout:

~~~bash
git clone https://github.com/KageRyo/file-deduplicator.git
cd file-deduplicator
cargo build --release
./target/release/dedup --version
~~~

For contributor checks and release validation, see
[CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/RELEASING.md](docs/RELEASING.md).

## Benchmarks

Run the Criterion suite with:

~~~bash
cargo bench --bench scan_bench
~~~

The suite covers many small files, large files, mostly unique files, many
same-sized files, and high duplicate ratios. It reports reproducible relative
measurements for the duplicate-detection pipeline, including cold and warm
hash-cache scans; results depend on the machine and filesystem.

## Rust API

File Deduplicator is currently distributed as a CLI-only crate. It does not
publish a Rust library target or promise a stable library API; the
implementation modules are internal to the dedup executable.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
