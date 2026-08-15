use crate::cli::KeepPolicy;
use crate::duplicate::DuplicateGroup;
use crate::hasher;
use crate::scanner::{FileInfo, normalized_path_key};
use anyhow::{Context, Result, anyhow};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SkippedOperation {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct CleanupReport {
    pub completed: usize,
    pub skipped: Vec<SkippedOperation>,
}

impl CleanupReport {
    pub fn has_skipped_files(&self) -> bool {
        !self.skipped.is_empty()
    }
}

#[derive(Clone, Debug)]
struct MovePlan {
    source: FileInfo,
    destination: PathBuf,
}

#[derive(Clone, Debug)]
struct GroupMovePlan {
    entries: Vec<FileInfo>,
    moves: Vec<MovePlan>,
    hash: String,
}

pub fn move_duplicates(
    groups: Vec<DuplicateGroup>,
    to_dir: &Path,
    dry_run: bool,
) -> Result<CleanupReport> {
    let plans = build_move_plans(&groups, to_dir)?;
    let mut report = CleanupReport::default();

    for plan in plans {
        let validation = validate_group(&plan.entries, &plan.hash);
        if !validation.is_empty() {
            report_skips(&validation);
            report.skipped.extend(validation);
            report_skip_for_remaining_entries(&mut report, &plan.entries, &plan.hash);
            continue;
        }

        println!("Keeping: {}", plan.entries[0].path.display());
        if dry_run {
            for operation in &plan.moves {
                println!(
                    "[DRY RUN] Would move: {} -> {}",
                    operation.source.path.display(),
                    operation.destination.display()
                );
            }
            continue;
        }

        if !to_dir.exists() {
            fs::create_dir_all(to_dir).context("failed to create move destination directory")?;
        }

        for operation in plan.moves {
            if operation.destination.exists() {
                return Err(anyhow!(
                    "move destination appeared during planning: {}",
                    operation.destination.display()
                ));
            }

            match move_file_with_fallback(
                &operation.source,
                &operation.destination,
                &plan.hash,
                |source: &Path, destination: &Path| fs::rename(source, destination),
            ) {
                Ok(()) => report.completed += 1,
                Err(error) if error.to_string().starts_with("file changed since scan") => {
                    let skipped = SkippedOperation {
                        path: operation.source.path.clone(),
                        reason: error.to_string(),
                    };
                    report_skips(std::slice::from_ref(&skipped));
                    report.skipped.push(skipped);
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to move {} to {}",
                            operation.source.path.display(),
                            operation.destination.display()
                        )
                    });
                }
            }
        }
    }

    Ok(report)
}

pub fn delete_duplicates(
    groups: Vec<DuplicateGroup>,
    policy: KeepPolicy,
    dry_run: bool,
) -> Result<CleanupReport> {
    let mut report = CleanupReport::default();

    for group in groups {
        let entries = ordered_entries(&group, policy);
        let (to_keep, to_delete) = entries
            .split_first()
            .ok_or_else(|| anyhow!("duplicate group has no files"))?;

        let validation = validate_group(&entries, &group.hash);
        if !validation.is_empty() {
            report_skips(&validation);
            report.skipped.extend(validation);
            report_skip_for_remaining_entries(&mut report, &entries, &group.hash);
            continue;
        }

        println!("Keeping: {}", to_keep.path.display());

        for file in to_delete {
            match revalidate_file(file, &group.hash) {
                Ok(()) if dry_run => {
                    println!("[DRY RUN] Would delete: {}", file.path.display());
                }
                Ok(()) => {
                    println!("Deleting: {}", file.path.display());
                    fs::remove_file(&file.path)
                        .with_context(|| format!("failed to delete {}", file.path.display()))?;
                    report.completed += 1;
                }
                Err(reason) => {
                    let skipped = SkippedOperation {
                        path: file.path.clone(),
                        reason,
                    };
                    report_skips(std::slice::from_ref(&skipped));
                    report.skipped.push(skipped);
                }
            }
        }
    }

    Ok(report)
}

