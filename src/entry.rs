use std::path::{Path, PathBuf};
use std::fs::*;
use std::ops::Deref;
use lazy_static::lazy_static;
use napi::ValueType::String;
use regex::Regex;
use rayon::prelude::*;

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
    static ref NAMED_MODULE_RE: Regex = Regex::new(r#"import\s+.+from\s+['"]([\.\~]/.+)['"]"#).unwrap();
    static ref UNNAMED_MODULE_RE: Regex = Regex::new(r#"import\s+['"]([\.\~]/.+)['"]"#).unwrap();
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
        let path_buf = if source.starts_with("./") {
            let transformed_path = source.replace("./", "");
            let dir = entry_path.parent().unwrap();
            dir.join(Path::new(&transformed_path))
        } else {
            let transformed_path = source.replace("~/", "");
            let dir = project_path.parent().unwrap();
            project_path.join(Path::new(&transformed_path))
        };
        entry.deps.push(make_user_file(&path_buf, project_path));
    }

    entry
}

fn resolve_with_extension(path: &PathBuf) {
    let files = path.parent().unwrap().read_dir().unwrap();
    for file in files.into_iter() {
        if let Ok(entry) = file {
            println!("{:?}", entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::string::String;
    use crate::entry::make_user_file;

    #[test]
    fn make_user_file_relative_path_with_ext() {
        let mut project_path = PathBuf::new()
            .join(std::env::current_dir().unwrap())
            .join("tests/fixtures/project_a");
        let mut path = PathBuf::from(&project_path)
            .join("relative_w_ext.js");

        let res = make_user_file(&path, project_path.as_path());
        println!("res: {:?}", res);
        assert_eq!(res.deps.len(), 1 as usize);

        let path_str = res.path.to_str().unwrap();
        let dep_1_path_str = res.deps[0].path.to_str().unwrap();
        assert_eq!(path_str, path.to_str().unwrap());
        assert_eq!(dep_1_path_str, PathBuf::from(&project_path)
            .join("b.js").to_str().unwrap())
    }
}