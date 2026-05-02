use crate::cli::KeepPolicy;
use crate::duplicate::DuplicateGroup;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn move_duplicates(groups: Vec<DuplicateGroup>, to_dir: &Path) -> Result<()> {
    if !to_dir.exists() {
        fs::create_dir_all(to_dir).context("Failed to create destination directory")?;
    }

    for group in groups {
        // Keep the first one, move the rest
        let (to_keep, to_move) = group.files.split_at(1);
        println!("Keeping: {}", to_keep[0].display());

        for file in to_move {
            let file_name = file
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
            let dest = to_dir.join(file_name);

            // Handle collision in destination
            let mut final_dest = dest.clone();
            let mut count = 1;
            while final_dest.exists() {
                let mut new_name = file_name.to_os_string();
                new_name.push(format!("_{}", count));
                final_dest = to_dir.join(new_name);
                count += 1;
            }

            println!("Moving: {} -> {}", file.display(), final_dest.display());
            fs::rename(file, final_dest).context("Failed to move file")?;
        }
    }

    Ok(())
}

pub fn delete_duplicates(
    groups: Vec<DuplicateGroup>,
    policy: KeepPolicy,
    dry_run: bool,
) -> Result<()> {
    for group in groups {
        let mut files = group.files.clone();

        // Sort based on policy
        match policy {
            KeepPolicy::First => {
                // Keep as is, first one is kept
            }
            KeepPolicy::Newest => {
                files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
                files.reverse(); // Newest first
            }
            KeepPolicy::Oldest => {
                files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
            }
        }

        let (to_keep, to_delete) = files.split_at(1);
        println!("Keeping: {}", to_keep[0].display());

        for file in to_delete {
            if dry_run {
                println!("[DRY RUN] Would delete: {}", file.display());
            } else {
                println!("Deleting: {}", file.display());
                fs::remove_file(file).context("Failed to delete file")?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use std::time::Duration;
    use std::thread;

    #[test]
    fn test_move_duplicates() {
        let dir = tempdir().unwrap();
        let to_dir = tempdir().unwrap();
        let file1 = dir.path().join("f1");
        let file2 = dir.path().join("f2");
        fs::write(&file1, "content").unwrap();
        fs::write(&file2, "content").unwrap();

        let group = DuplicateGroup {
            size: 7,
            hash: "h".to_string(),
            files: vec![file1.clone(), file2.clone()],
        };

        move_duplicates(vec![group], to_dir.path()).unwrap();

        assert!(file1.exists()); // Kept
        assert!(!file2.exists()); // Moved
        assert!(to_dir.path().join("f2").exists());
    }

    #[test]
    fn test_delete_duplicates_safety() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("f1");
        let file2 = dir.path().join("f2");
        fs::write(&file1, "content").unwrap();
        fs::write(&file2, "content").unwrap();

        let group = DuplicateGroup {
            size: 7,
            hash: "h".to_string(),
            files: vec![file1.clone(), file2.clone()],
        };

        // Dry run
        delete_duplicates(vec![group.clone()], KeepPolicy::First, true).unwrap();
        assert!(file1.exists());
        assert!(file2.exists());

        // Actual delete
        delete_duplicates(vec![group], KeepPolicy::First, false).unwrap();
        assert!(file1.exists()); // Kept
        assert!(!file2.exists()); // Deleted
    }

    #[test]
    fn test_delete_keep_newest() {
        let dir = tempdir().unwrap();
        let file_old = dir.path().join("old");
        let file_new = dir.path().join("new");
        
        fs::write(&file_old, "content").unwrap();
        // Wait a bit to ensure different modification times
        thread::sleep(Duration::from_millis(100));
        fs::write(&file_new, "content").unwrap();

        let group = DuplicateGroup {
            size: 7,
            hash: "h".to_string(),
            files: vec![file_old.clone(), file_new.clone()],
        };

        delete_duplicates(vec![group], KeepPolicy::Newest, false).unwrap();
        
        assert!(file_new.exists()); // Kept newest
        assert!(!file_old.exists()); // Deleted oldest
    }
}
