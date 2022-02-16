use std::path::PathBuf;
use dashmap::DashMap;
use crate::entry::{FileItem, make_entries, MakeEntriesOptions};

#[napi(object)]
#[derive(Clone)]
pub struct SetupOptions {
    pub project: String,
    pub project_root: String,
    pub glob_entries: Option<Vec<String>>,
    pub entries: Option<Vec<String>>,
    pub cache_dir: Option<String>,
}

#[napi(js_name = "ModulesWatcher")]
pub struct Watcher {
    pub setup_options: SetupOptions,
    store: DashMap<String, FileItem>,
    entries: Vec<FileItem>,
    pub processed: bool,
}

#[napi]
impl Watcher {
    #[napi(factory)]
    pub fn setup(opts: SetupOptions) -> Self {
        let watcher_opts = opts.clone();
        let entries_vec = opts.entries.unwrap_or(vec![]);
        let globs_vec = opts.glob_entries.unwrap_or(vec![]);
        let entry_paths: Vec<PathBuf> = entries_vec.iter().map(|x| PathBuf::from(x)).collect();
        let entry_globs: Vec<&str> = globs_vec.iter().map(|x| &x[..]).collect();

        let (store, entries) = make_entries(entry_paths, Some(entry_globs), PathBuf::from(opts.project_root), None);
        Watcher { setup_options: watcher_opts, store, entries, processed: true }
    }

    #[napi]
    pub fn makeChanges(&self) {}
}


#[cfg(test)]
mod tests {
    use lazy_static::lazy_static;
    use crate::watcher::{SetupOptions, Watcher};
    use std::path::{PathBuf};

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
}