use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dashmap::DashMap;
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

use js_watcher::entry::{MakeEntriesOptions, make_file_item, make_entries};
use js_watcher::watcher::{SetupOptions, Watcher};

lazy_static! {
  static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
  static ref THREEJS_PATH: PathBuf = CWD.join("tests/fixtures/three_js");
}

fn bench_make_entries(c: &mut Criterion) {
  let mut group = c.benchmark_group("make_entries");
  group.bench_function("three_js (all supported paths)", |b| {
    b.iter_with_large_drop(|| {
      make_entries(
        Vec::new(),
        Some(vec!["**/*.js"]),
        THREEJS_PATH.to_path_buf(),
        &None,
      );
    })
  });
  group.finish();
}

fn bench_make_changes(c: &mut Criterion) {
  let mut watcher = Watcher::setup(SetupOptions {
    project: "threejs".to_string(),
    project_root: THREEJS_PATH.to_str().unwrap().to_string(),
    glob_entries: Some(vec!["**/*.js".to_string()]),
    entries: None,
    cache_dir: None,
    supported_paths: None,
    debug: None
  });
  let mut group = c.benchmark_group("make_changes");
  group.bench_function("three_js", |b| {
    b.iter_with_large_drop(|| {
      watcher.make_changes();
    })
  });
  group.finish();
}


fn bench_make_file_item(c: &mut Criterion) {
  let file_path = CWD.join("tests").join("fixtures").join("three_js").join("core").join("BufferAttribute.js");

  c.bench_function("make_file_item", |b| {
    b.iter(|| {
      let store = DashMap::new();
      make_file_item(file_path.as_path(), Path::new(THREEJS_PATH.to_str().unwrap()), &store, &None);
    });
  });
  
}

criterion_group!(benches, bench_make_entries, bench_make_changes, bench_make_file_item);
criterion_main!(benches);
