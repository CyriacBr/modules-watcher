use crate::entry::{
  make_entries, make_file_item, make_missing_entries, MakeEntriesOptions, SupportedPaths,
};
use crate::file_item::FileItem;
use dashmap::DashMap;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
  ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
  pub debug: Option<bool>,
}

#[napi(object)]
#[derive(Clone)]
pub struct EntryChangeCause {
  pub file: String,
  pub state: FileState,
}

#[napi(object)]
#[derive(Clone)]
pub struct EntryChange {
  pub change_type: EntryChangeType,
  pub entry: String,
  pub cause: Option<EntryChangeCause>,
  pub tree: Option<Vec<String>>,
}

#[napi(string_enum)]
#[derive(PartialEq, Debug)]
pub enum EntryChangeType {
  Added,
  DepAdded,
  Modified,
  DepModified,
  Deleted,
  DepDeleted,
}

#[napi(string_enum)]
#[derive(PartialEq, Debug)]
pub enum FileState {
  NotModified,
  Modified,
  Created,
  Deleted,
}

struct WatcherInner {
  pub setup_options: SetupOptions,
  store: DashMap<String, FileItem>,
  entries: Vec<FileItem>,
  pub processed: bool,
  pub cache_dir: String,
  make_entries_opts: Option<MakeEntriesOptions>,
  debug: bool,
}

#[napi(js_name = "ModulesWatcher")]
pub struct Watcher {
  inner: Arc<Mutex<WatcherInner>>,
  stop_watch_flag: Arc<AtomicBool>,
}

