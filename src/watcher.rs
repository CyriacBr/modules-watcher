use crate::entry::{
  make_entries, make_file_item, make_missing_entries, FileItem, MakeEntriesOptions, NapiFileItem,
  SupportedPaths,
};
use dashmap::DashMap;
use hotwatch::Event;
use napi::threadsafe_function::{
  ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::{
  collections::HashMap,
  io::{BufRead, BufReader},
  path::PathBuf,
};

#[napi(object)]
#[derive(Clone)]
pub struct SetupOptions {
  pub project: String,
  pub project_root: String,
  pub glob_entries: Option<Vec<String>>,
  pub entries: Option<Vec<String>>,
  pub cache_dir: Option<String>,
  pub supported_paths: Option<SupportedPaths>,
}

#[napi(object)]
#[derive(Clone)]
pub struct EntryChange {
  // TODO: use Enum
  pub change_type: String,
  pub entry: String,
  pub tree: Option<Vec<String>>,
}

#[derive(PartialEq)]
pub enum FileState {
  NotModified,
  Modified,
  Created,
  Deleted,
}

#[napi(js_name = "ModulesWatcher")]
pub struct Watcher {
  pub setup_options: SetupOptions,
  store: DashMap<String, FileItem>,
  entries: Vec<FileItem>,
  pub processed: bool,
  pub cache_dir: String,
  stop_watch_flag: Arc<AtomicBool>,
  make_entries_opts: Option<MakeEntriesOptions>,
}

#[napi]
impl Watcher {
  pub fn clone_struct(&self) -> Watcher {
    let store: DashMap<String, FileItem> = DashMap::new();
    for ref_multi in &self.store {
      store.insert(ref_multi.key().to_string(), ref_multi.value().clone_item());
    }
    Watcher {
      setup_options: self.setup_options.clone(),
      store,
      entries: self.entries.iter().map(|x| x.clone_item()).collect(),
      processed: self.processed,
      cache_dir: self.cache_dir.clone(),
      stop_watch_flag: self.stop_watch_flag.clone(),
      make_entries_opts: self.make_entries_opts.clone(),
    }
  }

  #[napi(factory)]
  pub fn setup(opts: SetupOptions) -> Self {
    let watcher_opts = opts.clone();
    let entries_vec = opts.entries.unwrap_or_default();
    let project_root = opts.project_root;
    let cache_dir = opts.cache_dir.unwrap_or_else(|| {
      PathBuf::from(project_root.clone())
        .join("mw-cache")
        .to_str()
        .unwrap()
        .to_string()
    });

    let globs_vec = opts.glob_entries.unwrap_or_default();
    let entry_paths: Vec<PathBuf> = entries_vec.iter().map(PathBuf::from).collect();
    let entry_globs: Vec<&str> = globs_vec.iter().map(|x| &x[..]).collect();

    let make_entries_opts = Some(MakeEntriesOptions {
      supported_paths: opts.supported_paths,
    });

    let (store, entries) = make_entries(
      entry_paths,
      Some(entry_globs),
      PathBuf::from(project_root),
      &make_entries_opts,
    );
    Watcher {
      setup_options: watcher_opts,
      store,
      entries,
      processed: true,
      cache_dir,
      stop_watch_flag: Arc::new(AtomicBool::new(false)),
      make_entries_opts,
    }
  }

  #[napi]
  pub fn get_entries(&self) -> Vec<NapiFileItem> {
    self.entries.iter().map(|x| x.to_napi()).collect()
  }

  fn update_store(&mut self) {
    let opts = &self.setup_options;
    let entries_vec = opts.entries.clone().unwrap_or_default();
    let project_root = (&opts.project_root).to_string();
    let globs_vec = opts.glob_entries.clone().unwrap_or_default();
    let entry_paths: Vec<PathBuf> = entries_vec.iter().map(PathBuf::from).collect();
    let entry_globs: Vec<&str> = globs_vec.iter().map(|x| &x[..]).collect();

    let new_entries = make_missing_entries(
      entry_paths,
      Some(entry_globs),
      PathBuf::from(project_root),
      &self.store,
      &self.make_entries_opts,
    );
    self.entries.extend(new_entries.into_iter());
  }

  fn make_file_deps(&self, file_path: &str) {
    self.store.remove(file_path).unwrap();
    let project_root = &self.setup_options.project_root;
    let path = PathBuf::from(file_path);
    make_file_item(
      &path,
      std::path::Path::new(project_root),
      &self.store,
      &self.make_entries_opts,
    )
    .unwrap();
  }

  fn get_checksums_cache(&self) -> HashMap<String, u32> {
    let path = PathBuf::from(self.cache_dir.clone()).join("checksums");
    if !path.exists() {
      return HashMap::new();
    }

    let mut map: HashMap<String, u32> = HashMap::new();

    let file = std::fs::File::open(path).unwrap();
    let reader = BufReader::new(file);

    for line in reader.lines() {
      let ln = line.unwrap();
      let slots: Vec<&str> = ln.split_whitespace().collect();
      let path = slots[0];
      let checksum = str::parse::<u32>(slots[1]).unwrap();
      map.insert(path.to_string(), checksum);
    }

    map
  }

  fn set_checksum_cache(&self, checksum_store: &DashMap<String, u32>) {
    let mut result = String::from("");
    for ref_multi in checksum_store {
      result += &format!("{} {}\n", ref_multi.key(), ref_multi.value());
    }

    let dir = PathBuf::from(self.cache_dir.clone());
    if !dir.exists() {
      std::fs::create_dir(&dir).unwrap_or_else(|_| {
        panic!(
          "Couldn't create cache directory at {}",
          dir.to_str().unwrap()
        )
      });
    }
    let path = dir.join("checksums");
    std::fs::write(path, result.trim_end()).unwrap();
  }

  #[napi]
  pub fn make_changes(&mut self) -> Vec<EntryChange> {
    let old_checksum_store = self.get_checksums_cache();
    let new_checksum_store: DashMap<String, u32> = DashMap::new();

    self.update_store();

    let changes: Vec<EntryChange> = self
      .entries
      .par_iter()
      .map(|x| {
        let mut tree: Vec<String> = Vec::new();
        let mut files = vec![x.path.to_str().unwrap().to_string()];
        files.extend(x.deps.clone());
        let entry_changes: Vec<Option<EntryChange>> = files
          .iter()
          .enumerate()
          .map(|(i, dep)| {
            tree.insert(0, dep.to_string());
            let is_entry = i == 0;
            // Try to determine if the file changed
            let (checksum, state) = self.get_file_state(dep, &old_checksum_store);
            if state == FileState::Deleted {
              // self.store.remove(dep);
              return Some(EntryChange {
                change_type: if is_entry {
                  "deleted".to_string()
                } else {
                  "dep-deleted".to_string()
                },
                entry: x.path.to_str().unwrap().to_string(),
                tree: if is_entry { None } else { Some(tree.clone()) },
              });
            }
            new_checksum_store.insert(dep.to_string(), checksum);
            match state {
              FileState::Created => {
                return Some(EntryChange {
                  change_type: if is_entry {
                    "added".to_string()
                  } else {
                    "dep-added".to_string()
                  },
                  entry: x.path.to_str().unwrap().to_string(),
                  tree: if is_entry { None } else { Some(tree.clone()) },
                });
              }
              FileState::Modified => {
                if is_entry {
                  // if entry changed, recompute deps
                  self.make_file_deps(dep);
                }
                return Some(EntryChange {
                  change_type: if is_entry {
                    "modified".to_string()
                  } else {
                    "dep-modified".to_string()
                  },
                  entry: x.path.to_str().unwrap().to_string(),
                  tree: if is_entry { None } else { Some(tree.clone()) },
                });
              }
              _ => None,
            }
          })
          .collect();
        entry_changes
      })
      .flatten()
      .filter(|x| x.is_some())
      .map(|x| x.unwrap())
      .collect();

    self.set_checksum_cache(&new_checksum_store);

    changes
  }

  fn get_file_state(
    &self,
    file_path: &str,
    checksum_store: &HashMap<String, u32>,
  ) -> (u32, FileState) {
    if !Path::new(file_path).exists() {
      return (0, FileState::Deleted);
    }
    let content = std::fs::read_to_string(&file_path).unwrap();
    let curr_checksum = crc32fast::hash(content.as_bytes());
    if let Some(res) = checksum_store.get(file_path) {
      if curr_checksum == *res {
        (curr_checksum, FileState::NotModified)
      } else {
        (curr_checksum, FileState::Modified)
      }
    } else {
      (curr_checksum, FileState::Created)
    }
  }

  #[napi]
  pub fn get_dirs_to_watch(&self) -> Vec<String> {
    let mut set = HashSet::new();
    for ref_multi in &self.store {
      let parent = ref_multi.path.parent().unwrap();
      set.insert(parent.to_str().unwrap().to_string());
    }
    set.into_iter().collect()
  }

  pub fn watch<F>(&mut self, retrieve_entries: bool, on_event: F)
  where
    F:
      Fn(Option<Vec<FileItem>>) -> Result<(), ()> + std::marker::Sync + std::marker::Send + 'static,
  {
    use notify::{watcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;

    let paths = self.get_dirs_to_watch();
    let mut self_clone = self.clone_struct();

    let flag = self.stop_watch_flag.clone();
    let on_event_arced = Arc::new(on_event);
    std::thread::spawn(move || {
      let (tx, rx) = channel();
      let mut watcher = watcher(tx, std::time::Duration::from_millis(200)).unwrap();

      for path in &paths {
        watcher.watch(&path, RecursiveMode::Recursive).unwrap();
      }

      let on_event_cb = on_event_arced.clone();
      let mut event_handler = |path: PathBuf, event: notify::DebouncedEvent| {
        match event {
          Event::Create(_) => {
            self_clone.update_store();
          }
          Event::Write(_) => {
            if self_clone.store.contains_key(path.to_str().unwrap()) {
              self_clone.make_file_deps(path.to_str().unwrap());
            }
          }
          _ => {}
        }
        if !retrieve_entries {
          on_event_cb(None).unwrap();
        } else if let Some(item) = self_clone.store.get(path.to_str().unwrap()) {
          let entries = item.get_entries(&self_clone.store);
          on_event_cb(Some(
            entries
              .iter()
              .map(|x| self_clone.store.get(x).unwrap().clone_item())
              .collect(),
          ))
          .unwrap();
        }
      };

      loop {
        if flag.load(Ordering::Relaxed) {
          flag.store(false, Ordering::Relaxed);
          for path in &paths {
            watcher.unwatch(path).unwrap();
          }
          break;
        }
        match rx.try_recv() {
          Ok(event) => {
            // println!("event: {:?}", event);
            match &event {
              Event::Write(path) => {
                event_handler(path.to_path_buf(), event);
              }
              Event::Create(path) => {
                event_handler(path.to_path_buf(), event);
              }
              Event::Remove(path) => {
                event_handler(path.to_path_buf(), event);
              }
              _ => {}
            }
          }
          Err(TryRecvError::Empty) => {}
          Err(e) => panic!("a watch error occurred: {:?}", e),
        }
      }
    });
    // listening...
  }

  #[napi]
  pub fn stop_watching(&self) {
    self.stop_watch_flag.store(true, Ordering::Relaxed);
  }

  #[napi(
    js_name = "watch",
    ts_args_type = "retrieveItem: boolean, callback: (err: null | Error, result: null | NapiFileItem[]) => void"
  )]
  pub fn napi_watch(&mut self, retrieve_item: bool, callback: napi::JsFunction) {
    let tsfn: ThreadsafeFunction<Option<Vec<NapiFileItem>>, ErrorStrategy::CalleeHandled> =
      callback
        .create_threadsafe_function(
          0,
          |ctx: ThreadSafeCallContext<Option<Vec<NapiFileItem>>>| {
            if let Some(items) = ctx.value {
              let mut result = ctx.env.create_array(items.len() as u32).unwrap();
              for (i, item) in items.iter().enumerate() {
                let mut obj = ctx.env.create_object().unwrap();
                let mut deps_array = ctx.env.create_array(item.deps.len() as u32).unwrap();
                for (j, dep) in item.deps.iter().enumerate() {
                  deps_array.set(j as u32, dep.clone()).unwrap();
                }
                obj.set("path", item.path.clone()).unwrap();
                obj.set("deps", deps_array.coerce_to_object()).unwrap();
                result.set(i as u32, obj).unwrap();
              }
              return Ok(vec![result.coerce_to_object().unwrap()]);
            }
            Ok(vec![])
          },
        )
        .unwrap();

    self.watch(retrieve_item, move |item| {
      if let Some(items) = item {
        tsfn.call(
          Ok(Some(items.iter().map(|item| item.to_napi()).collect())),
          ThreadsafeFunctionCallMode::Blocking,
        );
      } else {
        tsfn.call(Ok(None), ThreadsafeFunctionCallMode::Blocking);
      }
      Ok(())
    })
  }
}

