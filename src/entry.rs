use std::path::{Path, PathBuf};
use std::fs::*;
use std::ops::Deref;
use lazy_static::lazy_static;
use napi::ValueType::String;
use regex::Regex;
use rayon::prelude::*;
use path_clean::{clean, PathClean};

#[derive(Debug)]
pub struct UserFile {
    path: PathBuf,
    deps: Vec<UserFile>,
}

pub fn make_entries(entry_paths: Vec<&Path>, project_path: &Path) -> Vec<UserFile> {
    let entries = entry_paths.par_iter().map(|p| make_user_file(&PathBuf::from(p), project_path)).collect();
    entries
}

lazy_static! {
    static ref NAMED_MODULE_RE: Regex = Regex::new(r#"import\s+.+from\s+['"]([([\.\~]/)|(\.\./)].+)['"]"#).unwrap();
    static ref UNNAMED_MODULE_RE: Regex = Regex::new(r#"import\s+['"]([([\.\~]/)|(\.\./)].+)['"]"#).unwrap();
}
fn make_user_file(entry_path: &PathBuf, project_path: &Path) -> UserFile {
    let mut entry = UserFile {
        path: PathBuf::from(entry_path),
        deps: Vec::new()
    };

    // Scan file for imports
    let content = read_to_string(entry_path).expect(&("Couldn't read file: ".to_owned() + entry_path.to_str().unwrap()));
    for cap in NAMED_MODULE_RE.captures_iter(&content).chain(UNNAMED_MODULE_RE.captures_iter(&content)) {
        let source =  &cap[1];
        let mut path_buf = if source.starts_with("./") || source.starts_with("../") {
            let dir = entry_path.parent().unwrap();
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
        entry.deps.push(make_user_file(&path_buf, project_path));
    }

    entry
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
                        return Some(PathBuf::from(entry.path()))
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
                        return Some(PathBuf::from(entry.path()))
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
    use lazy_static::lazy_static;
    use crate::entry::{make_user_file, resolve_with_extension};

    lazy_static! {
        static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
        static ref PROJECT_A_PATH: PathBuf = CWD.join("tests/fixtures/project_a");
    }

    #[test]
    fn test_resolve_with_extension() {
        let mut path = CWD.join("tests/fixtures/project_a/b");

        let res = resolve_with_extension(&path).unwrap();
        assert_eq!(res.to_str(), CWD.join("tests/fixtures/project_a/b.js").to_str());
    }

    #[test]
    fn make_user_file_relative_path_with_ext() {
        let mut path = PROJECT_A_PATH.join("relative_w_ext.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_path_without_ext() {
        let mut path = PROJECT_A_PATH.join("relative_wo_ext.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_path_with_index() {
        let mut path = PROJECT_A_PATH.join("relative_w_index.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("c/index.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_relative_parent() {
        let mut path = PROJECT_A_PATH.join("c/relative_parent.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_project_path() {
        let mut path = PROJECT_A_PATH.join("project_path.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("c/index.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_tree() {
        let mut path = PROJECT_A_PATH.join("x.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        let dep_1_1_path_str = res.deps[0].deps[0].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("y.js").to_str().unwrap());
        assert_eq!(dep_1_1_path_str, PROJECT_A_PATH.join("z.js").to_str().unwrap());
    }

    #[test]
    fn make_user_file_multiple() {
        let mut path = PROJECT_A_PATH.join("many.js");

        let res = make_user_file(&path, PROJECT_A_PATH.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 2 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        let dep_2_path_str = res.deps[1].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PROJECT_A_PATH.join("z.js").to_str().unwrap());
        assert_eq!(dep_2_path_str, PROJECT_A_PATH.join("b.js").to_str().unwrap());
    }
}