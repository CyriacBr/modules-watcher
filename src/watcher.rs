use std::path::PathBuf;
use dashmap::DashMap;
use crate::entry::{FileItem, make_entries, make_user_file, MakeEntriesOptions};
use rayon::prelude::*;

#[napi(object)]
#[derive(Clone)]
pub struct SetupOptions {
    pub project: String,
    pub project_root: String,
    pub glob_entries: Option<Vec<String>>,
    pub entries: Option<Vec<String>>,
    pub cache_dir: Option<String>,
}

#[napi(object)]
#[derive(Clone)]
pub struct EntryChange {
    pub change_type: String,
    pub entry: String,
    pub tree: Option<Vec<String>>,
}

#[napi(js_name = "ModulesWatcher")]
pub struct Watcher {
    pub setup_options: SetupOptions,
    store: DashMap<String, FileItem>,
    entries: Vec<FileItem>,
    pub processed: bool,
    pub cache_dir: String,
}

#[napi]
impl Watcher {
    #[napi(factory)]
    pub fn setup(opts: SetupOptions) -> Self {
        let watcher_opts = opts.clone();
        let entries_vec = opts.entries.unwrap_or(vec![]);
        let project_root = opts.project_root;
        let cache_dir = opts.cache_dir.unwrap_or(PathBuf::from(project_root.clone()).join("mw-cache").to_str().unwrap().to_string());

        let globs_vec = opts.glob_entries.unwrap_or(vec![]);
        let entry_paths: Vec<PathBuf> = entries_vec.iter().map(|x| PathBuf::from(x)).collect();
        let entry_globs: Vec<&str> = globs_vec.iter().map(|x| &x[..]).collect();

        let (store, entries) = make_entries(entry_paths, Some(entry_globs), PathBuf::from(project_root), None);
        Watcher { setup_options: watcher_opts, store, entries, processed: true, cache_dir }
    }

    pub fn make_file_deps(&self, file_path: &str) {
        let project_root = &self.setup_options.project_root;
        let path = PathBuf::from(file_path);
        make_user_file(&path, std::path::Path::new(project_root), &self.store, &None).unwrap();
    }

    #[napi]
    pub fn makeChanges(&self) -> Vec<EntryChange> {
        let changes: Vec<EntryChange> = self.entries.par_iter().map(|x| {
            let mut tree: Vec<String> = Vec::new();
            let mut files = vec![x.path.to_str().unwrap().to_string()];
            files.extend(x.direct_deps.clone());
            for (i, dep) in files.iter().enumerate() {
                tree.insert(0, dep.to_string());
                let is_entry = i == 0;
                // Try to determine if the file changed
                if let Some(changed) = self.is_file_changed(&dep, &self.cache_dir) {
                    if changed {
                        if is_entry {
                            // if entry changed, recompute deps
                            self.make_file_deps(&dep);
                        }
                        return Some(EntryChange {
                            change_type: if is_entry { "modified".to_string() } else { "dep-modified".to_string() },
                            entry: x.path.to_str().unwrap().to_string(),
                            tree: Some(tree.clone()),
                        });
                    }
                }
                // If the file wasn't even cached, we continue unless file = entry, in which case that mean it's a new file
                else if is_entry {
                    return Some(EntryChange {
                        change_type: "added".to_string(),
                        entry: x.path.to_str().unwrap().to_string(),
                        tree: None,
                    });
                }
            }
            None
        }).filter(|x| x.is_some()).map(|x| x.unwrap()).collect();
        changes
    }

    // THIS HAS SIDE-EFFECT
    fn is_file_changed(&self, file_path: &str, cache_path: &str) -> Option<bool> {
        let key = format!("checksum:{}", file_path);
        let content = std::fs::read_to_string(&file_path).unwrap();
        let curr_checksum = crc32fast::hash(content.as_bytes());
        match cacache::read_sync(cache_path, &key) {
            Ok(data) => {
                let parsed_data = str::parse::<u32>(&std::str::from_utf8(&data).unwrap()).unwrap();

                cacache::write_sync(cache_path, &key, format!("{}", curr_checksum).as_bytes()).unwrap();

                if curr_checksum != parsed_data {
                    return Some(true);
                }
                Some(false)
            }
            _ => {
                cacache::write_sync(cache_path, &key, format!("{}", curr_checksum).as_bytes()).unwrap();
                return None;
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use lazy_static::lazy_static;
    use crate::watcher::{SetupOptions, Watcher};
    use std::path::{Path, PathBuf};
    use std::time::UNIX_EPOCH;

    lazy_static! {
        static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
        static ref PROJECT_A_PATH: PathBuf = CWD.join("tests/fixtures/project_a");
    }

    #[test]
    fn setup_test() {
        let path_1 = PROJECT_A_PATH.join("relative_w_ext.js").to_str().unwrap().to_string();
        let path_2 = PROJECT_A_PATH.join("y.js").to_str().unwrap().to_string();
        let watcher = Watcher::setup(SetupOptions {
            project: "Project A".to_string(),
            project_root: PROJECT_A_PATH.to_str().unwrap().to_string(),
            glob_entries: None,
            entries: Some(vec![path_1, path_2]),
            cache_dir: None,
        });
        assert_eq!(watcher.processed, true);
    }

    #[test]
    fn make_changes_test() {
        let path_1 = PROJECT_A_PATH.join("timestamp.js").to_str().unwrap().to_string();
        let path_2 = PROJECT_A_PATH.join("y.js").to_str().unwrap().to_string();
        let watcher = Watcher::setup(SetupOptions {
            project: "Project A".to_string(),
            project_root: PROJECT_A_PATH.to_str().unwrap().to_string(),
            glob_entries: None,
            entries: Some(vec![path_1.clone(), path_2.clone()]),
            cache_dir: None,
        });

        // First call, we expect to detect two changes of type added
        if std::path::Path::new(&watcher.cache_dir).exists() {
            std::fs::remove_dir_all(&watcher.cache_dir);
        } else {
            std::fs::create_dir(&watcher.cache_dir);
        }
        let changes = watcher.makeChanges();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].change_type, "added".to_string());
        assert_eq!(changes[1].change_type, "added".to_string());

        // Second call, we expect no changes
        let changes = watcher.makeChanges();
        assert_eq!(changes.len(), 0);

        // Third call after modifying a file. We expect changes
        let since_the_epoch = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap();
        std::fs::write(path_1, format!("modified at: {}", since_the_epoch.as_millis()));
        let changes = watcher.makeChanges();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, "modified".to_string());

        // 4th call, we modify a dep
        std::fs::write(PROJECT_A_PATH.join("z.js").to_str().unwrap().to_string(), format!("export const Z = {};", since_the_epoch.as_millis()));
        let changes = watcher.makeChanges();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, "dep-modified".to_string());
        assert_eq!(changes[0].entry, path_2);
    }
}