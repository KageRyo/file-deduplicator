use crate::scanner::FileInfo;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_DIRECTORY_ENV: &str = "DEDUP_CACHE_DIR";
const CACHE_FILE_NAME: &str = "hashes-v1.json";
const CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize)]
pub struct CacheStats {
    pub enabled: bool,
    pub partial_hits: usize,
    pub partial_misses: usize,
    pub full_hits: usize,
    pub full_misses: usize,
    pub invalidated_entries: usize,
    pub pruned_entries: usize,
}

#[derive(Debug)]
pub struct HashCache {
    path: PathBuf,
    entries: BTreeMap<String, CacheEntry>,
    stats: CacheStats,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CacheEntry {
    path: String,
    identity: String,
    size: u64,
    modified: FileTimestamp,
    #[serde(default)]
    changed: Option<FileTimestamp>,
    partial_hash: Option<String>,
    full_hash: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct FileTimestamp {
    seconds: i64,
    nanoseconds: u32,
}

#[derive(Deserialize, Serialize)]
struct CacheFile {
    version: u32,
    entries: BTreeMap<String, CacheEntry>,
}

impl HashCache {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            entries: BTreeMap::new(),
            stats: CacheStats {
                enabled: true,
                ..CacheStats::default()
            },
        }
    }

    pub fn load(path: PathBuf) -> io::Result<Self> {
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::new(path)),
            Err(error) => return Err(error),
        };

        let cache_file: CacheFile = serde_json::from_slice(&contents).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid cache file: {error}"),
            )
        })?;
        if cache_file.version != CACHE_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported cache version {}, expected {CACHE_VERSION}",
                    cache_file.version
                ),
            ));
        }

        Ok(Self {
            path,
            entries: cache_file.entries,
            stats: CacheStats {
                enabled: true,
                ..CacheStats::default()
            },
        })
    }

    pub fn lookup_partial(&mut self, file: &FileInfo) -> Option<String> {
        let key = cache_key(file);
        let result = self
            .lookup(&key, file)
            .and_then(|entry| entry.partial_hash.clone());
        if result.is_some() {
            self.stats.partial_hits += 1;
        } else {
            self.stats.partial_misses += 1;
        }
        result
    }

    pub fn lookup_full(&mut self, file: &FileInfo) -> Option<String> {
        let key = cache_key(file);
        let result = self
            .lookup(&key, file)
            .and_then(|entry| entry.full_hash.clone());
        if result.is_some() {
            self.stats.full_hits += 1;
        } else {
            self.stats.full_misses += 1;
        }
        result
    }

    pub fn record_partial(&mut self, file: &FileInfo, hash: String) {
        if let Some(entry) = self.entry_for(file) {
            entry.partial_hash = Some(hash);
            entry.full_hash = None;
        }
    }

    pub fn record_full(&mut self, file: &FileInfo, hash: String) {
        if let Some(entry) = self.entry_for(file) {
            entry.full_hash = Some(hash);
        }
    }

    pub fn prune_missing(&mut self) {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| Path::new(&entry.path).exists());
        self.stats.pruned_entries += before - self.entries.len();
    }

    pub fn stats(&self) -> CacheStats {
        self.stats.clone()
    }

    pub fn save(&self) -> io::Result<()> {
        let cache_file = CacheFile {
            version: CACHE_VERSION,
            entries: self.entries.clone(),
        };
        let contents = serde_json::to_vec_pretty(&cache_file).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unable to serialize cache: {error}"),
            )
        })?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temporary_path =
            self.path
                .with_file_name(format!(".{}.{}.tmp", CACHE_FILE_NAME, std::process::id()));
        if let Err(error) = fs::write(&temporary_path, contents).and_then(|()| {
            #[cfg(windows)]
            if self.path.exists() {
                fs::remove_file(&self.path)?;
            }
            fs::rename(&temporary_path, &self.path)
        }) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }

        Ok(())
    }

    fn lookup(&mut self, key: &str, file: &FileInfo) -> Option<&CacheEntry> {
        let matches = self
            .entries
            .get(key)
            .is_some_and(|entry| entry_matches(entry, file));
        if !matches {
            if self.entries.remove(key).is_some() {
                self.stats.invalidated_entries += 1;
            }
            return None;
        }
        self.entries.get(key)
    }

    fn entry_for(&mut self, file: &FileInfo) -> Option<&mut CacheEntry> {
        let key = cache_key(file);
        let modified = FileTimestamp::from_system_time(file.modified)?;
        let entry = CacheEntry {
            path: stable_path(&file.path),
            identity: file.identity.cache_key(),
            size: file.size,
            modified,
            changed: file.changed.and_then(FileTimestamp::from_system_time),
            partial_hash: None,
            full_hash: None,
        };

        let replace = self
            .entries
            .get(&key)
            .is_some_and(|current| !entry_matches(current, file));
        if replace {
            self.stats.invalidated_entries += 1;
        }
        Some(
            self.entries
                .entry(key)
                .and_modify(|current| {
                    if replace {
                        *current = entry.clone();
                    }
                })
                .or_insert(entry),
        )
    }
}

