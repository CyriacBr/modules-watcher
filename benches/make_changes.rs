use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

#[macro_use]
extern crate napi_derive;
extern crate core;

#[path = "../src/entry.rs"]
mod entry;
#[path = "../src/watcher.rs"]
mod watcher;

use entry::{SupportedPath, MakeEntriesOptions};
use watcher::{SetupOptions, Watcher};


lazy_static! {
        static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
        static ref THREEJS_PATH: PathBuf = CWD.join("tests/fixtures/three_js");
    }

fn bench_make_changes(c: &mut Criterion) {
    let watcher = Watcher::setup(SetupOptions {
        project: "threejs".to_string(),
        project_root: THREEJS_PATH.to_str().unwrap().to_string(),
        glob_entries: Some(vec!["**/*.js".to_string()]),
        entries: None,
        cache_dir: None,
    });
    let mut group = c.benchmark_group("make_changes");
    group.bench_function("three_js (first_run)", |b| b.iter(|| {
        watcher.makeChanges();
    }));
    group.bench_function("three_js (second_run)", |b| b.iter(|| {
        watcher.makeChanges();
    }));
    group.finish();
}

criterion_group!(benches, bench_make_changes);
criterion_main!(benches);