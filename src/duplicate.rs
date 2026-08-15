use crate::hasher;
use crate::scanner::{FileInfo, PhysicalFileId, normalized_path_key};
use indicatif::ProgressBar;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Serialize, Clone, Debug)]
pub struct DuplicateGroup {
    pub size: u64,
    pub hash: String,
    pub files: Vec<PathBuf>,
    #[serde(skip)]
    pub(crate) entries: Vec<FileInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct HashError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug)]
pub struct DuplicateResult {
    pub groups: Vec<DuplicateGroup>,
    pub hash_errors: Vec<HashError>,
    pub candidates_hashed: usize,
}

impl DuplicateResult {
    pub fn is_complete(&self) -> bool {
        self.hash_errors.is_empty()
    }
}

pub fn find_duplicates(files: Vec<FileInfo>, pb: Option<&ProgressBar>) -> DuplicateResult {
    let mut size_groups: HashMap<u64, Vec<FileInfo>> = HashMap::new();
    for file in files {
        size_groups.entry(file.size).or_default().push(file);
    }

    let mut candidates = Vec::new();
    for (_, mut files) in size_groups {
        if files.len() < 2 {
            continue;
        }

        files.sort_by_key(|file| normalized_path_key(&file.path));

        // Hard-link aliases refer to one physical file. Keep the
        // lexicographically smallest path as the representative so they are
        // not hashed twice or reported as reclaimable duplicate storage.
        let mut identities = HashSet::<PhysicalFileId>::new();
        let mut unique_files = Vec::new();
        for file in files {
            if identities.insert(file.identity.clone()) {
                unique_files.push(file);
            }
        }
        if unique_files.len() > 1 {
            candidates.extend(unique_files);
        }
    }

    candidates.sort_by_key(|file| normalized_path_key(&file.path));

    if let Some(pb) = pb {
        pb.set_length(candidates.len() as u64);
    }

    let mut hashed_candidates = Vec::new();
    let mut hash_errors = Vec::new();

    let results: Vec<_> = candidates
        .into_par_iter()
        .map(|file| {
            let path = file.path.clone();
            let result = hasher::hash_file(&path)
                .map(|hash| (file, hash))
                .map_err(|error| HashError {
                    path,
                    message: format!("{error:#}"),
                });
            if let Some(pb) = pb {
                pb.inc(1);
            }
            result
        })
        .collect();

    for result in results {
        match result {
            Ok(file) => hashed_candidates.push(file),
            Err(error) => hash_errors.push(error),
        }
    }

    hashed_candidates.sort_by(|left, right| {
        normalized_path_key(&left.0.path).cmp(&normalized_path_key(&right.0.path))
    });
    hash_errors.sort_by(|left, right| {
        normalized_path_key(&left.path)
            .cmp(&normalized_path_key(&right.path))
            .then_with(|| left.message.cmp(&right.message))
    });

    let candidates_hashed = hashed_candidates.len() + hash_errors.len();
    let mut duplicate_map: HashMap<(u64, String), Vec<FileInfo>> = HashMap::new();
    for (file, hash) in hashed_candidates {
        duplicate_map
            .entry((file.size, hash))
            .or_default()
            .push(file);
    }

    let mut groups = duplicate_map
        .into_iter()
        .filter_map(|((size, hash), mut entries)| {
            entries.sort_by(|left, right| {
                normalized_path_key(&left.path).cmp(&normalized_path_key(&right.path))
            });

            // The identity filter is performed before hashing as well, but
            // repeat it here so this invariant remains true if callers build
            // FileInfo values from another source in the future.
            let mut identities = HashSet::<PhysicalFileId>::new();
            entries.retain(|entry| identities.insert(entry.identity.clone()));

            if entries.len() < 2 {
                return None;
            }

            let files = entries.iter().map(|entry| entry.path.clone()).collect();
            Some(DuplicateGroup {
                size,
                hash,
                files,
                entries,
            })
        })
        .collect::<Vec<_>>();

    groups.sort_by(|left, right| {
        left.files
            .first()
            .map(|path| normalized_path_key(path))
            .cmp(&right.files.first().map(|path| normalized_path_key(path)))
            .then_with(|| left.size.cmp(&right.size))
            .then_with(|| left.hash.cmp(&right.hash))
    });

    DuplicateResult {
        groups,
        hash_errors,
        candidates_hashed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_dir;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_find_duplicates() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1");
        let file2 = dir.path().join("file2");
        let file3 = dir.path().join("file3");

        fs::write(&file1, "same").unwrap();
        fs::write(&file2, "same").unwrap();
        fs::write(&file3, "different").unwrap();

        let report = scan_dir(dir.path().to_path_buf(), None, &[]).unwrap();
        let duplicates = find_duplicates(report.files, None);
        assert!(duplicates.is_complete());
        assert_eq!(duplicates.groups.len(), 1);
        assert_eq!(duplicates.groups[0].files.len(), 2);
        assert_eq!(duplicates.groups[0].files[0], file1);
        assert_eq!(duplicates.groups[0].files[1], file2);
    }

    #[test]
    fn hash_errors_are_reported() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        fs::write(&first, b"same").unwrap();
        fs::write(&second, b"same").unwrap();

        let mut report = scan_dir(dir.path().to_path_buf(), None, &[]).unwrap();
        let removed = report
            .files
            .iter()
            .find(|file| file.path == second)
            .unwrap()
            .path
            .clone();
        fs::remove_file(&removed).unwrap();

        let result = find_duplicates(std::mem::take(&mut report.files), None);
        assert_eq!(result.hash_errors.len(), 1);
        assert_eq!(result.hash_errors[0].path, removed);
        assert!(result.groups.is_empty());
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn hard_link_aliases_are_not_reported_as_duplicate_storage() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first");
        let alias = dir.path().join("alias");
        fs::write(&first, b"same").unwrap();
        fs::hard_link(&first, &alias).unwrap();

        let report = scan_dir(dir.path().to_path_buf(), None, &[]).unwrap();
        let result = find_duplicates(report.files, None);
        assert!(result.groups.is_empty());
        assert!(result.hash_errors.is_empty());
        assert_eq!(result.candidates_hashed, 0);
    }
}
