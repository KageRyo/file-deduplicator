# File Deduplicator (dedup)

[![Rust CI](https://github.com/KageRyo/file-deduplicator/actions/workflows/ci.yml/badge.svg)](https://github.com/KageRyo/file-deduplicator/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance, safe, and modern Rust CLI tool for finding and managing duplicate files. Designed for speed using size-based pre-filtering and parallel BLAKE3 hashing.

## 🚀 Key Features

- **Blazing Fast**: Uses parallel processing (`Rayon`) and the cryptographic-grade `BLAKE3` hash for maximum throughput.
- **Smart Filtering**: Avoids unnecessary hashing by grouping files by size first.
- **Advanced CLI**: Filter by minimum size, exclude paths/patterns, and output results in human-readable or JSON formats.
- **Safe Cleanup**: 
    - `move`: Archive duplicates to a backup folder.
    - `delete`: Securely remove files with `dry-run` and `confirm` safety checks.
- **User-Friendly**: Interactive progress bars and detailed space-saving estimations.

## 📦 Installation

```bash
git clone https://github.com/KageRyo/file-deduplicator.git
cd file-deduplicator
cargo install --path .
```

## 🛠 Usage

### 🔍 Scanning
```bash
# Basic scan of a directory
dedup scan ./Downloads

# Advanced scan with filters
dedup scan ./project --min-size 1MB --exclude target --exclude .git

# Export to JSON for post-processing
dedup scan ./images --json > report.json
```

### 📁 Management
```bash
# Move duplicates to a backup directory
dedup move ./Downloads --to ./duplicates_backup

# Delete duplicates with a specific policy (first | newest | oldest)
dedup delete ./Downloads --dry-run
dedup delete ./Downloads --confirm --keep newest
```

## ⚙️ How It Works (Architecture)

1. **Scanner Phase**: Recursively traverses directories using `WalkDir`, collecting file metadata (paths and sizes).
2. **Size Filter**: Groups files by their exact byte count. Only size-identical files are considered candidates.
3. **Parallel Hashing**: Hashing is performed in parallel across available CPU cores. Small segments of the file are streamed into the `BLAKE3` hasher to keep memory usage low.
4. **Duplicate Grouping**: Files with matching size AND hash are grouped.
5. **Reporter**: Calculates potential disk space savings and presents the results.

## 📊 Performance

By combining size-based pre-filtering with parallel BLAKE3, `dedup` significantly outperforms naive "hash-everything" tools, especially on directories with large media files.

| Strategy | Speed | Resource Usage |
| :--- | :--- | :--- |
| **Naive Hash-All** | Slow (I/O bound) | High Disk I/O |
| **Size-First + BLAKE3** | **Ultra Fast** | **Optimized I/O** |

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---
Built with ❤️ by [KageRyo](https://github.com/KageRyo)
