use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
}

pub fn scan_dir(path: PathBuf, min_size: Option<u64>, exclude: &[String]) -> Vec<FileInfo> {
    let mut files = Vec::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();

            // Check exclusion
            if exclude.iter().any(|ex| path.to_string_lossy().contains(ex)) {
                continue;
            }

            if let Ok(metadata) = entry.metadata() {
                let size = metadata.len();

                // Check min size
                if min_size.is_some_and(|min| size < min) {
                    continue;
                }

                files.push(FileInfo {
                    path: path.to_path_buf(),
                    size,
                });
            }
        }
    }

    files
}

pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.to_uppercase();
    let (num_part, unit_part) =
        s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let num: u64 = num_part.parse().ok()?;

    match unit_part.trim() {
        "B" | "" => Some(num),
        "KB" | "K" => Some(num * 1024),
        "MB" | "M" => Some(num * 1024 * 1024),
        "GB" | "G" => Some(num * 1024 * 1024 * 1024),
        "TB" | "T" => Some(num * 1024 * 1024 * 1024 * 1024),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1KB"), Some(1024));
        assert_eq!(parse_size("2MB"), Some(2 * 1024 * 1024));
        assert_eq!(parse_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("123"), Some(123));
        assert_eq!(parse_size("abc"), None);
    }

    #[test]
    fn test_scan_dir() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();

        let file1 = dir.path().join("file1.txt");
        let file2 = sub.join("file2.txt");
        let file3 = dir.path().join("small.txt");

        fs::write(&file1, "large content").unwrap(); // 13 bytes
        fs::write(&file2, "large content").unwrap(); // 13 bytes
        fs::write(&file3, "tiny").unwrap(); // 4 bytes

        // 1. Basic scan
        let files = scan_dir(dir.path().to_path_buf(), None, &[]);
        assert_eq!(files.len(), 3);

        // 2. Min size filter
        let files = scan_dir(dir.path().to_path_buf(), Some(10), &[]);
        assert_eq!(files.len(), 2);

        // 3. Exclude filter
        let files = scan_dir(dir.path().to_path_buf(), None, &["sub".to_string()]);
        assert_eq!(files.len(), 2);
        assert!(
            !files
                .iter()
                .any(|f| f.path.to_string_lossy().contains("sub"))
        );
    }
}