fn ordered_entries(group: &DuplicateGroup, policy: KeepPolicy) -> Vec<FileInfo> {
    let mut entries = group.entries.clone();
    match policy {
        KeepPolicy::First => entries.sort_by(|left, right| {
            normalized_path_key(&left.path).cmp(&normalized_path_key(&right.path))
        }),
        KeepPolicy::Newest => entries.sort_by(|left, right| {
            right.modified.cmp(&left.modified).then_with(|| {
                normalized_path_key(&left.path).cmp(&normalized_path_key(&right.path))
            })
        }),
        KeepPolicy::Oldest => entries.sort_by(|left, right| {
            left.modified.cmp(&right.modified).then_with(|| {
                normalized_path_key(&left.path).cmp(&normalized_path_key(&right.path))
            })
        }),
    }
    entries
}

fn validate_group(entries: &[FileInfo], hash: &str) -> Vec<SkippedOperation> {
    entries
        .iter()
        .filter_map(|file| {
            revalidate_file(file, hash)
                .err()
                .map(|reason| SkippedOperation {
                    path: file.path.clone(),
                    reason,
                })
        })
        .collect()
}

fn report_skip_for_remaining_entries(report: &mut CleanupReport, entries: &[FileInfo], hash: &str) {
    let failed_paths: HashSet<PathBuf> = report
        .skipped
        .iter()
        .map(|skip| skip.path.clone())
        .collect();
    for entry in entries {
        if failed_paths.contains(&entry.path) {
            continue;
        }
        let skipped = SkippedOperation {
            path: entry.path.clone(),
            reason: format!(
                "duplicate group skipped because another file failed revalidation (expected hash {})",
                hash
            ),
        };
        report.skipped.push(skipped);
    }
}

fn report_skips(skipped: &[SkippedOperation]) {
    for file in skipped {
        eprintln!("Skipping {}: {}", file.path.display(), file.reason);
    }
}

fn revalidate_file(expected: &FileInfo, expected_hash: &str) -> std::result::Result<(), String> {
    let before = FileInfo::from_path(&expected.path)
        .map_err(|error| format!("file changed since scan (metadata unavailable: {error})"))?;
    if !same_snapshot(expected, &before) {
        return Err("file changed since scan (metadata differs)".to_string());
    }

    let actual_hash = hasher::hash_file(&expected.path)
        .map_err(|error| format!("file changed since scan (hash unavailable: {error:#})"))?;
    if actual_hash != expected_hash {
        return Err("file changed since scan (content hash differs)".to_string());
    }

    let after = FileInfo::from_path(&expected.path).map_err(|error| {
        format!("file changed during verification (metadata unavailable: {error})")
    })?;
    if !same_snapshot(expected, &after) {
        return Err("file changed during verification (metadata differs)".to_string());
    }

    Ok(())
}

fn same_snapshot(expected: &FileInfo, actual: &FileInfo) -> bool {
    expected.path == actual.path
        && expected.size == actual.size
        && expected.modified == actual.modified
        && expected.identity == actual.identity
}

fn build_move_plans(groups: &[DuplicateGroup], to_dir: &Path) -> Result<Vec<GroupMovePlan>> {
    let mut reserved = HashSet::new();
    let mut plans = Vec::new();

    for group in groups {
        let entries = ordered_entries(group, KeepPolicy::First);
        let mut moves = Vec::new();
        for source in entries.iter().skip(1) {
            let file_name = source
                .path
                .file_name()
                .ok_or_else(|| anyhow!("invalid file name: {}", source.path.display()))?;
            let destination = next_destination(to_dir, file_name, &reserved);
            reserved.insert(destination.clone());
            moves.push(MovePlan {
                source: source.clone(),
                destination,
            });
        }
        plans.push(GroupMovePlan {
            entries,
            moves,
            hash: group.hash.clone(),
        });
    }

    Ok(plans)
}

fn next_destination(to_dir: &Path, file_name: &OsStr, reserved: &HashSet<PathBuf>) -> PathBuf {
    let original = to_dir.join(file_name);
    if !original.exists() && !reserved.contains(&original) {
        return original;
    }

    let mut count = 1_u64;
    loop {
        let candidate = to_dir.join(collision_name(file_name, count));
        if !candidate.exists() && !reserved.contains(&candidate) {
            return candidate;
        }
        count += 1;
    }
}

fn collision_name(file_name: &OsStr, count: u64) -> OsString {
    let path = Path::new(file_name);
    let stem = path.file_stem().unwrap_or(file_name);
    let extension = path.extension().filter(|extension| !extension.is_empty());
    let mut name = stem.to_os_string();
    name.push(format!("_{count}"));
    if let Some(extension) = extension {
        name.push(".");
        name.push(extension);
    }
    name
}

