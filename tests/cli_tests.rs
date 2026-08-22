use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn dedup_command(cache_dir: &Path) -> Result<Command, Box<dyn std::error::Error>> {
    let mut command = Command::cargo_bin("dedup")?;
    command.env("DEDUP_CACHE_DIR", cache_dir);
    Ok(command)
}

#[test]
fn test_scan_invalid_size() -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir = tempdir()?;
    let mut cmd = dedup_command(cache_dir.path())?;
    cmd.arg("scan").arg(".").arg("--min-size").arg("hello");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid size format: hello"));
    Ok(())
}

#[test]
fn test_scan_basic() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let cache_dir = tempdir()?;
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = dedup_command(cache_dir.path())?;
    cmd.arg("scan").arg(dir.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Found 1 duplicate groups."));
    Ok(())
}

#[test]
fn test_shell_completions_generate_for_supported_shells() -> Result<(), Box<dyn std::error::Error>>
{
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
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
    let cache_dir = tempdir()?;
    fs::write(dir.path().join("f1.txt"), "dup")?;
    fs::write(dir.path().join("f2.txt"), "dup")?;

    let mut valid = dedup_command(cache_dir.path())?;
    valid
        .arg("scan")
        .arg(dir.path())
        .arg("--threads")
        .arg("1")
        .assert()
        .success();

    let mut invalid = dedup_command(cache_dir.path())?;
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
    let cache_dir = tempdir()?;
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = dedup_command(cache_dir.path())?;
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
    let cache_dir = tempdir()?;
    let destination = dir.path().join("backup");
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = dedup_command(cache_dir.path())?;
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
    let cache_dir = tempdir()?;
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "dup")?;
    fs::write(&f2, "dup")?;

    let mut cmd = dedup_command(cache_dir.path())?;
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
    let cache_dir = tempdir()?;
    let destination = dir.path().join("backup");
    let old = dir.path().join("old.txt");
    let new = dir.path().join("new.txt");
    fs::write(&old, "dup")?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::write(&new, "dup")?;

    let mut cmd = dedup_command(cache_dir.path())?;
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
    let cache_dir = tempdir()?;
    fs::write(dir.path().join("f1.txt"), "dup")?;
    fs::write(dir.path().join("f2.txt"), "dup")?;

    let mut cmd = dedup_command(cache_dir.path())?;
    let output = cmd.arg("scan").arg(dir.path()).arg("--json").output()?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["application_version"], env!("CARGO_PKG_VERSION"));
    assert!(json.get("summary").is_some());
    assert_eq!(json["cache"]["enabled"], true);
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn repeated_scans_reuse_the_persistent_hash_cache() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let cache_dir = tempdir()?;
    fs::write(dir.path().join("first.txt"), "duplicate")?;
    fs::write(dir.path().join("second.txt"), "duplicate")?;

    let first = Command::cargo_bin("dedup")?
        .env("DEDUP_CACHE_DIR", cache_dir.path())
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .output()?;
    assert!(first.status.success());
    let first_json: serde_json::Value = serde_json::from_slice(&first.stdout)?;
    assert_eq!(first_json["cache"]["partial_misses"], 2);
    assert_eq!(first_json["cache"]["full_misses"], 2);

    let second = Command::cargo_bin("dedup")?
        .env("DEDUP_CACHE_DIR", cache_dir.path())
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .output()?;
    assert!(second.status.success());
    let second_json: serde_json::Value = serde_json::from_slice(&second.stdout)?;
    assert_eq!(second_json["cache"]["partial_hits"], 2);
    assert_eq!(second_json["cache"]["full_hits"], 2);
    assert_eq!(second_json["duplicate_groups"].as_array().unwrap().len(), 1);
    Ok(())
}

#[test]
fn changed_files_invalidate_cached_hashes() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let cache_dir = tempdir()?;
    let first = dir.path().join("first.txt");
    let second = dir.path().join("second.txt");
    fs::write(&first, "duplicate")?;
    fs::write(&second, "duplicate")?;

    let initial = Command::cargo_bin("dedup")?
        .env("DEDUP_CACHE_DIR", cache_dir.path())
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .output()?;
    assert!(initial.status.success());

    fs::write(&first, "different")?;
    let changed = Command::cargo_bin("dedup")?
        .env("DEDUP_CACHE_DIR", cache_dir.path())
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .output()?;
    assert!(changed.status.success());
    let json: serde_json::Value = serde_json::from_slice(&changed.stdout)?;
    assert!(json["cache"]["invalidated_entries"].as_u64().unwrap() >= 1);
    assert!(json["duplicate_groups"].as_array().unwrap().is_empty());
    Ok(())
}

#[test]
fn no_cache_disables_persistent_hash_storage() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let cache_dir = tempdir()?;
    fs::write(dir.path().join("first.txt"), "duplicate")?;
    fs::write(dir.path().join("second.txt"), "duplicate")?;

    let output = Command::cargo_bin("dedup")?
        .env("DEDUP_CACHE_DIR", cache_dir.path())
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .arg("--no-cache")
        .output()?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["cache"]["enabled"], false);
    assert!(!cache_dir.path().join("file-deduplicator").exists());
    Ok(())
}

#[test]
fn max_depth_limits_cli_scans() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let nested = dir.path().join("nested");
    fs::create_dir(&nested)?;
    fs::write(dir.path().join("first.txt"), "duplicate")?;
    fs::write(dir.path().join("second.txt"), "duplicate")?;
    fs::write(nested.join("first.txt"), "nested")?;
    fs::write(nested.join("second.txt"), "nested")?;

    let output = Command::cargo_bin("dedup")?
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .arg("--max-depth")
        .arg("1")
        .arg("--no-cache")
        .output()?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["summary"]["files_scanned"], 2);
    assert_eq!(json["duplicate_groups"].as_array().unwrap().len(), 1);
    Ok(())
}

#[test]
fn incomplete_scan_returns_a_distinct_exit_code() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let cache_dir = tempdir()?;
    let missing = dir.path().join("missing");

    let mut cmd = dedup_command(cache_dir.path())?;
    let output = cmd.arg("scan").arg(&missing).arg("--json").output()?;

    assert_eq!(output.status.code(), Some(2));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["summary"]["scan_failures"], 1);
    Ok(())
}

#[test]
fn destructive_commands_refuse_incomplete_scans() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let cache_dir = tempdir()?;
    let missing = dir.path().join("missing");

    let mut cmd = dedup_command(cache_dir.path())?;
    cmd.arg("delete").arg(&missing).arg("--confirm");
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Refusing to delete"))
        .stderr(predicate::str::contains("scan is incomplete"));
    Ok(())
}
