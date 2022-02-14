use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::*;
use std::ops::Deref;
use lazy_static::lazy_static;
use regex::Regex;
use rayon::prelude::*;
use path_clean::{clean, PathClean};
use dashmap::{DashMap};
use dashmap::mapref::one::Ref;
use glob::glob;

pub struct FileItem {
    path: PathBuf,
    direct_deps: Vec<String>,
}

impl FileItem {
    pub fn clone(&self) -> FileItem {
        FileItem {
            path: PathBuf::from(&self.path),
            direct_deps: self.direct_deps.iter().map(|x| String::from(x)).collect(),
        }
    }

    pub fn get_all_deps(&self, store: &DashMap<String, FileItem>) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        self.direct_deps.iter().for_each(|d| {
            result.push(d.to_owned());
            let item_ref = store.get(d).expect(&format!("Couldn't find {} inside the store", d));
            result.extend(item_ref.get_all_deps(store).iter().map(|x| x.to_owned()));
        });
        result
    }
}

pub fn make_entries(entry_paths: Vec<PathBuf>, entry_glob: Option<&str>, project_path: PathBuf) -> (DashMap<String, FileItem>, Vec<FileItem>) {
    let store = DashMap::new();
    let mut paths: Vec<PathBuf> = entry_paths.clone();

    if let Some(glob_str) = entry_glob {
        let full_glob = if glob_str.starts_with("/") {
            glob_str.to_owned()
        } else {
            project_path.join(glob_str).to_str().unwrap().to_owned()
        };
        paths.extend(glob(&full_glob).expect("Failed to read glob pattern").map(|x| x.unwrap()));
    }
    paths.par_iter().for_each(|p| {
        make_user_file(p, &project_path, &store);
    });
    let entry_path_str_list: Vec<String> = paths.iter().map(|x| x.to_str().unwrap().to_string()).collect();
    let entries = entry_path_str_list.iter().map(|x| store.get(x).unwrap().clone()).collect();
    (store, entries)
}

lazy_static! {
    static ref NAMED_MODULE_RE: Regex = Regex::new(r#"import\s+.+from\s+['"]([([\.\~]/)|(\.\./)].+)['"]"#).unwrap();
    static ref UNNAMED_MODULE_RE: Regex = Regex::new(r#"import\s+['"]([([\.\~]/)|(\.\./)].+)['"]"#).unwrap();
}
fn make_user_file<'a>(file_path: &'a PathBuf, project_path: &'a Path, store: &'a DashMap<String, FileItem>) -> Ref<'a, String, FileItem> {
    let key = file_path.to_str().unwrap();
    if store.contains_key(key) {
        return store.get(key).unwrap();
    }

    store.insert(key.to_string(), FileItem {
        path: PathBuf::from(&file_path),
        direct_deps: Vec::new(),
    });

    // Scan file for imports
    let content = read_to_string(&file_path).expect(&("Couldn't read file: ".to_owned() + file_path.to_str().unwrap()));
    for cap in NAMED_MODULE_RE.captures_iter(&content).chain(UNNAMED_MODULE_RE.captures_iter(&content)) {
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
        else if let None = path_buf.extension() {
            if let Some(found) = resolve_with_extension(&path_buf) {
                path_buf = found;
            } else {
                panic!("Couldn't handle import: {}", path_buf.to_str().unwrap());
            }
        }
        make_user_file(&path_buf, project_path, store);
        store.get_mut(key).unwrap().direct_deps.push(path_buf.to_str().unwrap().to_string());
    }

    store.get(key).unwrap()
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
fn resolve_with_extension(path: &PathBuf) -> Option<PathBuf> {
    let file_name = path.file_stem().unwrap();
    let files = path.parent().unwrap().read_dir().unwrap();
    for file in files.into_iter() {
        if let Ok(entry) = file {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    let name = entry_path.file_stem().unwrap().to_str().unwrap();
                    if name.eq(file_name) {
                        return Some(PathBuf::from(entry.path()));
                    }
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
fn resolve_index(path: &PathBuf) -> Option<PathBuf> {
    if !path.is_dir() {
        return None;
    }
    let files = path.read_dir().unwrap();
    for file in files.into_iter() {
        if let Ok(entry) = file {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    let name = entry_path.file_stem().unwrap().to_str().unwrap();
                    if name.eq("index") {
                        return Some(PathBuf::from(entry.path()));
                    }
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
    use crate::entry::{make_entries, make_user_file, resolve_with_extension};

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

        let (store, entries) = make_entries(paths, None, PROJECT_A_PATH.to_path_buf());
        assert_eq!(entries.len(), 2 as usize);
    }

    #[test]
    fn make_entries_test_glob() {
        let (store, entries) = make_entries(Vec::new(), Some("**/relative_*.js"), PROJECT_A_PATH.to_path_buf());
        assert_eq!(entries.len(), 4 as usize);
    }

    #[test]
    fn make_entries_test_three_js() {
        let duration = std::time::Instant::now();
        let (store, entries) = make_entries(Vec::new(), Some("**/*.js"), THREEJS_PATH.to_path_buf());
        println!("Elapsed: {}ms", duration.elapsed().as_millis());
        println!("Processed files: {}", store.len());
        assert_eq!(store.len() > 0, true);
    }

    #[test]
    fn make_user_file_relative_path_with_ext() {
        let store =  DashMap::new();
        let mut path = PROJECT_A_PATH.join("relative_w_ext.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path(), &store);
        assert_eq!(res.direct_deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.direct_deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_path_without_ext() {
        let store =  DashMap::new();
        let mut path = PROJECT_A_PATH.join("relative_wo_ext.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path(), &store);
        assert_eq!(res.direct_deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.direct_deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_path_with_index() {
        let store =  DashMap::new();
        let mut path = PROJECT_A_PATH.join("relative_w_index.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path(), &store);
        assert_eq!(res.direct_deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.direct_deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("c/index.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_parent() {
        let store =  DashMap::new();
        let mut path = PROJECT_A_PATH.join("c/relative_parent.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path(), &store);
        assert_eq!(res.direct_deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.direct_deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_project_path() {
        let store =  DashMap::new();
        let mut path = PROJECT_A_PATH.join("project_path.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path(), &store);
        assert_eq!(res.direct_deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &res.direct_deps[0];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("c/index.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_tree() {
        let store =  DashMap::new();
        let mut path = PROJECT_A_PATH.join("x.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path(), &store);
        let deps = res.get_all_deps(&store);
        assert_eq!(res.direct_deps.len(), 1 as usize);
        assert_eq!(deps.len(), 2 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &deps[0];
        let dep_1_1_path_str = &deps[1];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("y.js").to_str().unwrap());
        assert_eq!(dep_1_1_path_str, PROJECT_A_PATH.join("z.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_multiple() {
        let store =  DashMap::new();
        let mut path = PROJECT_A_PATH.join("many.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path(), &store);
        let deps = res.get_all_deps(&store);
        assert_eq!(res.direct_deps.len(), 2 as usize);
        assert_eq!(deps.len(), 2 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = &deps[0];
        let dep_2_path_str = &deps[1];
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("z.js").to_str().unwrap());
        assert_eq!(dep_2_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }
}