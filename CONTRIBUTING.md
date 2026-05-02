# Contributing to File Deduplicator

First off, thank you for considering contributing to File Deduplicator! It's people like you that make the open-source community such an amazing place.

## How Can I Contribute?

### Reporting Bugs

- Check the [Issues tab](https://github.com/KageRyo/file-deduplicator/issues) to see if the bug has already been reported.
- If not, open a new issue. Include a clear title, a description of the problem, and steps to reproduce it.

### Suggesting Enhancements

- Open a new issue with the tag "enhancement".
- Describe the feature you'd like to see and why it would be useful.

### Pull Requests

1. **Fork the repo** and create your branch from `main`.
2. **Install dependencies**.
3. **Write tests** for your changes.
4. **Ensure CI passes** locally by running:
   ```bash
   cargo fmt --all -- --check
   cargo clippy -- -D warnings
   cargo test
   ```
5. **Format your commit messages** using [Conventional Commits](https://www.conventionalcommits.org/).

## Development Setup

- Ensure you have the latest stable Rust toolchain installed.
- To run benchmarks: `cargo bench`.

## Style Guide

- We follow the standard Rust style. Run `cargo fmt` before committing.
- Document public functions and modules where appropriate.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