fn move_file_with_fallback<F>(
    expected: &FileInfo,
    destination: &Path,
    expected_hash: &str,
    rename: F,
) -> Result<()>
where
    F: Fn(&Path, &Path) -> io::Result<()>,
{
    revalidate_file(expected, expected_hash)
        .map_err(|reason| anyhow!("{reason}: {}", expected.path.display()))?;

    match rename(&expected.path, destination) {
        Ok(()) => {
            verify_destination(destination, expected_hash)?;
            Ok(())
        }
        Err(error) if is_cross_device_error(&error) => {
            copy_verify_then_remove(expected, destination, expected_hash)
        }
        Err(error) => Err(anyhow!(error).context("rename failed")),
    }
}

fn copy_verify_then_remove(
    expected: &FileInfo,
    destination: &Path,
    expected_hash: &str,
) -> Result<()> {
    let mut source = File::open(&expected.path).with_context(|| {
        format!(
            "failed to open {} for cross-filesystem copy",
            expected.path.display()
        )
    })?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .with_context(|| {
            format!(
                "failed to create copy destination {}",
                destination.display()
            )
        })?;

    if let Err(error) = io::copy(&mut source, &mut target) {
        drop(target);
        let _ = fs::remove_file(destination);
        return Err(error).context("failed to copy file across filesystems");
    }
    if let Err(error) = target.flush() {
        drop(target);
        let _ = fs::remove_file(destination);
        return Err(error).context("failed to flush copied file");
    }
    if let Err(error) = target.sync_all() {
        drop(target);
        let _ = fs::remove_file(destination);
        return Err(error).context("failed to sync copied file");
    }
    drop(target);
    drop(source);

    if let Err(error) = verify_destination(destination, expected_hash) {
        let _ = fs::remove_file(destination);
        return Err(error).context("copied file failed verification");
    }

    if let Err(reason) = revalidate_file(expected, expected_hash) {
        let _ = fs::remove_file(destination);
        return Err(anyhow!(
            "file changed since scan: {} ({reason})",
            expected.path.display()
        ));
    }

    if let Err(error) = fs::remove_file(&expected.path) {
        let _ = fs::remove_file(destination);
        return Err(error).context("failed to remove source after verified copy");
    }

    Ok(())
}

fn verify_destination(destination: &Path, expected_hash: &str) -> Result<()> {
    let actual_hash = hasher::hash_file(destination)
        .with_context(|| format!("failed to verify destination {}", destination.display()))?;
    if actual_hash != expected_hash {
        return Err(anyhow!(
            "destination hash differs from scanned hash (expected {}, got {})",
            expected_hash,
            actual_hash
        ));
    }
    Ok(())
}

