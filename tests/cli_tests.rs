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
fn test_shell_completions_generate_for_supported_shells() -> Result<(), Box<dyn std::error::Error>>
{
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let mut cmd = Command::cargo_bin("dedup")?;
        cmd.arg("completions").arg(shell);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("dedup"));
    }
    Ok(())
}

#[test]
fn test_threads_option_requires_a_positive_value() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    fs::write(dir.path().join("f1.txt"), "dup")?;
    fs::write(dir.path().join("f2.txt"), "dup")?;

    let mut valid = Command::cargo_bin("dedup")?;
    valid
        .arg("scan")
        .arg(dir.path())
        .arg("--threads")
        .arg("1")
        .assert()
        .success();

    let mut invalid = Command::cargo_bin("dedup")?;
    invalid
        .arg("scan")
        .arg(dir.path())
        .arg("--threads")
        .arg("0")
        .assert()
        .failure()
        .stderr(predicate::str::contains("must be at least 1"));
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
fn test_trash_dry_run_does_not_modify_filesystem() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = Command::cargo_bin("dedup")?;
    cmd.arg("trash")
        .arg(dir.path())
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("[DRY RUN] Would trash:"));

    assert!(f1.exists());
    assert!(f2.exists());
    Ok(())
}

#[test]
fn test_move_accepts_keep_policy() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let destination = dir.path().join("backup");
    let old = dir.path().join("old.txt");
    let new = dir.path().join("new.txt");
    fs::write(&old, "dup")?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::write(&new, "dup")?;

    let mut cmd = Command::cargo_bin("dedup")?;
    cmd.arg("move")
        .arg(dir.path())
        .arg("--to")
        .arg(&destination)
        .arg("--keep")
        .arg("newest");
    cmd.assert().success();

    assert!(new.exists());
    assert!(!old.exists());
    assert!(destination.join("old.txt").exists());
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
fn incomplete_scan_returns_a_distinct_exit_code() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let missing = dir.path().join("missing");

    let mut cmd = Command::cargo_bin("dedup")?;
    let output = cmd.arg("scan").arg(&missing).arg("--json").output()?;

    assert_eq!(output.status.code(), Some(2));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["summary"]["scan_failures"], 1);
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
        .code(2)
        .stderr(predicate::str::contains("Refusing to delete"))
        .stderr(predicate::str::contains("scan is incomplete"));
    Ok(())
}
