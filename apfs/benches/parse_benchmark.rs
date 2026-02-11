use criterion::{criterion_group, criterion_main, Criterion};
use std::io::BufReader;

fn open_appfs() -> Option<BufReader<std::fs::File>> {
    let path = std::path::Path::new("../tests/appfs.raw");
    if !path.exists() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    Some(BufReader::new(file))
}

fn bench_open(c: &mut Criterion) {
    if open_appfs().is_none() {
        eprintln!("Skipping benchmarks - appfs.raw not found");
        return;
    }

    c.bench_function("apfs_open", |b| {
        b.iter(|| {
            let reader = open_appfs().unwrap();
            let _vol = apfs::ApfsVolume::open(reader).unwrap();
        })
    });
}

fn bench_list_root(c: &mut Criterion) {
    let reader = match open_appfs() {
        Some(r) => r,
        None => return,
    };

    let mut vol = apfs::ApfsVolume::open(reader).unwrap();

    c.bench_function("apfs_list_root", |b| {
        b.iter(|| {
            let _entries = vol.list_directory("/").unwrap();
        })
    });
}

fn bench_walk_all(c: &mut Criterion) {
    if open_appfs().is_none() {
        return;
    }

    c.bench_function("apfs_walk_all", |b| {
        b.iter(|| {
            let reader = open_appfs().unwrap();
            let mut vol = apfs::ApfsVolume::open(reader).unwrap();
            let _entries = vol.walk().unwrap();
        })
    });
}

fn bench_stat(c: &mut Criterion) {
    let reader = match open_appfs() {
        Some(r) => r,
        None => return,
    };

    let mut vol = apfs::ApfsVolume::open(reader).unwrap();

    // Find a file path to stat
    let walk = vol.walk().unwrap();
    let file_path = walk.iter()
        .find(|e| e.entry.kind == apfs::EntryKind::File && e.entry.size > 0)
        .map(|e| e.path.clone());

    if let Some(path) = file_path {
        c.bench_function("apfs_stat", |b| {
            b.iter(|| {
                let _stat = vol.stat(&path).unwrap();
            })
        });
    }
}

fn bench_read_small_file(c: &mut Criterion) {
    let reader = match open_appfs() {
        Some(r) => r,
        None => return,
    };

    let mut vol = apfs::ApfsVolume::open(reader).unwrap();

    // Find a small file to read
    let walk = vol.walk().unwrap();
    let file_path = walk.iter()
        .find(|e| e.entry.kind == apfs::EntryKind::File && e.entry.size > 0 && e.entry.size < 100_000)
        .map(|e| e.path.clone());

    if let Some(path) = file_path {
        c.bench_function("apfs_read_small_file", |b| {
            b.iter(|| {
                let _data = vol.read_file(&path).unwrap();
            })
        });
    }
}

criterion_group!(benches, bench_open, bench_list_root, bench_walk_all, bench_stat, bench_read_small_file);
criterion_main!(benches);
