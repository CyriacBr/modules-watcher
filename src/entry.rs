use std::path::{Path, PathBuf};
use std::fs::*;
use lazy_static::lazy_static;
use regex::{CaptureMatches, Regex};
use rayon::prelude::*;
use path_clean::{PathClean};
use dashmap::{DashMap};
use dashmap::mapref::one::Ref;
use glob::glob;

pub struct FileItem {
    pub path: PathBuf,
    pub deps: Vec<String>,
}

impl FileItem {
    pub fn clone_item(&self) -> FileItem {
        FileItem {
            path: PathBuf::from(&self.path),
            deps: self.deps.iter().map(String::from).collect(),
        }
    }

    pub fn get_entry(&self, store: &DashMap<String, FileItem>) -> Option<String> {
        for item in store {
            if item.deps.is_empty() && item.path.eq(&self.path) {
                return Some(self.path.to_str().unwrap().to_string());
            }
            for dep in &item.deps {
                if dep.eq(&self.path.to_str().unwrap().to_string()) {
                    return Some(item.path.to_str().unwrap().to_string());
                }
            }
        }
        None
    }
}

pub struct MakeEntriesOptions {
    pub supported_paths: Vec<SupportedPath>,
}

pub enum SupportedPath {
    ESM(Vec<String>),
    DynEsmReq(Vec<String>),
}

pub fn make_entries(entry_paths: Vec<PathBuf>, entry_globs: Option<Vec<&str>>, project_path: PathBuf, opts: Option<MakeEntriesOptions>) -> (DashMap<String, FileItem>, Vec<FileItem>) {
    let store = DashMap::new();
    let entries = make_missing_entries(entry_paths, entry_globs, project_path, &store, opts);
    (store, entries)
}

pub fn make_missing_entries(entry_paths: Vec<PathBuf>, entry_globs: Option<Vec<&str>>, project_path: PathBuf, store: &DashMap<String, FileItem>, opts: Option<MakeEntriesOptions>) -> Vec<FileItem> {
    let mut paths: Vec<PathBuf> = entry_paths;

    for glob_str in entry_globs.unwrap_or_default() {
        let full_glob = if glob_str.starts_with('/') {
            glob_str.to_owned()
        } else {
            project_path.join(glob_str).to_str().unwrap().to_owned()
        };
        paths.extend(glob(&full_glob).expect("Failed to read glob pattern").map(|x| x.unwrap()));
    }

    paths.retain(|x| !store.contains_key(x.to_str().unwrap()));

    paths.par_iter().for_each(|p| {
        make_file_item(p, &project_path, store, &opts);
    });
    let entry_path_str_list: Vec<String> = paths.iter().map(|x| x.to_str().unwrap().to_string()).collect();
    let entries = entry_path_str_list.iter().map(|x| store.get(x).unwrap().clone_item()).collect();
    entries
}

