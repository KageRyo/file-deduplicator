# File Deduplicator (dedup)

[![Rust CI](https://github.com/KageRyo/file-deduplicator/actions/workflows/ci.yml/badge.svg)](https://github.com/KageRyo/file-deduplicator/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

dedup is a command-line tool for finding duplicate files and either
reviewing, moving, or removing duplicate paths. It groups files by size before
streaming full BLAKE3 hashes, and can emit results as human-readable text or
JSON.

## Features

- Full-content duplicate detection after a size pre-filter.
- Deterministic first keep policy: the lexicographically smallest path in a
  duplicate group is kept.
- Glob-based exclusions such as target/**, **/.git/**, and *.tmp.
- JSON scan output with scan and hashing error details.
- A move --dry-run preview that does not create directories or move files.
- Revalidation of file metadata and full BLAKE3 hashes before moving or
  deleting files.
- Hard-link paths to the same physical file are not counted as reclaimable
  duplicate storage.

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
~~~

With --json, standard output contains only the JSON document. Progress and
human-readable diagnostics are not mixed into that output. The JSON summary
includes successfully scanned files, skipped files, scan failures, hash
candidates, and hash failures.

Exclusion patterns use / separators on every platform and are matched
relative to the scan root. Patterns containing / match the relative path;
patterns without / also match individual path components, so *.tmp, .git, and
target work at any depth. Exclusions are globs, not substring matches.

### Move

Preview moves before changing the filesystem:

~~~bash
dedup move ./Downloads --to ./duplicates_backup --dry-run
~~~

Run the move after reviewing the source-to-destination list:

~~~bash
dedup move ./Downloads --to ./duplicates_backup
~~~

dedup first tries a filesystem rename. If the destination is on another
filesystem, it copies the file, verifies the copied content, revalidates the
source, and removes the source only after those checks succeed. Destination
collisions preserve the extension (photo.jpg becomes photo_1.jpg).

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

## Rust API

v0.1.0 is intentionally a CLI-only package. It does not publish a Rust
library target or promise a stable library API; the implementation modules are
internal to the dedup executable.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
