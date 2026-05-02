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
