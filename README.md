# File Deduplicator

A safe and fast Rust CLI tool for finding duplicate files using size-based filtering and content hashing.

## Features

- Recursively scan directories
- Detect duplicate files using file size and BLAKE3 hashing
- Estimate potential disk space savings
- Skip unnecessary hashing by grouping files by size first
- Parallel processing for high performance
- Progress bar for visual feedback

## Usage

```bash
# Basic scan
cargo run -- scan ./path/to/directory

# Scan with filters
cargo run -- scan ./path/to/directory --min-size 1MB --exclude node_modules

# Output results in JSON
cargo run -- scan ./path/to/directory --json

# Move duplicates to a specific folder
cargo run -- move ./path/to/directory --to ./duplicates_backup

# Delete duplicates (Dry run)
cargo run -- delete ./path/to/directory --dry-run

# Delete duplicates (Confirm)
cargo run -- delete ./path/to/directory --confirm --keep newest
```

## How It Works

1. **Scan**: Recursively list all files in the target directory and collect their sizes.
2. **Size Filter**: Group files by their size. Only files that share the same size are candidates for being duplicates.
3. **Hashing**: Calculate the BLAKE3 hash only for files in groups with more than one file.
4. **Duplicate Detection**: Group files by their hash. Any group with more than one file is a set of duplicates.
5. **Report**: Output a human-readable report showing the duplicates and potential space savings.

## Roadmap

- [x] Basic scanning and duplicate detection
- [x] Size-based pre-filtering
- [x] BLAKE3 hashing
- [x] Human-readable reports
- [x] Support for `--min-size` filter
- [x] Support for `--exclude` paths
- [x] JSON output support
- [x] Parallel hashing using Rayon
- [x] Safe cleanup (move or delete)
- [x] Progress bar for visual feedback
- [ ] Benchmark suite