#[cfg(test)]
mod tests {
  use crate::watcher::{SetupOptions, Watcher};
  use lazy_static::lazy_static;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::Arc;
  use std::time::UNIX_EPOCH;

  lazy_static! {
    static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
    static ref PROJECT_A_PATH: PathBuf = CWD.join("tests").join("fixtures").join("project_a");
    static ref THREEJS_PATH: PathBuf = CWD.join("tests").join("fixtures").join("three_js");
  }

  #[test]
  fn setup_test() {
    let path_1 = PROJECT_A_PATH
      .join("relative_w_ext.js")
      .to_str()
      .unwrap()
      .to_string();
    let path_2 = PROJECT_A_PATH.join("y.js").to_str().unwrap().to_string();
    let watcher = Watcher::setup(SetupOptions {
      project: "Project A".to_string(),
      project_root: PROJECT_A_PATH.to_str().unwrap().to_string(),
      glob_entries: None,
      entries: Some(vec![path_1, path_2]),
      cache_dir: None,
      supported_paths: None,
    });
    assert_eq!(watcher.processed, true);
  }

  #[test]
  fn make_changes_three_js() {
    let mut watcher = Watcher::setup(SetupOptions {
      project: "Project threejs".to_string(),
      project_root: THREEJS_PATH.to_str().unwrap().to_string(),
      glob_entries: Some(vec!["**/*.js".to_string()]),
      entries: None,
      cache_dir: None,
      supported_paths: None,
    });

    let duration = std::time::Instant::now();
    watcher.make_changes();
    println!("Elapsed: {}ms", duration.elapsed().as_millis());
    assert_eq!(1, 1);
  }

