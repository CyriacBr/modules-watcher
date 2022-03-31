use js_watcher::entry::make_entries;
use std::path::PathBuf;
use path_absolutize::*;

fn main() {
  let cwd: PathBuf = std::env::current_dir().unwrap();
  let threejs_path: PathBuf = cwd.join("..").join("js_watcher").join("tests").join("fixtures").join("three_js").absolutize().unwrap().to_path_buf();

  let duration = std::time::Instant::now();
  let (store, _) = make_entries(
    Vec::new(),
    Some(vec!["**/*.js"]),
    threejs_path.to_path_buf(),
    &None,
  );
  println!("Elapsed: {}ms", duration.elapsed().as_millis());
  println!("Processed files: {}", store.len());
  println!("File with deps: {:#?}", store.iter().find(|x| x.deps.len() > 0).unwrap().value());
}
