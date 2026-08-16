use anyhow::Result;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const PARTIAL_HASH_CHUNK_SIZE: usize = 64 * 1024;

pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; 8192];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

/// Hash the beginning and end of a file as a cheap filter before full hashing.
///
/// Files no larger than one chunk are read completely. Larger files hash one
/// chunk from each end; callers must still use `hash_file` before declaring
/// files duplicates.
pub fn partial_hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let chunk_size = PARTIAL_HASH_CHUNK_SIZE as u64;
    let read_size = size.min(chunk_size) as usize;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0; read_size];

    file.read_exact(&mut buffer)?;
    hasher.update(&buffer);

    if size > chunk_size {
        file.seek(SeekFrom::Start(size - chunk_size))?;
        let mut tail = vec![0; PARTIAL_HASH_CHUNK_SIZE];
        file.read_exact(&mut tail)?;
        hasher.update(&tail);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn partial_hash_matches_for_identical_files() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        let content = vec![42; PARTIAL_HASH_CHUNK_SIZE * 2 + 17];
        fs::write(&first, &content).unwrap();
        fs::write(&second, &content).unwrap();

        assert_eq!(
            partial_hash_file(&first).unwrap(),
            partial_hash_file(&second).unwrap()
        );
    }

    #[test]
    fn partial_hash_distinguishes_files_with_different_edges() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        let first_content = vec![0; PARTIAL_HASH_CHUNK_SIZE + 1];
        let mut second_content = first_content.clone();
        second_content[0] = 1;
        fs::write(&first, &first_content).unwrap();
        fs::write(&second, &second_content).unwrap();

        assert_ne!(
            partial_hash_file(&first).unwrap(),
            partial_hash_file(&second).unwrap()
        );
    }
}
