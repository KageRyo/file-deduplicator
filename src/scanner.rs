use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
#[cfg(unix)]
use std::time::{Duration, UNIX_EPOCH};
use walkdir::{Error as WalkDirError, WalkDir};

#[cfg(windows)]
use std::hash::{Hash, Hasher};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::sync::Arc;

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
    Windows(Arc<same_file::Handle>),
    Path(PathBuf),
}

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub changed: Option<SystemTime>,
    pub identity: PhysicalFileId,
}

impl FileInfo {
    fn from_metadata(path: &Path, metadata: &Metadata) -> io::Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            size: metadata.len(),
            modified: metadata.modified()?,
            changed: metadata_change_time(metadata),
            identity: physical_file_id(path, metadata)?,
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

impl PhysicalFileId {
    pub fn cache_key(&self) -> String {
        match self {
            #[cfg(unix)]
            Self::Unix { device, inode } => format!("unix:{device}:{inode}"),
            #[cfg(windows)]
            Self::Windows(handle) => {
                let mut hasher = StableHasher::default();
                handle.hash(&mut hasher);
                format!("windows:{:016x}", hasher.finish())
            }
            Self::Path(path) => format!("path:{}", normalized_path(path)),
        }
    }
}

#[cfg(windows)]
#[derive(Default)]
struct StableHasher(u64);

#[cfg(windows)]
impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0 = if self.0 == 0 {
            0xcbf29ce484222325
        } else {
            self.0
        };
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

#[cfg(unix)]
fn metadata_change_time(metadata: &Metadata) -> Option<SystemTime> {
    system_time_from_unix_parts(metadata.ctime(), metadata.ctime_nsec())
}

#[cfg(not(unix))]
fn metadata_change_time(_: &Metadata) -> Option<SystemTime> {
    None
}

#[cfg(unix)]
fn system_time_from_unix_parts(seconds: i64, nanoseconds: i64) -> Option<SystemTime> {
    let nanoseconds = u32::try_from(nanoseconds).ok()?;
    if seconds >= 0 {
        UNIX_EPOCH.checked_add(Duration::new(seconds as u64, nanoseconds))
    } else if nanoseconds == 0 {
        UNIX_EPOCH.checked_sub(Duration::new(seconds.unsigned_abs(), 0))
    } else {
        UNIX_EPOCH.checked_sub(Duration::new(
            seconds.unsigned_abs() - 1,
            1_000_000_000 - nanoseconds,
        ))
    }
}

fn physical_file_id(path: &Path, metadata: &Metadata) -> io::Result<PhysicalFileId> {
    #[cfg(unix)]
    {
        let _ = path;
        Ok(PhysicalFileId::Unix {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    #[cfg(windows)]
    {
        let _ = metadata;
        same_file::Handle::from_path(path)
            .map(Arc::new)
            .map(PhysicalFileId::Windows)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Ok(PhysicalFileId::Path(path.to_path_buf()))
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

#[derive(Clone, Debug, Default)]
pub struct ScanOptions {
    pub min_size: Option<u64>,
    pub exclude: Vec<String>,
    pub max_depth: Option<usize>,
    pub one_file_system: bool,
}

impl ScanReport {
    pub fn is_complete(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn skipped_files(&self) -> usize {
        self.excluded_files + self.below_min_size_files + self.errors.len()
    }

    pub fn summary(
        &self,
        partial_hash_candidates: usize,
        hash_candidates: usize,
        hash_failures: usize,
    ) -> ScanSummary {
        ScanSummary {
            files_scanned: self.files.len(),
            files_skipped: self.skipped_files(),
            scan_failures: self.errors.len(),
            partial_hash_candidates,
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
    pub partial_hash_candidates: usize,
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

#[allow(dead_code)]
pub fn scan_dir(path: PathBuf, min_size: Option<u64>, exclude: &[String]) -> Result<ScanReport> {
    scan_dir_with_options(
        path,
        ScanOptions {
            min_size,
            exclude: exclude.to_vec(),
            ..ScanOptions::default()
        },
    )
}

pub fn scan_dir_with_options(path: PathBuf, options: ScanOptions) -> Result<ScanReport> {
    let exclusions = ExcludePatterns::new(&options.exclude)?;
    let mut report = ScanReport {
        files: Vec::new(),
        excluded_files: 0,
        below_min_size_files: 0,
        errors: Vec::new(),
    };

    let mut walker = WalkDir::new(&path);
    if let Some(max_depth) = options.max_depth {
        walker = walker.max_depth(max_depth);
    }
    if options.one_file_system {
        walker = walker.same_file_system(true);
    }

    for item in walker {
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

        if options
            .min_size
            .is_some_and(|minimum| metadata.len() < minimum)
        {
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
    fn max_depth_limits_recursive_traversal() {
        let dir = tempdir().unwrap();
        let root_file = dir.path().join("root.txt");
        let nested_dir = dir.path().join("nested");
        let nested_file = nested_dir.join("nested.txt");
        let deep_dir = nested_dir.join("deep");
        let deep_file = deep_dir.join("deep.txt");
        fs::create_dir_all(&deep_dir).unwrap();
        fs::write(&root_file, b"root").unwrap();
        fs::write(&nested_file, b"nested").unwrap();
        fs::write(&deep_file, b"deep").unwrap();

        let report = scan_dir_with_options(
            dir.path().to_path_buf(),
            ScanOptions {
                max_depth: Some(1),
                ..ScanOptions::default()
            },
        )
        .unwrap();

        assert!(report.files.iter().any(|file| file.path == root_file));
        assert!(!report.files.iter().any(|file| file.path == nested_file));
        assert!(!report.files.iter().any(|file| file.path == deep_file));

        let root_only = scan_dir_with_options(
            dir.path().to_path_buf(),
            ScanOptions {
                max_depth: Some(0),
                ..ScanOptions::default()
            },
        )
        .unwrap();
        assert!(root_only.files.is_empty());
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn one_file_system_keeps_regular_tree_scans_complete() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), b"content").unwrap();

        let report = scan_dir_with_options(
            dir.path().to_path_buf(),
            ScanOptions {
                one_file_system: true,
                ..ScanOptions::default()
            },
        )
        .unwrap();

        assert!(report.is_complete());
        assert_eq!(report.files.len(), 1);
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

    #[test]
    #[cfg(unix)]
    fn symlinks_are_not_followed() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        fs::write(&target, b"content").unwrap();
        symlink(&target, &link).unwrap();

        let report = scan_dir(dir.path().to_path_buf(), None, &[]).unwrap();

        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].path, target);
    }
}