  #[test]
  fn make_changes_test() {
    let path_1 = PROJECT_A_PATH
      .join("timestamp.js")
      .to_str()
      .unwrap()
      .to_string();
    let path_2 = PROJECT_A_PATH.join("y.js").to_str().unwrap().to_string();
    let mut watcher = Watcher::setup(SetupOptions {
      project: "Project A".to_string(),
      project_root: PROJECT_A_PATH.to_str().unwrap().to_string(),
      glob_entries: None,
      entries: Some(vec![path_1.clone(), path_2.clone()]),
      cache_dir: None,
      supported_paths: None,
    });

    // First call, we expect to detect two changes of type added
    if std::path::Path::new(&watcher.cache_dir).exists() {
      std::fs::remove_dir_all(&watcher.cache_dir).unwrap();
    } else {
      std::fs::create_dir(&watcher.cache_dir).unwrap();
    }
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].change_type, "added".to_string());
    assert_eq!(changes[1].change_type, "added".to_string());
    assert_eq!(changes[2].change_type, "dep-added".to_string());

    // Second call, we expect no changes
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 0);

    // Third call after modifying a file. We expect changes
    let since_the_epoch = std::time::SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap();
    std::fs::write(
      path_1,
      format!("modified at: {}", since_the_epoch.as_millis()),
    )
    .unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, "modified".to_string());

    // 4th call, we modify a dep
    std::fs::write(
      PROJECT_A_PATH.join("z.js").to_str().unwrap().to_string(),
      format!("export const Z = {};", since_the_epoch.as_millis()),
    )
    .unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, "dep-modified".to_string());
    assert_eq!(changes[0].entry, path_2);

    // 5th call, we remove z
    std::fs::remove_file(PROJECT_A_PATH.join("z.js")).unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, "dep-deleted".to_string());
    assert_eq!(changes[0].entry, path_2);

    // 6h call, we restore z
    std::fs::write(
      PROJECT_A_PATH.join("z.js").to_str().unwrap().to_string(),
      format!("export const Z = {};", since_the_epoch.as_millis()),
    )
    .unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, "dep-added".to_string());
    assert_eq!(changes[0].entry, path_2);
  }

  #[test]
  fn watch_test() {
    let path_1 = PROJECT_A_PATH.join("y2.js").to_str().unwrap().to_string();
    let mut watcher = Watcher::setup(SetupOptions {
      project: "Project A".to_string(),
      project_root: PROJECT_A_PATH.to_str().unwrap().to_string(),
      glob_entries: None,
      entries: Some(vec![path_1]),
      cache_dir: None,
      supported_paths: None,
    });
    assert_eq!(watcher.processed, true);

    let called = Arc::new(AtomicBool::new(false)).clone();
    let called_thread = called.clone();
    watcher.watch(true, move |_| {
      called_thread.store(true, Ordering::Relaxed);
      Ok(())
    });
    // We modify a dep of y
    let since_the_epoch = std::time::SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap();
    std::fs::write(
      PROJECT_A_PATH.join("z2.js").to_str().unwrap().to_string(),
      format!("export const Z = {};", since_the_epoch.as_millis()),
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    watcher.stop_watching();

    assert_eq!(called.load(Ordering::Relaxed), true);
  }
}
