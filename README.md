# File Deduplicator

A safe and fast Rust CLI tool for finding duplicate files using size-based filtering and content hashing.

## Features

- Recursively scan directories
- Detect duplicate files using file size and BLAKE3 hashing
- Estimate potential disk space savings
- Skip unnecessary hashing by grouping files by size first

## Usage

```bash
# Basic scan
cargo run -- scan ./path/to/directory
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
- [ ] Support for `--min-size` filter
- [ ] Support for `--exclude` paths
- [ ] JSON output support
- [ ] Parallel hashing using Rayon
- [ ] Safe cleanup (move or delete)