impl FileTimestamp {
    fn from_system_time(time: SystemTime) -> Option<Self> {
        let duration = time.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            seconds: i64::try_from(duration.as_secs()).ok()?,
            nanoseconds: duration.subsec_nanos(),
        })
    }
}

fn entry_matches(entry: &CacheEntry, file: &FileInfo) -> bool {
    FileTimestamp::from_system_time(file.modified).is_some_and(|modified| {
        entry.identity == file.identity.cache_key()
            && entry.size == file.size
            && entry.modified == modified
            && entry.changed == file.changed.and_then(FileTimestamp::from_system_time)
    })
}

fn cache_key(file: &FileInfo) -> String {
    stable_path(&file.path)
}

fn stable_path(path: &Path) -> String {
    let absolute = fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir()
                .map(|directory| directory.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    });
    absolute.to_string_lossy().replace('\\', "/")
}

pub fn default_cache_path() -> Option<PathBuf> {
    let base = env::var_os(CACHE_DIRECTORY_ENV)
        .map(PathBuf::from)
        .or_else(|| {
            #[cfg(windows)]
            {
                env::var_os("LOCALAPPDATA").map(PathBuf::from)
            }
            #[cfg(not(windows))]
            {
                env::var_os("XDG_CACHE_HOME")
                    .map(PathBuf::from)
                    .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            }
        })?;

    Some(base.join("file-deduplicator").join(CACHE_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn cache_round_trip_reuses_matching_hashes() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("file.bin");
        let cache_path = directory.path().join("cache.json");
        fs::write(&path, b"content").unwrap();
        let file = FileInfo::from_path(&path).unwrap();

        let mut cache = HashCache::new(cache_path.clone());
        cache.record_partial(&file, "partial".to_string());
        cache.record_full(&file, "full".to_string());
        cache.save().unwrap();

        let mut loaded = HashCache::load(cache_path).unwrap();
        assert_eq!(loaded.lookup_partial(&file).as_deref(), Some("partial"));
        assert_eq!(loaded.lookup_full(&file).as_deref(), Some("full"));
        assert_eq!(loaded.stats().partial_hits, 1);
        assert_eq!(loaded.stats().full_hits, 1);
    }

    #[test]
    fn changed_file_invalidates_cached_hashes() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("file.bin");
        fs::write(&path, b"first").unwrap();
        let first = FileInfo::from_path(&path).unwrap();
        let mut cache = HashCache::new(directory.path().join("cache.json"));
        cache.record_full(&first, "full".to_string());

        fs::write(&path, b"other").unwrap();
        let second = FileInfo::from_path(&path).unwrap();
        assert_eq!(cache.lookup_full(&second), None);
        assert_eq!(cache.stats().invalidated_entries, 1);
    }

    #[test]
    fn deleted_and_moved_paths_are_not_reused() {
        let directory = tempdir().unwrap();
        let original_path = directory.path().join("original.bin");
        let moved_path = directory.path().join("moved.bin");
        fs::write(&original_path, b"content").unwrap();
        let original = FileInfo::from_path(&original_path).unwrap();
        let mut cache = HashCache::new(directory.path().join("cache.json"));
        cache.record_full(&original, "full".to_string());

        fs::rename(&original_path, &moved_path).unwrap();
        let moved = FileInfo::from_path(&moved_path).unwrap();
        assert_eq!(cache.lookup_full(&moved), None);

        cache.prune_missing();
        assert_eq!(cache.stats().pruned_entries, 1);
    }
}
