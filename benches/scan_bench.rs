#![allow(dead_code, unused_imports)]

use criterion::{
    BenchmarkGroup, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use std::fs;
use tempfile::TempDir;

#[path = "../src/cache.rs"]
mod cache;
#[path = "../src/duplicate.rs"]
mod duplicate;
#[path = "../src/hasher.rs"]
mod hasher;
#[path = "../src/scanner.rs"]
mod scanner;

use scanner::FileInfo;

struct BenchData {
    _dir: TempDir,
    files: Vec<FileInfo>,
}

#[derive(Clone, Copy)]
enum DataShape {
    ManySmall,
    Large,
    MostlyUnique,
    SameSized,
    HighDuplicateRatio,
}

fn setup_bench_data(num_files: usize, size: usize, shape: DataShape) -> BenchData {
    let dir = TempDir::new().unwrap();
    let mut files = Vec::with_capacity(num_files);

    for index in 0..num_files {
        let path = dir.path().join(format!("file_{index}.bin"));
        fs::write(&path, content_for(size, index, num_files, shape)).unwrap();
        files.push(FileInfo::from_path(&path).unwrap());
    }

    BenchData { _dir: dir, files }
}

fn content_for(size: usize, index: usize, total: usize, shape: DataShape) -> Vec<u8> {
    let mut content = vec![0; size];
    match shape {
        DataShape::ManySmall => content.fill((index % 4) as u8),
        DataShape::Large => content.fill((index % 2) as u8),
        DataShape::MostlyUnique => {
            content[..8].copy_from_slice(&(index as u64).to_le_bytes());
        }
        DataShape::SameSized => {
            let middle = hasher::PARTIAL_HASH_CHUNK_SIZE;
            content[middle + (index % (size - middle * 2))] = (index % 251) as u8;
        }
        DataShape::HighDuplicateRatio => {
            let unique_start = total * 95 / 100;
            content.fill(if index < unique_start {
                7
            } else {
                (index % 5) as u8
            });
        }
    }
    content
}

fn bench_scenario<'a>(
    group: &mut BenchmarkGroup<'a, criterion::measurement::WallTime>,
    name: &str,
    data: &BenchData,
) {
    group.throughput(Throughput::Elements(data.files.len() as u64));
    group.bench_function(name, |benchmark| {
        benchmark.iter(|| {
            black_box(
                duplicate::find_duplicates(black_box(data.files.clone()), None, None).unwrap(),
            )
        });
    });
}

fn bench_thread_count<'a>(
    group: &mut BenchmarkGroup<'a, criterion::measurement::WallTime>,
    data: &BenchData,
    threads: usize,
) {
    let name = format!("many_same_sized_files_threads_{threads}");
    group.throughput(Throughput::Elements(data.files.len() as u64));
    group.bench_function(name, |benchmark| {
        benchmark.iter(|| {
            black_box(
                duplicate::find_duplicates(black_box(data.files.clone()), None, Some(threads))
                    .unwrap(),
            )
        });
    });
}

fn bench_duplicate_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("duplicate_detection");

    let many_small = setup_bench_data(1_000, 4 * 1024, DataShape::ManySmall);
    bench_scenario(&mut group, "many_small_files", &many_small);

    let large = setup_bench_data(8, 4 * 1024 * 1024, DataShape::Large);
    bench_scenario(&mut group, "large_files", &large);

    let mostly_unique = setup_bench_data(128, 16 * 1024, DataShape::MostlyUnique);
    bench_scenario(&mut group, "mostly_unique_files", &mostly_unique);

    let same_sized = setup_bench_data(
        128,
        hasher::PARTIAL_HASH_CHUNK_SIZE * 3,
        DataShape::SameSized,
    );
    bench_scenario(&mut group, "many_same_sized_files", &same_sized);
    for threads in [1, 2, 4] {
        bench_thread_count(&mut group, &same_sized, threads);
    }

    let high_duplicate_ratio = setup_bench_data(200, 32 * 1024, DataShape::HighDuplicateRatio);
    bench_scenario(&mut group, "high_duplicate_ratio", &high_duplicate_ratio);

    group.finish();
}

fn bench_cache_behavior(c: &mut Criterion) {
    let data = setup_bench_data(128, 64 * 1024, DataShape::HighDuplicateRatio);
    let mut group = c.benchmark_group("duplicate_detection_cache");
    group.throughput(Throughput::Elements(data.files.len() as u64));

    group.bench_function("cold_scan", |benchmark| {
        benchmark.iter(|| {
            let mut cache = cache::HashCache::new(data._dir.path().join("cold-cache.json"));
            black_box(
                duplicate::find_duplicates_with_cache(
                    black_box(data.files.clone()),
                    None,
                    None,
                    Some(&mut cache),
                )
                .unwrap(),
            )
        });
    });

    let mut warm_cache = cache::HashCache::new(data._dir.path().join("warm-cache.json"));
    duplicate::find_duplicates_with_cache(data.files.clone(), None, None, Some(&mut warm_cache))
        .unwrap();
    group.bench_function("warm_scan", |benchmark| {
        benchmark.iter(|| {
            black_box(
                duplicate::find_duplicates_with_cache(
                    black_box(data.files.clone()),
                    None,
                    None,
                    Some(&mut warm_cache),
                )
                .unwrap(),
            )
        });
    });

    group.finish();
}

criterion_group!(benches, bench_duplicate_detection, bench_cache_behavior);
criterion_main!(benches);
