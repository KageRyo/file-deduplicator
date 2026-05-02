use std::collections::HashMap;
use std::path::PathBuf;
use crate::scanner::FileInfo;
use crate::hasher;

use serde::Serialize;

#[derive(Serialize)]
pub struct DuplicateGroup {
    pub size: u64,
    pub hash: String,
    pub files: Vec<PathBuf>,
}

use indicatif::ProgressBar;

use rayon::prelude::*;

pub fn find_duplicates(files: Vec<FileInfo>, pb: Option<&ProgressBar>) -> Vec<DuplicateGroup> {
    // 1. Group by size
    let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for file in files {
        size_groups.entry(file.size).or_default().push(file.path);
    }

    // 2. Filter candidates
    let candidates: Vec<(u64, PathBuf)> = size_groups
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .flat_map(|(size, paths)| paths.into_iter().map(move |path| (size, path)))
        .collect();

    if candidates.is_empty() {
        return Vec::new();
    }

    if let Some(pb) = pb {
        pb.set_length(candidates.len() as u64);
    }

    // 3. Hash candidates in parallel
    let hashed_candidates: Vec<(u64, String, PathBuf)> = candidates
        .into_par_iter()
        .filter_map(|(size, path)| {
            let res = hasher::hash_file(&path).ok().map(|hash| (size, hash, path));
            if let Some(pb) = pb {
                pb.inc(1);
            }
            res
        })
        .collect();

    // 4. Group by (size, hash)
    let mut duplicate_map: HashMap<(u64, String), Vec<PathBuf>> = HashMap::new();
    for (size, hash, path) in hashed_candidates {
        duplicate_map.entry((size, hash)).or_default().push(path);
    }

    // 5. Filter actual duplicates (count > 1)
    duplicate_map
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|((size, hash), files)| DuplicateGroup {
            size,
            hash,
            files,
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
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

        let files = vec![
            FileInfo { path: file1.clone(), size: 4 },
            FileInfo { path: file2.clone(), size: 4 },
            FileInfo { path: file3.clone(), size: 9 },
        ];

        let duplicates = find_duplicates(files, None);
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].files.len(), 2);
        assert!(duplicates[0].files.contains(&file1));
        assert!(duplicates[0].files.contains(&file2));
    }
}
