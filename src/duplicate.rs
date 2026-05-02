use std::collections::HashMap;
use std::path::PathBuf;
use crate::scanner::FileInfo;
use crate::hasher;

pub struct DuplicateGroup {
    pub size: u64,
    pub hash: String,
    pub files: Vec<PathBuf>,
}

pub fn find_duplicates(files: Vec<FileInfo>) -> Vec<DuplicateGroup> {
    // 1. Group by size
    let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for file in files {
        size_groups.entry(file.size).or_default().push(file.path);
    }

    // 2. Filter candidate groups (size matching)
    let candidates: Vec<(u64, Vec<PathBuf>)> = size_groups
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();

    let mut duplicate_groups = Vec::new();

    // 3. For each candidate group, group by hash
    for (size, paths) in candidates {
        let mut hash_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for path in paths {
            if let Ok(hash) = hasher::hash_file(&path) {
                hash_groups.entry(hash).or_default().push(path);
            }
        }

        // 4. Filter duplicates (hash matching)
        for (hash, files) in hash_groups {
            if files.len() > 1 {
                duplicate_groups.push(DuplicateGroup {
                    size,
                    hash,
                    files,
                });
            }
        }
    }

    duplicate_groups
}
