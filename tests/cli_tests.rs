use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_scan_invalid_size() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("file-deduplicator")?;
    cmd.arg("scan").arg(".").arg("--min-size").arg("hello");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid size format: hello"));
    Ok(())
}

#[test]
fn test_scan_basic() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = Command::cargo_bin("file-deduplicator")?;
    cmd.arg("scan").arg(dir.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Found 1 duplicate groups."));
    Ok(())
}

#[test]
fn test_delete_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = Command::cargo_bin("file-deduplicator")?;
    cmd.arg("delete").arg(dir.path()).arg("--dry-run");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[DRY RUN] Would delete:"));

    assert!(f1.exists());
    assert!(f2.exists());
    Ok(())
}