lazy_static! {
    static ref NAMED_MODULE_RE: Regex = Regex::new(r#"(?:import|export)\s+.+from\s+['"]([([\.\~]/)|(\.\./)].+)['"]"#).unwrap();
    static ref UNNAMED_MODULE_RE: Regex = Regex::new(r#"(?:import|export)\s+['"]([([\.\~]/)|(\.\./)].+)['"]"#).unwrap();
    static ref REQUIRE_DYNIMP_RE: Regex = Regex::new(r#"(?:require|import)\(['"]([([\.\~]/)|(\.\./)].+)['"]\)"#).unwrap();

    static ref DEFAULT_JS_EXTS: Vec<String> = vec!["ts", "js", "cjs", "mjs"].iter().map(|x| x.to_string()).collect();
    static ref DEFAULT_SUPPATH_ESM: SupportedPath = SupportedPath::ESM(DEFAULT_JS_EXTS.clone());
    static ref DEFAULT_SUPPATH_DYN_ESM: SupportedPath = SupportedPath::DynEsmReq(DEFAULT_JS_EXTS.clone());
}
pub fn make_file_item<'a>(file_path: &'a Path, project_path: &'a Path, store: &'a DashMap<String, FileItem>, opts: &Option<MakeEntriesOptions>) -> Option<Ref<'a, String, FileItem>> {
    let key = file_path.to_str().unwrap();
    if store.contains_key(key) {
        return Some(store.get(key).unwrap_or_else(|| panic!("Couldn't read {} inside the store", key)));
    }

    let supported_paths: Vec<&SupportedPath> = match opts {
        Some(opts_val) => opts_val.supported_paths.iter().collect(),
        _ => vec![&DEFAULT_SUPPATH_ESM, &DEFAULT_SUPPATH_DYN_ESM]
    };
    let mut supported_exts: Vec<&String> = Vec::new();
    let mut esm_exts: &Vec<String> = &Vec::new();
    let mut dyn_esm_exts: &Vec<String> = &Vec::new();
    for supported_path in &supported_paths {
        match *supported_path {
            SupportedPath::ESM(exts) => {
                esm_exts = exts;
                supported_exts.extend(exts.iter());
            }
            SupportedPath::DynEsmReq(exts) => {
                dyn_esm_exts = exts;
                supported_exts.extend(exts.iter());
            }
        }
    }
    let file_ext = file_path.extension().unwrap().to_str().unwrap().to_string();
    if !supported_exts.iter().any(|x| (*x).eq(&file_ext)) {
        return None;
    }

    store.insert(key.to_string(), FileItem {
        path: PathBuf::from(&file_path),
        deps: Vec::new(),
    });
    let mut all_deps: Vec<String> = Vec::new();

    // Scan file for imports
    let content = read_to_string(&file_path).unwrap_or_else(|_| panic!("Couldn't read file {} ", file_path.to_str().unwrap()));

    let mut captures: Vec<CaptureMatches> = Vec::new();
    for path_type in supported_paths {
        match path_type {
            SupportedPath::ESM(_) => {
                if esm_exts.iter().any(|x| x.eq(&file_ext)) {
                    captures.push(NAMED_MODULE_RE.captures_iter(&content));
                    captures.push(UNNAMED_MODULE_RE.captures_iter(&content));
                }
            }
            SupportedPath::DynEsmReq(_) => {
                if dyn_esm_exts.iter().any(|x| x.eq(&file_ext)) {
                    captures.push(REQUIRE_DYNIMP_RE.captures_iter(&content));
                }
            }
        }
    }
    for capture in captures {
        for cap in capture {
            let source = &cap[1];
            let mut path_buf = if source.starts_with("./") || source.starts_with("../") {
                let dir = file_path.parent().unwrap();
                dir.join(source).clean()
            } else if source.starts_with("~/") {
                let transformed_path = source.replace("~/", "");
                project_path.join(Path::new(&transformed_path)).clean()
            } else {
                panic!("Couldn't handle import: {}", source);
            };
            // If the imported file is a directory, we need to resolve it's index file
            if path_buf.is_dir() {
                if let Some(found) = resolve_index(&path_buf) {
                    path_buf = found;
                } else {
                    panic!("Couldn't handle import: {}", path_buf.to_str().unwrap());
                }
            }
            // If the imported file has no extension, we need to resolve it
            else if path_buf.extension().is_none() {
                if let Some(found) = resolve_with_extension(&path_buf) {
                    path_buf = found;
                } else {
                    panic!("Couldn't handle import: {}", path_buf.to_str().unwrap());
                }
            }
            all_deps.push(path_buf.to_str().unwrap().to_string());
            if let Some(file_ref) = make_file_item(&path_buf.clone(), project_path, store, opts) {
                all_deps.extend(file_ref.deps.clone());
            }
        }
    }
    store.get_mut(key).unwrap_or_else(|| panic!("Couldn't read {} inside the store", key))
        .deps = all_deps;

    Some(store.get(key).unwrap_or_else(|| panic!("Couldn't read {} inside the store", key)))
}

/// Take a path of a file without extension and resolve it's extension.
/// ```rs
/// let path = PathBuf::from("/stuff/project/foo");
/// let index_path = resolve_with_extension(&path).unwrap().to_str();
/// // "/stuff/project/foo.js"
/// ```
/// This will return the path of the first file that:
/// * matches the file_name of the argument
/// * possess an extension
fn resolve_with_extension(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_stem().unwrap();
    let files = path.parent().unwrap().read_dir().unwrap();
    for entry in files.flatten() {
        let entry_path = entry.path();
        if entry_path.is_file() {
            if let Some(_ext) = entry_path.extension() {
                let name = entry_path.file_stem().unwrap().to_str().unwrap();
                if name.eq(file_name) {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

/// Resolves the index file that matches the path send as parameter.
/// ```rs
/// let path = PathBuf::from("/stuff/project/foo");
/// let index_path = resolve_index(&path).unwrap().to_str();
/// // "/stuff/project/foo/index.ts"
/// ```
fn resolve_index(path: &Path) -> Option<PathBuf> {
    if !path.is_dir() {
        return None;
    }
    let files = path.read_dir().unwrap();
    for entry in files.flatten() {
        let entry_path = entry.path();
        if entry_path.is_file() {
            if let Some(_ext) = entry_path.extension() {
                let name = entry_path.file_stem().unwrap().to_str().unwrap();
                if name.eq("index") {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::string::String;
    use dashmap::DashMap;
    use lazy_static::lazy_static;
    use crate::entry::{make_entries, make_file_item, resolve_with_extension};

    lazy_static! {
        static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
        static ref PROJECT_A_PATH: PathBuf = CWD.join("tests/fixtures/project_a");
        static ref THREEJS_PATH: PathBuf = CWD.join("tests/fixtures/three_js");
    }

    #[test]
    fn test_resolve_with_extension() {
        let mut path = CWD.join("tests/fixtures/project_a/b");

        let res = resolve_with_extension(&path).unwrap();
        assert_eq!(res.to_str(), CWD.join("tests/fixtures/project_a/b.js").to_str());
    }

    #[test]
    fn make_entries_test_no_glob() {
        let path_1 = PROJECT_A_PATH.join("relative_w_ext.js");
        let path_2 = PROJECT_A_PATH.join("y.js");
        let mut paths = Vec::new();
        paths.push(path_1);
        paths.push(path_2);

        let (store, entries) = make_entries(paths, None, PROJECT_A_PATH.to_path_buf(), None);
        assert_eq!(entries.len(), 2 as usize);
    }

    #[test]
    fn make_entries_test_glob() {
        let (store, entries) = make_entries(Vec::new(), Some(vec!["**/relative_*.js"]), PROJECT_A_PATH.to_path_buf(), None);
        assert_eq!(entries.len(), 4 as usize);
    }

    #[test]
    fn make_entries_test_three_js() {
        let duration = std::time::Instant::now();
        let (store, entries) = make_entries(Vec::new(), Some(vec!["**/*.js"]), THREEJS_PATH.to_path_buf(), None);
        println!("Elapsed: {}ms", duration.elapsed().as_millis());
        println!("Processed files: {}", store.len());
        assert_eq!(store.len() > 0, true);
    }

    #[test]
    fn make_user_file_relative_path_with_ext() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("relative_w_ext.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_path_without_ext() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("relative_wo_ext.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_path_with_index() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("relative_w_index.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("c/index.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_parent() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("c/relative_parent.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_project_path() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("project_path.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("c/index.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_tree() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("x.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 2 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        let dep_1_1_path_str = &res.deps[1];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("y.js").to_str().unwrap());
        assert_eq!(dep_1_1_path_str, PROJECT_A_PATH.join("z.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_multiple() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("many.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 2 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        let dep_2_path_str = &res.deps[1];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("z.js").to_str().unwrap());
        assert_eq!(dep_2_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_export() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("export.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_require() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("require.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_dyn_import() {
        let store = DashMap::new();
        let mut path = PROJECT_A_PATH.join("dyn_import.js");

        let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }
}