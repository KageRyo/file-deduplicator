# Releasing File Deduplicator

This document separates the one-time v0.1.0 crates.io publish from later
automated releases.

## Validate a package

Run these checks from a clean checkout of the release commit:

~~~bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo package --list
cargo publish --dry-run
~~~

Review the package list. It should contain the manifest, lockfile, README,
license, and source files only; repository fixtures, tests, benchmarks,
temporary files, build output, and local configuration must not be included in
the published crate.

## v0.1.0: first publish (manual)

v0.1.0 is the first publication of this crate. After the release-readiness
pull request has been reviewed and merged:

1. Confirm that the package name is file-deduplicator, the version is 0.1.0,
   and the package validation checks are green.
2. Authenticate to crates.io through the maintainer's out-of-band local
   publishing setup. No registry credential belongs in this repository or its
   GitHub Actions workflows.
3. Run cargo publish from the release commit after cargo publish --dry-run
   succeeds.
4. Verify the crate page and a clean cargo install file-deduplicator
   installation.

The first publish is intentionally manual. It does not require a v0.1.0 Git
tag or a GitHub Release, and the automated publish workflow explicitly skips
v0.1.0.

## Configure Trusted Publishing after the first publish

After v0.1.0 is available on crates.io, configure crates.io Trusted
Publishing for the KageRyo/file-deduplicator repository and the publish
workflow. Complete that setup in the crates.io project settings before
attempting an automated release. The workflow uses GitHub Actions OIDC and a
short-lived publish credential; it does not use a long-lived registry token
stored as a repository secret.

The GitHub Actions publish job uses the `crates.io` environment and links it to
the crate page at https://crates.io/crates/file-deduplicator. Configure the
crates.io Trusted Publisher with the same optional environment name,
`crates.io`, so the OIDC publisher restriction matches the workflow.

If Trusted Publishing has not been configured, do not push a release tag
expecting the workflow to publish. Use the documented manual process instead.

## Later releases

For a later version:

1. Update the Cargo package version and lockfile as needed.
2. Run the complete validation checklist and merge the release pull request.
3. Confirm Trusted Publishing is configured for the repository and workflow.
4. Create and push a version tag such as v0.2.1 from the merged commit.
5. Review the publish workflow result, the resulting crates.io package, and
   the GitHub Release created by the follow-up release job.

The publish workflow also builds and attaches versioned archives for:

- Linux x86_64 (`x86_64-unknown-linux-gnu`)
- Windows x86_64 (`x86_64-pc-windows-msvc`)
- macOS Intel (`x86_64-apple-darwin`)
- macOS Apple Silicon (`aarch64-apple-darwin`)

Each archive contains the `dedup` executable and has a matching SHA-256
checksum asset. The release is created only after both crates.io publishing
and all binary builds succeed.

The workflow has an explicit v0.1.0 guard so that an accidental v0.1.0 tag
cannot trigger a publish attempt before Trusted Publishing was configured.
For later tags, the GitHub Release job runs only after the crates.io publish
job succeeds. Its title is formatted as `file-deduplicator vX.Y.Z`, while the
Git tag remains `vX.Y.Z` for Cargo and workflow version validation.
