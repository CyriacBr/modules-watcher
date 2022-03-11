use modules_watcher::entry::make_entries;
use lazy_static::lazy_static;
use std::path::PathBuf;

#[macro_use]
extern crate napi_derive;
extern crate core;

lazy_static! {
    static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
    static ref PROJECT_A_PATH: PathBuf = CWD.join("tests").join("fixtures").join("project_a");
    static ref THREEJS_PATH: PathBuf = CWD.join("tests").join("fixtures").join("three_js");
  }

fn main() {
    let duration = std::time::Instant::now();
    let (store, _) = make_entries(
      Vec::new(),
      Some(vec!["**/*.js"]),
      THREEJS_PATH.to_path_buf(),
      &None,
    );
    println!("Elapsed: {}ms", duration.elapsed().as_millis());
    println!("Processed files: {}", store.len());
}