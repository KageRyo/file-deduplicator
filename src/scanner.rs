use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::{Error as WalkDirError, WalkDir};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

/// The filesystem identity captured for a scanned file.
///
/// Unix files are identified by device and inode. Windows files are identified
/// by volume serial number and file index. The path fallback keeps the scanner
/// conservative on other platforms where the standard library does not expose
/// a stable physical-file identifier.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[allow(dead_code)]
pub enum PhysicalFileId {
    #[cfg(unix)]
    Unix {
        device: u64,
        inode: u64,
    },
    #[cfg(windows)]
    Windows {
        volume_serial_number: u32,
        file_index: u64,
    },
    Path(PathBuf),
}

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub identity: PhysicalFileId,
}

impl FileInfo {
    fn from_metadata(path: &Path, metadata: &Metadata) -> io::Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            size: metadata.len(),
            modified: metadata.modified()?,
            identity: physical_file_id(path, metadata),
        })
    }

    pub fn from_path(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path is not a regular file",
            ));
        }
        Self::from_metadata(path, &metadata)
    }
}

fn physical_file_id(_path: &Path, metadata: &Metadata) -> PhysicalFileId {
    #[cfg(unix)]
    {
        PhysicalFileId::Unix {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    #[cfg(windows)]
    {
        if let (Some(volume_serial_number), Some(file_index)) =
            (metadata.volume_serial_number(), metadata.file_index())
        {
            PhysicalFileId::Windows {
                volume_serial_number,
                file_index,
            }
        } else {
            PhysicalFileId::Path(_path.to_path_buf())
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        PhysicalFileId::Path(_path.to_path_buf())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanErrorKind {
    WalkDir,
    Metadata,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanError {
    pub path: Option<PathBuf>,
    pub kind: ScanErrorKind,
    pub message: String,
}

impl ScanError {
    pub fn kind_as_str(&self) -> &'static str {
        match self.kind {
            ScanErrorKind::WalkDir => "walkdir",
            ScanErrorKind::Metadata => "metadata",
        }
    }
}

#[derive(Debug)]
pub struct ScanReport {
    pub files: Vec<FileInfo>,
    pub excluded_files: usize,
    pub below_min_size_files: usize,
    pub errors: Vec<ScanError>,
}

impl ScanReport {
    pub fn is_complete(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn skipped_files(&self) -> usize {
        self.excluded_files + self.below_min_size_files + self.errors.len()
    }

    pub fn summary(&self, hash_candidates: usize, hash_failures: usize) -> ScanSummary {
        ScanSummary {
            files_scanned: self.files.len(),
            files_skipped: self.skipped_files(),
            scan_failures: self.errors.len(),
            hash_candidates,
            hash_failures,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanSummary {
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub scan_failures: usize,
    pub hash_candidates: usize,
    pub hash_failures: usize,
}

/// Compiled path exclusions.
///
/// Patterns are normalized to `/` separators and matched against the path
/// relative to the scan root. Patterns containing `/` use normal glob
/// semantics, so `target/**` and `**/.git/**` work across platforms. A pattern
/// without `/` is also matched against every path component; this makes
/// `*.tmp`, `.git`, and `target` useful at any directory depth without falling
/// back to unsafe substring matching.
pub struct ExcludePatterns {
    full_path: GlobSet,
    components: GlobSet,
}

impl ExcludePatterns {
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut full_path_builder = GlobSetBuilder::new();
        let mut component_builder = GlobSetBuilder::new();

        for raw_pattern in patterns {
            let pattern = normalize_glob(raw_pattern);
            let glob = Glob::new(&pattern)?;
            full_path_builder.add(glob);

            if !pattern.contains('/') {
                component_builder.add(Glob::new(&pattern)?);
            }
        }

        Ok(Self {
            full_path: full_path_builder.build()?,
            components: component_builder.build()?,
        })
    }

    pub fn is_excluded(&self, root: &Path, path: &Path) -> bool {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let relative = normalized_path(relative);
        let absolute = normalized_path(path);

        if self.full_path.is_match(&relative) || self.full_path.is_match(&absolute) {
            return true;
        }

        relative
            .split('/')
            .filter(|component| !component.is_empty())
            .any(|component| self.components.is_match(component))
    }
}

pub fn scan_dir(path: PathBuf, min_size: Option<u64>, exclude: &[String]) -> Result<ScanReport> {
    let exclusions = ExcludePatterns::new(exclude)?;
    let mut report = ScanReport {
        files: Vec::new(),
        excluded_files: 0,
        below_min_size_files: 0,
        errors: Vec::new(),
    };

    for item in WalkDir::new(&path) {
        let entry = match item {
            Ok(entry) => entry,
            Err(error) => {
                report.errors.push(scan_error_from_walkdir(error));
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        if exclusions.is_excluded(&path, entry.path()) {
            report.excluded_files += 1;
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                report.errors.push(ScanError {
                    path: Some(entry.path().to_path_buf()),
                    kind: ScanErrorKind::Metadata,
                    message: error.to_string(),
                });
                continue;
            }
        };

        if min_size.is_some_and(|minimum| metadata.len() < minimum) {
            report.below_min_size_files += 1;
            continue;
        }

        match FileInfo::from_metadata(entry.path(), &metadata) {
            Ok(file) => report.files.push(file),
            Err(error) => report.errors.push(ScanError {
                path: Some(entry.path().to_path_buf()),
                kind: ScanErrorKind::Metadata,
                message: error.to_string(),
            }),
        }
    }

    report
        .files
        .sort_by_key(|file| normalized_path_key(&file.path));
    report.errors.sort_by(|left, right| {
        normalized_optional_path_key(left.path.as_deref())
            .cmp(&normalized_optional_path_key(right.path.as_deref()))
            .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
            .then_with(|| left.message.cmp(&right.message))
    });

    Ok(report)
}

fn scan_error_from_walkdir(error: WalkDirError) -> ScanError {
    ScanError {
        path: error.path().map(Path::to_path_buf),
        kind: ScanErrorKind::WalkDir,
        message: error
            .io_error()
            .map(ToString::to_string)
            .unwrap_or_else(|| error.to_string()),
    }
}

fn normalize_glob(pattern: &str) -> String {
    pattern.trim().replace('\\', "/")
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn normalized_path_key(path: &Path) -> String {
    normalized_path(path)
}

fn normalized_optional_path_key(path: Option<&Path>) -> String {
    path.map(normalized_path).unwrap_or_default()
}

pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();
    let split_at = s
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(s.len());
    let (num_part, unit_part) = s.split_at(split_at);
    let number: u64 = num_part.parse().ok()?;
    let multiplier = match unit_part.trim() {
        "B" | "" => 1,
        "KB" | "K" => 1024,
        "MB" | "M" => 1024_u64.pow(2),
        "GB" | "G" => 1024_u64.pow(3),
        "TB" | "T" => 1024_u64.pow(4),
        _ => return None,
    };
    number.checked_mul(multiplier)
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
        assert_eq!(parse_size("999999999999999999999999GB"), None);
    }

    #[test]
    fn test_scan_dir() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();

        let file1 = dir.path().join("file1.txt");
        let file2 = sub.join("file2.txt");
        let file3 = dir.path().join("small.txt");

        fs::write(&file1, "large content").unwrap();
        fs::write(&file2, "large content").unwrap();
        fs::write(&file3, "tiny").unwrap();

        let report = scan_dir(dir.path().to_path_buf(), None, &[]).unwrap();
        assert_eq!(report.files.len(), 3);
        assert!(report.is_complete());

        let report = scan_dir(dir.path().to_path_buf(), Some(10), &[]).unwrap();
        assert_eq!(report.files.len(), 2);
        assert_eq!(report.below_min_size_files, 1);

        let report = scan_dir(dir.path().to_path_buf(), None, &["sub".to_string()]).unwrap();
        assert_eq!(report.files.len(), 2);
        assert_eq!(report.excluded_files, 1);
        assert!(!report.files.iter().any(|file| file.path == file2));
    }

    #[test]
    fn glob_exclusions_match_paths_not_substrings() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("target/nested")).unwrap();
        fs::create_dir_all(dir.path().join("project/.git/objects")).unwrap();
        fs::create_dir_all(dir.path().join("not-target")).unwrap();
        fs::create_dir_all(dir.path().join("nested")).unwrap();

        fs::write(dir.path().join("target/nested/target.bin"), b"target").unwrap();
        fs::write(dir.path().join("project/.git/objects/object"), b"git").unwrap();
        fs::write(dir.path().join("not-target/keep.bin"), b"keep").unwrap();
        fs::write(dir.path().join("nested/skip.tmp"), b"tmp").unwrap();
        fs::write(dir.path().join("keep.txt"), b"keep").unwrap();

        let report = scan_dir(
            dir.path().to_path_buf(),
            None,
            &[
                "target/**".to_string(),
                "**/.git/**".to_string(),
                "*.tmp".to_string(),
            ],
        )
        .unwrap();

        let paths: Vec<_> = report
            .files
            .iter()
            .map(|file| file.path.strip_prefix(dir.path()).unwrap().to_path_buf())
            .collect();
        assert!(paths.contains(&PathBuf::from("not-target/keep.bin")));
        assert!(paths.contains(&PathBuf::from("keep.txt")));
        assert!(!paths.iter().any(|path| path.starts_with("target")));
        assert!(!paths.iter().any(|path| path.starts_with("project/.git")));
        assert!(
            !paths
                .iter()
                .any(|path| path.extension() == Some("tmp".as_ref()))
        );
    }

    #[test]
    fn scan_errors_are_reported() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");

        let report = scan_dir(missing.clone(), None, &[]).unwrap();

        assert!(report.files.is_empty());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].path.as_deref(), Some(missing.as_path()));
        assert!(!report.is_complete());
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn hard_link_paths_share_physical_identity() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        fs::write(&first, b"same").unwrap();
        fs::hard_link(&first, &second).unwrap();

        let report = scan_dir(dir.path().to_path_buf(), None, &[]).unwrap();
        assert_eq!(report.files.len(), 2);
        assert_eq!(report.files[0].identity, report.files[1].identity);
    }
}
