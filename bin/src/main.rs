use js_watcher::entry::make_entries;
use std::path::PathBuf;

fn main() {
  let cwd: PathBuf = std::env::current_dir().unwrap();
  let threejs_path: PathBuf = cwd.join("tests").join("fixtures").join("three_js");

  let duration = std::time::Instant::now();
  let (store, _) = make_entries(
    Vec::new(),
    Some(vec!["**/*.js"]),
    threejs_path.to_path_buf(),
    &None,
  );
  println!("Elapsed: {}ms", duration.elapsed().as_millis());
  println!("Processed files: {}", store.len());
}
