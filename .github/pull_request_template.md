## Summary

<!-- Describe the change and link the relevant issue. -->

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] Documentation and CLI help are updated when needed.

## Checklist

- [ ] The change is limited to the stated issue.
- [ ] New or changed behavior has regression coverage.
- [ ] Filesystem mutation tests use temporary directories only.
- [ ] Commit messages follow Conventional Commits.
