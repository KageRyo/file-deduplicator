#![allow(dead_code, unused_imports)]

use criterion::{Criterion, criterion_group, criterion_main};
use std::fs;
use tempfile::TempDir;

#[path = "../src/duplicate.rs"]
mod duplicate;
#[path = "../src/hasher.rs"]
mod hasher;
#[path = "../src/scanner.rs"]
mod scanner;

use scanner::FileInfo;

fn setup_bench_data(num_files: usize, size: usize) -> (TempDir, Vec<FileInfo>) {
    let dir = TempDir::new().unwrap();
    let mut files = Vec::new();
    for i in 0..num_files {
        let path = dir.path().join(format!("file_{}.bin", i));
        // Alternate content to create duplicates
        let content = if i % 2 == 0 {
            vec![0u8; size]
        } else {
            vec![1u8; size]
        };
        fs::write(&path, content).unwrap();
        files.push(FileInfo::from_path(&path).unwrap());
    }
    (dir, files)
}

fn bench_duplicate_detection(c: &mut Criterion) {
    let (_dir, files) = setup_bench_data(100, 1024 * 1024); // 100 files of 1MB each

    c.bench_function("find_duplicates_100_files_1mb", |b| {
        b.iter(|| duplicate::find_duplicates(files.clone(), None))
    });
}

criterion_group!(benches, bench_duplicate_detection);
criterion_main!(benches);
