# Contributing to File Deduplicator

Thank you for helping improve File Deduplicator. Please open an issue for
larger changes before starting implementation so the intended behavior can be
discussed openly.

## Development setup

The package uses Rust edition 2024 and supports Rust 1.85.0 or newer. Install
the stable toolchain, then run the CLI from the repository with Cargo.

Before opening a pull request, run:

~~~bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
~~~

The package and release checks are:

~~~bash
cargo package --list
cargo publish --dry-run
~~~

These commands validate the package without publishing it. Do not put
registry credentials in the repository, source files, workflow files, issue
comments, or pull requests.

## Dependency security audit

The CI security job checks Cargo.lock against RustSec advisories. To run the
same check locally, install cargo-audit once and then run:

~~~bash
cargo install cargo-audit --locked
cargo audit
~~~

An advisory failure is intentional CI failure and should be investigated
before merging. If an advisory is not immediately actionable, document the
reason and remediation plan in the relevant issue or pull request.

## GitHub Flow

Changes are developed on a branch based on main and merged through a pull
request:

1. Create a focused branch such as feature/name, fix/name, docs/name, or
   chore/name.
2. Add or update tests for behavior changes.
3. Keep commits focused and use
   [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/),
   for example fix: reject incomplete scans.
4. Push the branch and open a pull request targeting main.
5. Wait for the Linux, Windows, and macOS checks before requesting review.

Do not merge your own pull request unless the project maintainers ask you to
do so.

## Filesystem safety

Tests for move and delete behavior must use temporary directories created by
the test itself. They must not operate on repository files or paths outside
their temporary test directory. Add regression coverage when changing
revalidation, hard-link handling, collision naming, exclusions, or
cross-filesystem move behavior.

## License

By contributing, you agree that your contributions will be licensed under the
MIT License.
