use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

#[path = "../src/entry.rs"] mod entry;


lazy_static! {
        static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
        static ref THREEJS_PATH: PathBuf = CWD.join("tests/fixtures/three_js");
    }

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("three_js", |b| b.iter(|| {
        entry::make_entries(Vec::new(), Some("**/*.js"), THREEJS_PATH.to_path_buf());
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);