impl WatcherInner {
  pub fn _clone_struct(&self) -> WatcherInner {
    let store: DashMap<String, FileItem> = DashMap::new();
    for ref_multi in &self.store {
      store.insert(ref_multi.key().to_string(), ref_multi.value().clone_item());
    }
    WatcherInner {
      setup_options: self.setup_options.clone(),
      store,
      entries: self.entries.iter().map(|x| x.clone_item()).collect(),
      processed: self.processed,
      cache_dir: self.cache_dir.clone(),
      make_entries_opts: self.make_entries_opts.clone(),
      debug: self.debug,
    }
  }

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
    let debug = watcher_opts.debug.unwrap_or(false);

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
    WatcherInner {
      setup_options: watcher_opts,
      store,
      entries,
      processed: true,
      cache_dir,
      make_entries_opts,
      debug,
    }
  }

  pub fn get_entries(&self) -> Vec<FileItem> {
    self.entries.iter().map(|x| x.clone_item()).collect()
  }

  pub fn get_entries_from_item(&self, item: &FileItem) -> Vec<&FileItem> {
    let usage = item.get_usage(&self.store);
    let usage = usage.iter();
    self
      .entries
      .iter()
      .filter(|x| usage.clone().any(|y| y == x.path.to_str().unwrap()))
      .collect()
  }

  fn update_store_with_missing_entries(&mut self) {
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

  fn update_entries_from_store(&mut self) {
    self.entries = self
      .entries
      .iter()
      .map(|x| {
        let key = x.path.to_str().unwrap();
        if let Some(item) = self.store.get(key) {
          let mut res_item = item.clone_item();
          res_item.deps.retain(|x| self.store.contains_key(x));
          return Some(res_item);
        }
        None
      })
      .flatten()
      .collect();
  }

  pub fn remove_dep(&self, dep: &str) {
    self.store.remove(dep);
    self.store.par_iter_mut().for_each(|mut x| {
      x.deps.retain(|d| d != dep);
    });
  }

  fn make_file_deps(&self, file_path: &str) -> Vec<String> {
    if self.store.contains_key(file_path) {
      self.store.remove(file_path).unwrap();
    }
    let project_root = &self.setup_options.project_root;
    let path = PathBuf::from(file_path);
    let res = make_file_item(
      &path,
      std::path::Path::new(project_root),
      &self.store,
      &self.make_entries_opts,
    )
    .unwrap();
    res.deps.iter().map(String::from).collect()
  }

  fn get_checksums_cache(&self) -> HashMap<String, i64> {
    let path = PathBuf::from(self.cache_dir.clone()).join("checksums");
    if !path.exists() {
      return HashMap::new();
    }

    let mut map: HashMap<String, i64> = HashMap::new();

    let file = std::fs::File::open(path).unwrap();
    let reader = BufReader::new(file);

    for line in reader.lines() {
      let ln = line.unwrap();
      let slots: Vec<&str> = ln.split_whitespace().collect();
      let path = slots[0];
      let checksum = str::parse::<i64>(slots[1]).unwrap();
      map.insert(path.to_string(), checksum);
    }

    map
  }

  fn set_checksum_cache(&self, checksum_store: &DashMap<String, i64>) {
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

  pub fn make_changes(&mut self) -> Vec<EntryChange> {
    let old_checksum_store = self.get_checksums_cache();
    let new_checksum_store: DashMap<String, i64> = DashMap::new();

    self.update_store_with_missing_entries();

    let changes: Vec<EntryChange> = self
      .entries
      .par_iter()
      .map(|entry| {
        // for each entry

        // we update entry deps if the file got modified
        let entry_path = entry.path.to_str().unwrap();
        let (entry_checksum, entry_state) = self.get_file_state(entry_path, &old_checksum_store);
        let mut deps = entry.deps.iter().map(String::from).collect();
        if entry_state == FileState::Modified {
          deps = self.make_file_deps(entry_path);
        }

        let mut tree: Vec<String> = Vec::new();
        let mut files = vec![entry.path.to_str().unwrap().to_string()];
        files.extend(deps.into_iter());
        // collect changes for each deps (entry included) of the current entry
        let entry_changes: Vec<Option<EntryChange>> = files
          .iter()
          .enumerate()
          .map(|(i, dep)| {
            tree.insert(0, dep.to_string());
            let is_entry = i == 0;
            // Try to determine if the file changed
            let (checksum, state) = if is_entry {
              (entry_checksum, entry_state.clone())
            } else {
              self.get_file_state(dep, &old_checksum_store)
            };
            new_checksum_store.insert(dep.to_string(), checksum);
            match state {
              FileState::Deleted => {
                return Some(EntryChange {
                  change_type: if is_entry {
                    EntryChangeType::Deleted
                  } else {
                    EntryChangeType::DepDeleted
                  },
                  entry: entry.path.to_str().unwrap().to_string(),
                  cause: if is_entry {
                    None
                  } else {
                    Some(EntryChangeCause {
                      file: dep.to_string(),
                      state,
                    })
                  },
                  tree: if is_entry { None } else { Some(tree.clone()) },
                });
              }
              FileState::Created => {
                return Some(EntryChange {
                  change_type: if is_entry {
                    EntryChangeType::Added
                  } else {
                    EntryChangeType::DepAdded
                  },
                  entry: entry.path.to_str().unwrap().to_string(),
                  cause: if is_entry {
                    None
                  } else {
                    Some(EntryChangeCause {
                      file: dep.to_string(),
                      state,
                    })
                  },
                  tree: if is_entry { None } else { Some(tree.clone()) },
                });
              }
              FileState::Modified => {
                return Some(EntryChange {
                  change_type: if is_entry {
                    EntryChangeType::Modified
                  } else {
                    EntryChangeType::DepModified
                  },
                  entry: entry.path.to_str().unwrap().to_string(),
                  cause: if is_entry {
                    None
                  } else {
                    Some(EntryChangeCause {
                      file: dep.to_string(),
                      state,
                    })
                  },
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
    self.update_entries_from_store();

    changes
  }

  fn get_file_state(
    &self,
    file_path: &str,
    checksum_store: &HashMap<String, i64>,
  ) -> (i64, FileState) {
    if !Path::new(file_path).exists() {
      if let Some(res) = checksum_store.get(file_path) {
        if *res == -1 {
          return (-1, FileState::NotModified);
        }
      }
      return (-1, FileState::Deleted);
    }
    let content = std::fs::read_to_string(&file_path).unwrap();
    let curr_checksum = crc32fast::hash(content.as_bytes()) as i64;
    if let Some(old_value) = checksum_store.get(file_path) {
      if curr_checksum == *old_value {
        (curr_checksum, FileState::NotModified)
      } else if *old_value == -1 {
        (curr_checksum, FileState::Created)
      } else {
        (curr_checksum, FileState::Modified)
      }
    } else {
      (curr_checksum, FileState::Created)
    }
  }

  pub fn get_dirs_to_watch(&self) -> Vec<String> {
    let mut set = HashSet::new();
    set.insert(self.setup_options.project_root.clone());
    // TODO: ignore nested folders when a parent foldr is already selected
    for ref_multi in &self.store {
      let parent = ref_multi.path.parent().unwrap();
      set.insert(parent.to_str().unwrap().to_string());
    }
    set.into_iter().collect()
  }
}

#[napi]
impl Watcher {
  #[napi(factory)]
  pub fn setup(opts: SetupOptions) -> Self {
    let inner = WatcherInner::setup(opts);
    Watcher {
      inner: Arc::new(Mutex::new(inner)),
      stop_watch_flag: Arc::new(AtomicBool::new(false)),
    }
  }

  pub fn setup_options(&self) -> SetupOptions {
    self.inner.lock().unwrap().setup_options.clone()
  }

  pub fn processed(&self) -> bool {
    self.inner.lock().unwrap().processed
  }

  #[napi]
  pub fn cache_dir(&self) -> String {
    self.inner.lock().unwrap().cache_dir.clone()
  }

  #[napi]
  pub fn get_entries(&self) -> Vec<FileItem> {
    self.inner.lock().unwrap().get_entries()
  }

  #[napi]
  pub fn make_changes(&mut self) -> Vec<EntryChange> {
    self.inner.lock().unwrap().make_changes()
  }

  #[napi]
  pub fn get_dirs_to_watch(&self) -> Vec<String> {
    self.inner.lock().unwrap().get_dirs_to_watch()
  }

  pub fn watch<F>(&mut self, on_event: F)
  where
    F: Fn(Vec<EntryChange>) -> Result<(), String> + std::marker::Sync + std::marker::Send + 'static,
  {
    let flag = self.stop_watch_flag.clone();
    let on_event_arced = Arc::new(on_event);
    let inner = self.inner.clone();

    /**
     * Spawn a thread to poll for changes.
     * We're using a poller because watching for file system changes is unreliable
     * across platforms.
     * See https://github.com/notify-rs/notify/issues/465 and https://github.com/notify-rs/notify/issues/468
     **/
    std::thread::spawn(move || {
      let on_event_cb = on_event_arced.clone();
      loop {
        if flag.load(Ordering::Relaxed) {
          flag.store(false, Ordering::Relaxed);
          break;
        }
        let mut mutself = inner.lock().unwrap();
        let changes = mutself.make_changes();
        drop(mutself);
        if !changes.is_empty() {
          on_event_cb(changes).unwrap();
        }
        std::thread::sleep(Duration::from_millis(250));
      }
    });
    // listening...
  }

  #[napi]
  pub fn stop_watching(&self) {
    self.stop_watch_flag.store(true, Ordering::Relaxed);
    // wait for watching to stop
    loop {
      if !self.stop_watch_flag.load(Ordering::Relaxed) {
        break;
      }
    }
  }

  #[napi(
    js_name = "watch",
    ts_args_type = "callback: (err: null | Error, result: EntryChange[]) => void"
  )]
  pub fn napi_watch(&mut self, callback: napi::JsFunction) {
    let tsfn: ThreadsafeFunction<Vec<EntryChange>, ErrorStrategy::CalleeHandled> = callback
      .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<Vec<EntryChange>>| {
        let changes = ctx.value;
        let mut napi_array = ctx.env.create_array(changes.len() as u32).unwrap();
        for (i, change) in changes.into_iter().enumerate() {
          napi_array.set(i as u32, change).unwrap();
        }
        return Ok(vec![napi_array]);
      })
      .unwrap();

    self.watch(move |item| {
      tsfn.call(Ok(item), ThreadsafeFunctionCallMode::Blocking);
      Ok(())
    })
  }
}

#[cfg(test)]
mod tests {
  use crate::watcher::{EntryChangeType, SetupOptions, Watcher};
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
      debug: None,
    });
    assert_eq!(watcher.processed(), true);
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
      debug: None,
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
      debug: None,
    });

    // First call, we expect to detect two changes of type added
    if std::path::Path::new(&watcher.cache_dir()).exists() {
      std::fs::remove_dir_all(&watcher.cache_dir()).unwrap();
    } else {
      std::fs::create_dir(&watcher.cache_dir()).unwrap();
    }
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].change_type, EntryChangeType::Added);
    assert_eq!(changes[1].change_type, EntryChangeType::Added);
    assert_eq!(changes[2].change_type, EntryChangeType::DepAdded);

    // Second call, we expect no changes
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 0);

    // Third call after modifying a file. We expect changes
    let since_the_epoch = std::time::SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap();
    std::fs::write(
      path_1,
      format!("modified at: {} // timestamp", since_the_epoch.as_millis()),
    )
    .unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, EntryChangeType::Modified);

    // 4th call, we modify a dep
    std::fs::write(
      PROJECT_A_PATH.join("z.js").to_str().unwrap().to_string(),
      format!(
        "export const Z = {}; // timestamp",
        since_the_epoch.as_millis()
      ),
    )
    .unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, EntryChangeType::DepModified);
    assert_eq!(changes[0].entry, path_2);

    // 5th call, we remove z
    std::fs::remove_file(PROJECT_A_PATH.join("z.js")).unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, EntryChangeType::DepDeleted);
    assert_eq!(changes[0].entry, path_2);

    // 6h call, we restore z
    std::fs::write(
      PROJECT_A_PATH.join("z.js").to_str().unwrap().to_string(),
      format!(
        "export const Z = {}; // timestamp",
        since_the_epoch.as_millis()
      ),
    )
    .unwrap();
    let changes = watcher.make_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_type, EntryChangeType::DepAdded);
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
      debug: None,
    });
    assert_eq!(watcher.processed(), true);

    let called = Arc::new(AtomicBool::new(false)).clone();
    let called_thread = called.clone();
    watcher.watch(move |_| {
      called_thread.store(true, Ordering::Relaxed);
      Ok(())
    });
    // std::thread::sleep(std::time::Duration::from_secs(1));
    // We modify a dep of y2
    let since_the_epoch = std::time::SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap();
    std::fs::write(
      PROJECT_A_PATH.join("z2.js").to_str().unwrap().to_string(),
      format!(
        "export const Z = {} // timestamp;",
        since_the_epoch.as_millis()
      ),
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    watcher.stop_watching();

    assert_eq!(called.load(Ordering::Relaxed), true);
  }
}
