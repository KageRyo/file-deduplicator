use std::path::PathBuf;
use walkdir::WalkDir;

pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
}

pub fn scan_dir(path: PathBuf) -> Vec<FileInfo> {
    let mut files = Vec::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                files.push(FileInfo {
                    path: entry.path().to_path_buf(),
                    size: metadata.len(),
                });
            }
        }
    }

    files
}
