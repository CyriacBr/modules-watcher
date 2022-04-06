use criterion::{criterion_group, criterion_main, Criterion};
use lazy_static::lazy_static;
use std::{path::{PathBuf}, fs::File, io::{BufReader, BufRead}};
use memmap2::Mmap;
use wyhash::WyHash;
use std::hash::Hasher;

lazy_static! {
  static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
  static ref THREEJS_PATH: PathBuf = CWD.join("tests/fixtures/three_js");
  static ref FILE_A: PathBuf = CWD
    .join("tests")
    .join("fixtures")
    .join("three_js")
    .join("animation")
    .join("AnimationAction.js");
}

fn bench_read_to_string(c: &mut Criterion) {
  c.bench_function("read_to_string", |b| {
    b.iter(|| {
      let content = std::fs::read_to_string(FILE_A.as_path()).unwrap();
      crc32fast::hash(content.as_bytes());
    });
  });
}

fn bench_read(c: &mut Criterion) {
  c.bench_function("read", |b| {
    b.iter(|| {
      let bytes = std::fs::read(FILE_A.as_path()).unwrap();
      crc32fast::hash(&bytes);
    });
  });
}

fn bench_memmap(c: &mut Criterion) {
  c.bench_function("memmap", |b| {
    b.iter(|| {
      let file = File::open(FILE_A.as_path()).unwrap();
      let mmap = unsafe { Mmap::map(&file).unwrap()  };
      crc32fast::hash(&mmap[..]);
    });
  });
}

fn bench_bufreader(c: &mut Criterion) {
  c.bench_function("bufreader", |b| {
    b.iter(|| {
      let file = File::open(FILE_A.as_path()).unwrap();
      let mut reader = BufReader::new(file);
      let bytes = reader.fill_buf().unwrap();
      crc32fast::hash(&bytes);
    });
  });
}

fn bench_bufreader_with_capacity(c: &mut Criterion) {
  c.bench_function("bufreader_with_capacity", |b| {
    b.iter(|| {
      let file = File::open(FILE_A.as_path()).unwrap();
      let mut reader = BufReader::with_capacity(file.metadata().unwrap().len() as usize + 1,file);
      let bytes = reader.fill_buf().unwrap();
      crc32fast::hash(&bytes);
    });
  });
}

fn bench_hash(c: &mut Criterion) {
  let mut group = c.benchmark_group("hash");
  group.bench_function("crc32", |b| {
    b.iter(|| {
      let bytes = std::fs::read(FILE_A.as_path()).unwrap();
      crc32fast::hash(&bytes);
    })
  });
  group.bench_function("wyhash", |b| {
    b.iter(|| {
      let bytes = std::fs::read(FILE_A.as_path()).unwrap();
      let mut hasher = WyHash::with_seed(3);
      hasher.write(&bytes);
      hasher.finish();
    })
  });
  group.finish();
}

criterion_group!(benches, bench_read_to_string, bench_read, bench_memmap, bench_bufreader, bench_bufreader_with_capacity, bench_hash);
criterion_main!(benches);
