use criterion::{criterion_group, criterion_main, Criterion};
use file_deduplicator::duplicate;
use file_deduplicator::scanner::FileInfo;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

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
        files.push(FileInfo {
            path,
            size: size as u64,
        });
    }
    (dir, files)
}

fn bench_duplicate_detection(c: &mut Criterion) {
    let (_dir, files) = setup_bench_data(100, 1024 * 1024); // 100 files of 1MB each
    
    c.bench_function("find_duplicates_100_files_1mb", |b| {
        b.iter(|| {
            duplicate::find_duplicates(files.clone(), None)
        })
    });
}

criterion_group!(benches, bench_duplicate_detection);
criterion_main!(benches);
