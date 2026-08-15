use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_scan_invalid_size() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("dedup")?;
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

    let mut cmd = Command::cargo_bin("dedup")?;
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

    let mut cmd = Command::cargo_bin("dedup")?;
    cmd.arg("delete").arg(dir.path()).arg("--dry-run");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[DRY RUN] Would delete:"));

    assert!(f1.exists());
    assert!(f2.exists());
    Ok(())
}

#[test]
fn test_version_uses_package_version() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("dedup")?;
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
    Ok(())
}

#[test]
fn test_move_dry_run_does_not_modify_filesystem() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let destination = dir.path().join("backup");
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = Command::cargo_bin("dedup")?;
    cmd.arg("move")
        .arg(dir.path())
        .arg("--to")
        .arg(&destination)
        .arg("--dry-run");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[DRY RUN] Would move:"));

    assert!(f1.exists());
    assert!(f2.exists());
    assert!(!destination.exists());
    Ok(())
}

#[test]
fn test_json_output_contains_only_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    fs::write(dir.path().join("f1.txt"), "dup")?;
    fs::write(dir.path().join("f2.txt"), "dup")?;

    let mut cmd = Command::cargo_bin("dedup")?;
    let output = cmd.arg("scan").arg(dir.path()).arg("--json").output()?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert!(json.get("summary").is_some());
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn destructive_commands_refuse_incomplete_scans() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let missing = dir.path().join("missing");

    let mut cmd = Command::cargo_bin("dedup")?;
    cmd.arg("delete").arg(&missing).arg("--confirm");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Refusing to delete"))
        .stderr(predicate::str::contains("scan is incomplete"));
    Ok(())
}
