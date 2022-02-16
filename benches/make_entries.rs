use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

#[path = "../src/entry.rs"]
mod entry;

use entry::{SupportedPath, MakeEntriesOptions};


lazy_static! {
        static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
        static ref THREEJS_PATH: PathBuf = CWD.join("tests/fixtures/three_js");
    }

fn bench_make_entries(c: &mut Criterion) {
    let mut group = c.benchmark_group("make_entries");
    group.bench_function("three_js (all supported paths)", |b| b.iter(|| {
        entry::make_entries(Vec::new(), Some(vec!["**/*.js"]), THREEJS_PATH.to_path_buf(), None);
    }));
    group.bench_function("three_js (only ESM .js)", |b| b.iter(|| {
        entry::make_entries(Vec::new(), Some(vec!["**/*.js"]), THREEJS_PATH.to_path_buf(), Some(MakeEntriesOptions {
            supported_paths: vec![entry::SupportedPath::ESM(vec!["js".to_string()])]
        }));
    }));
    group.finish();
}

criterion_group!(benches, bench_make_entries);
criterion_main!(benches);