fn is_cross_device_error(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::EXDEV)
    }

    #[cfg(windows)]
    {
        // ERROR_NOT_SAME_DEVICE from Win32.
        error.raw_os_error() == Some(17)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = error;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hasher;
    use crate::scanner;
    use std::fs;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    fn group_for(paths: &[&Path]) -> DuplicateGroup {
        let entries: Vec<_> = paths
            .iter()
            .map(|path| scanner::FileInfo::from_path(path).unwrap())
            .collect();
        let hash = hasher::hash_file(paths[0]).unwrap();
        DuplicateGroup {
            size: entries[0].size,
            hash,
            files: entries.iter().map(|entry| entry.path.clone()).collect(),
            entries,
        }
    }

    #[test]
    fn test_move_duplicates() {
        let dir = tempdir().unwrap();
        let to_dir = tempdir().unwrap();
        let file1 = dir.path().join("f1");
        let file2 = dir.path().join("f2");
        fs::write(&file1, "content").unwrap();
        fs::write(&file2, "content").unwrap();

        let group = group_for(&[&file1, &file2]);
        let report = move_duplicates(vec![group], to_dir.path(), false).unwrap();

        assert_eq!(report.completed, 1);
        assert!(file1.exists());
        assert!(!file2.exists());
        assert!(to_dir.path().join("f2").exists());
    }

    #[test]
    fn move_dry_run_does_not_modify_filesystem() {
        let dir = tempdir().unwrap();
        let destination = dir.path().join("not-created");
        let file1 = dir.path().join("f1");
        let file2 = dir.path().join("f2");
        fs::write(&file1, "content").unwrap();
        fs::write(&file2, "content").unwrap();

        let group = group_for(&[&file1, &file2]);
        let report = move_duplicates(vec![group], &destination, true).unwrap();

        assert_eq!(report.completed, 0);
        assert!(report.skipped.is_empty());
        assert!(file1.exists());
        assert!(file2.exists());
        assert!(!destination.exists());
    }

    #[test]
    fn collision_names_preserve_extensions_and_are_deterministic() {
        let dir = tempdir().unwrap();
        let to_dir = dir.path().join("destination");
        fs::create_dir(&to_dir).unwrap();

        fs::write(to_dir.join("photo.jpg"), b"existing").unwrap();
        fs::write(to_dir.join("photo_1.jpg"), b"existing").unwrap();
        fs::write(to_dir.join("archive.tar.gz"), b"existing").unwrap();
        fs::write(to_dir.join("README"), b"existing").unwrap();

        let reserved = HashSet::new();
        assert_eq!(
            next_destination(&to_dir, OsStr::new("photo.jpg"), &reserved),
            to_dir.join("photo_2.jpg")
        );
        assert_eq!(
            next_destination(&to_dir, OsStr::new("archive.tar.gz"), &reserved),
            to_dir.join("archive.tar_1.gz")
        );
        assert_eq!(
            next_destination(&to_dir, OsStr::new("README"), &reserved),
            to_dir.join("README_1")
        );
    }

    #[test]
    fn test_delete_duplicates_safety() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("f1");
        let file2 = dir.path().join("f2");
        fs::write(&file1, "content").unwrap();
        fs::write(&file2, "content").unwrap();

        let group = group_for(&[&file1, &file2]);

        let report = delete_duplicates(vec![group.clone()], KeepPolicy::First, true).unwrap();
        assert_eq!(report.completed, 0);
        assert!(file1.exists());
        assert!(file2.exists());

        let report = delete_duplicates(vec![group], KeepPolicy::First, false).unwrap();
        assert_eq!(report.completed, 1);
        assert!(file1.exists());
        assert!(!file2.exists());
    }

    #[test]
    fn changed_after_scan_is_skipped() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("f1");
        let file2 = dir.path().join("f2");
        fs::write(&file1, "content").unwrap();
        fs::write(&file2, "content").unwrap();

        let group = group_for(&[&file1, &file2]);
        fs::write(&file2, "changed").unwrap();

        let report = delete_duplicates(vec![group], KeepPolicy::First, false).unwrap();

        assert_eq!(report.completed, 0);
        assert!(report.skipped.iter().any(|skip| skip.path == file2));
        assert!(file1.exists());
        assert!(file2.exists());
    }

    #[test]
    fn first_policy_uses_lexicographically_smallest_path() {
        let dir = tempdir().unwrap();
        let smallest = dir.path().join("a");
        let largest = dir.path().join("z");
        fs::write(&smallest, "content").unwrap();
        fs::write(&largest, "content").unwrap();
        let group = group_for(&[&largest, &smallest]);

        let entries = ordered_entries(&group, KeepPolicy::First);

        assert_eq!(entries[0].path, smallest);
    }

    #[test]
    fn test_delete_keep_newest() {
        let dir = tempdir().unwrap();
        let file_old = dir.path().join("old");
        let file_new = dir.path().join("new");

        fs::write(&file_old, "content").unwrap();
        thread::sleep(Duration::from_millis(100));
        fs::write(&file_new, "content").unwrap();

        let group = group_for(&[&file_old, &file_new]);
        let report = delete_duplicates(vec![group], KeepPolicy::Newest, false).unwrap();

        assert_eq!(report.completed, 1);
        assert!(file_new.exists());
        assert!(!file_old.exists());
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn cross_device_fallback_is_testable_without_another_mount() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        fs::write(&source, "content").unwrap();
        let expected = scanner::FileInfo::from_path(&source).unwrap();
        let hash = hasher::hash_file(&source).unwrap();

        move_file_with_fallback(&expected, &destination, &hash, |_source, _destination| {
            Err(cross_device_error())
        })
        .unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"content");
    }

    #[cfg(any(unix, windows))]
    fn cross_device_error() -> io::Error {
        #[cfg(unix)]
        {
            io::Error::from_raw_os_error(libc::EXDEV)
        }
        #[cfg(windows)]
        {
            io::Error::from_raw_os_error(17)
        }
    }
}
