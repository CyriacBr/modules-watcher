use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use glob::glob;
use memoize::memoize;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::*;
use std::path::{Component, Path, PathBuf};

#[path = "./path_clean.rs"]
mod path_clean;

use crate::parser::{parse_deps, ParseConditions};
use path_clean::*;

#[derive(Debug)]
pub struct FileItem {
  pub path: PathBuf,
  pub deps: HashSet<String>,
}

#[napi(object)]
pub struct NapiFileItem {
  pub path: String,
  pub deps: Vec<String>,
}

impl FileItem {
  pub fn clone_item(&self) -> FileItem {
    FileItem {
      path: PathBuf::from(&self.path),
      deps: self.deps.iter().map(String::from).collect(),
    }
  }

  pub fn get_entries(&self, store: &DashMap<String, FileItem>) -> Vec<String> {
    let res: Vec<String> = store
      .iter()
      .filter(|item| {
        for dep in &item.deps {
          if dep.eq(&self.path.to_str().unwrap().to_string()) {
            return true;
          }
        }
        false
      })
      .map(|item| item.path.to_str().unwrap().to_string())
      .collect();

    if res.is_empty() {
      return vec![self.path.to_str().unwrap().to_string()];
    }
    res
  }

  pub fn to_napi(&self) -> NapiFileItem {
    NapiFileItem {
      path: self.path.to_str().unwrap().to_string(),
      deps: self.deps.iter().map(String::from).collect(),
    }
  }
}

#[derive(Clone)]
pub struct MakeEntriesOptions {
  pub supported_paths: Option<SupportedPaths>,
}

#[napi(object)]
#[derive(Clone)]
pub struct SupportedPaths {
  pub esm: Option<Vec<String>>,
  pub dyn_esm: Option<Vec<String>>,
  pub cjs: Option<Vec<String>>,
  pub css: Option<Vec<String>>,
}

pub fn make_entries(
  entry_paths: Vec<PathBuf>,
  entry_globs: Option<Vec<&str>>,
  project_path: PathBuf,
  opts: &Option<MakeEntriesOptions>,
) -> (DashMap<String, FileItem>, Vec<FileItem>) {
  let store = DashMap::new();
  let entries = make_missing_entries(entry_paths, entry_globs, project_path, &store, opts);
  (store, entries)
}

pub fn make_missing_entries(
  entry_paths: Vec<PathBuf>,
  entry_globs: Option<Vec<&str>>,
  project_path: PathBuf,
  store: &DashMap<String, FileItem>,
  opts: &Option<MakeEntriesOptions>,
) -> Vec<FileItem> {
  let mut paths: Vec<PathBuf> = entry_paths;

  for glob_str in entry_globs.unwrap_or_default() {
    let full_glob = if glob_str.starts_with('/') {
      glob_str.to_owned()
    } else {
      project_path.join(glob_str).to_str().unwrap().to_owned()
    };
    paths.extend(
      glob(&full_glob)
        .expect("Failed to read glob pattern")
        .map(|x| x.unwrap()),
    );
  }

  paths.retain(|x| !store.contains_key(x.to_str().unwrap()));

  paths.par_iter().for_each(|p| {
    make_file_item(p, &project_path, store, opts);
  });
  let entry_path_str_list: Vec<String> = paths
    .iter()
    .map(|x| x.to_str().unwrap().to_string())
    .collect();
  let entries = entry_path_str_list
    .iter()
    .map(|x| store.get(x).unwrap().clone_item())
    .collect();
  entries
}

pub fn make_file_item<'a>(
  file_path: &'a Path,
  project_path: &'a Path,
  store: &'a DashMap<String, FileItem>,
  opts: &Option<MakeEntriesOptions>,
) -> Option<Ref<'a, String, FileItem>> {
  let key = file_path.to_str().unwrap();
  if store.contains_key(key) {
    return Some(
      store
        .get(key)
        .unwrap_or_else(|| panic!("Couldn't read {} inside the store", key)),
    );
  }

  let supported_paths: SupportedPaths = {
    let mut value = match opts {
      Some(opts_val) => match &opts_val.supported_paths {
        Some(sup_paths) => sup_paths.clone(),
        _ => SupportedPaths {
          esm: None,
          dyn_esm: None,
          cjs: None,
          css: None,
        },
      },
      _ => SupportedPaths {
        esm: None,
        dyn_esm: None,
        cjs: None,
        css: None,
      },
    };
    let js_exts: Vec<String> = vec!["cjs", "esm", "js", "ts", "tsx", "jsx", "tjs", "tsm"]
      .iter()
      .map(|x| x.to_string())
      .collect();
    let style_exts: Vec<String> = vec!["css", "scss", "sass"]
      .iter()
      .map(|x| x.to_string())
      .collect();
    if value.esm.is_none() {
      value.esm = Some(
        js_exts
          .clone()
          .into_iter()
          .chain([String::from("mdx")].into_iter())
          .collect(),
      );
    }
    if value.dyn_esm.is_none() {
      value.dyn_esm = Some(js_exts.clone());
    }
    if value.cjs.is_none() {
      value.cjs = Some(js_exts);
    }
    if value.css.is_none() {
      value.css = Some(style_exts);
    }
    value
  };

  let file_ext = file_path.extension().unwrap().to_str().unwrap().to_string();
  let parse_conditions = ParseConditions {
    css: supported_paths.css.unwrap().contains(&file_ext),
    esm: supported_paths.esm.unwrap().contains(&file_ext),
    lazy_esm: supported_paths.dyn_esm.unwrap().contains(&file_ext),
    require: supported_paths.cjs.unwrap().contains(&file_ext),
  };

  if !parse_conditions.css
    && !parse_conditions.esm
    && !parse_conditions.lazy_esm
    && !parse_conditions.require
  {
    return None;
  }

  store.insert(
    key.to_string(),
    FileItem {
      path: PathBuf::from(&file_path),
      deps: HashSet::new(),
    },
  );
  let mut all_deps: HashSet<String> = HashSet::new();

  // Scan file for imports
  let content = read_to_string(&file_path)
    .unwrap_or_else(|_| panic!("Couldn't read file {} ", file_path.to_str().unwrap()));

  let imports = parse_deps(&content, parse_conditions);
  for source in imports {
    let maybe_path_buf = if source.starts_with("./") || source.starts_with("../") {
      let dir = file_path.parent().unwrap();
      Some(dir.join(source).clean())
    } else if source.starts_with("~/") {
      let transformed_path = source.replace("~/", "");
      Some(project_path.join(Path::new(&transformed_path)).clean())
    } else {
      let node_modules_path = find_node_modules_dir(project_path.to_path_buf())
        .expect("Couldn't find node_modules folder");
      resolve_node_module(&source, node_modules_path.as_path())
    };
    let mut path_buf = maybe_path_buf?;
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
    all_deps.insert(path_buf.to_str().unwrap().to_string());
    if let Some(file_ref) = make_file_item(&path_buf.clone(), project_path, store, opts) {
      all_deps.extend(file_ref.deps.clone());
    }
  }
  store
    .get_mut(key)
    .unwrap_or_else(|| panic!("Couldn't read {} inside the store", key))
    .deps = all_deps;

  Some(
    store
      .get(key)
      .unwrap_or_else(|| panic!("Couldn't read {} inside the store", key)),
  )
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

#[memoize]
pub fn find_node_modules_dir(root: PathBuf) -> Option<PathBuf> {
  let mut counter: u8 = 0;
  let mut work_fn = move || {
    if counter >= 100 {
      return None;
    }
    for entry in root.read_dir().unwrap().flatten() {
      if entry.file_name().eq("node_modules") {
        return Some(entry.path());
      }
    }
    counter += 1;
    return find_node_modules_dir(root.parent().unwrap().to_path_buf());
  };

  work_fn()
}

fn resolve_node_module(module: &str, node_modules: &Path) -> Option<PathBuf> {
  let mut module_path = PathBuf::new();
  let components = Path::new(module).components();
  let components_count = components.count();
  let root = Path::new(module).components().next().unwrap().as_os_str();
  let root_pkg_dir = node_modules.join(root);
  for (i, comp) in Path::new(module).components().into_iter().enumerate() {
    if comp == Component::RootDir {
      continue;
    }
    let root = comp.as_os_str().to_str().unwrap();
    module_path = module_path.join(root);
    let pkg_path = node_modules.join(module_path.clone()).join("package.json");
    // maybe the module is an internal node_modules, which doesn't reside inside the project
    // node_modules folder
    if !pkg_path.exists() {
      return None;
    }
    let pkg_dir = node_modules.join(module_path.clone());
    let json_content = std::fs::read(pkg_path).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&json_content).unwrap();

    // If we have "exports": "./foo.js"
    if json["exports"].is_string() {
      return Some(pkg_dir.join(json["exports"].as_str().unwrap()).clean());
    }

    // If we have "exports": {}
    if json["exports"].is_object() {
      // transforms module to a relative path
      // foo     => .
      // foo/bar => ./bar
      let relative = module.replacen(module_path.to_str().unwrap(), ".", 1);
      for (key, value) in json["exports"].as_object().unwrap().into_iter() {
        if key.eq(&relative) {
          return Some(pkg_dir.join(value.as_str().unwrap()).clean());
        }
      }
    }

    if i != (components_count - 1) {
      continue;
    }

    // If we have "main": "./foo.js"
    if json["main"].is_string() {
      return Some(root_pkg_dir.join(json["main"].as_str().unwrap()).clean());
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use crate::entry::{make_entries, make_file_item, resolve_with_extension};
  use dashmap::DashMap;
  use lazy_static::lazy_static;
  use std::path::PathBuf;
  use std::string::String;

  use super::{find_node_modules_dir, resolve_node_module};

  lazy_static! {
    static ref CWD: PathBuf = PathBuf::from(std::env::current_dir().unwrap());
    static ref PROJECT_A_PATH: PathBuf = CWD.join("tests").join("fixtures").join("project_a");
    static ref THREEJS_PATH: PathBuf = CWD.join("tests").join("fixtures").join("three_js");
  }

  #[test]
  fn test_resolve_with_extension() {
    let path = PROJECT_A_PATH.join("b");

    let res = resolve_with_extension(&path).unwrap();
    assert_eq!(
      res.to_str(),
      CWD
        .join("tests")
        .join("fixtures")
        .join("project_a")
        .join("b.js")
        .to_str()
    );
  }

  #[test]
  fn make_entries_test_no_glob() {
    let path_1 = PROJECT_A_PATH.join("relative_w_ext.js");
    let path_2 = PROJECT_A_PATH.join("y.js");
    let mut paths = Vec::new();
    paths.push(path_1);
    paths.push(path_2);

    let (_, entries) = make_entries(paths, None, PROJECT_A_PATH.to_path_buf(), &None);
    assert_eq!(entries.len(), 2 as usize);
  }

  #[test]
  fn make_entries_test_glob() {
    let (_, entries) = make_entries(
      Vec::new(),
      Some(vec!["**/relative_*.js"]),
      PROJECT_A_PATH.to_path_buf(),
      &None,
    );
    assert_eq!(entries.len(), 4 as usize);
  }

  #[test]
  fn make_entries_test_three_js() {
    let duration = std::time::Instant::now();
    let (store, _) = make_entries(
      Vec::new(),
      Some(vec!["**/*.js"]),
      THREEJS_PATH.to_path_buf(),
      &None,
    );
    println!("Elapsed: {}ms", duration.elapsed().as_millis());
    println!("Processed files: {}", store.len());
    assert_eq!(store.len() > 0, true);
  }

  #[test]
  fn make_user_file_relative_path_with_ext() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("relative_w_ext.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 1 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    let dep_1_path_str = &deps[0];
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      dep_1_path_str,
      PROJECT_A_PATH.join("b.js").to_str().unwrap()
    );
  }

  #[test]
  fn make_user_file_relative_path_without_ext() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("relative_wo_ext.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 1 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    let dep_1_path_str = &deps[0];
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      dep_1_path_str,
      PROJECT_A_PATH.join("b.js").to_str().unwrap()
    );
  }

  #[test]
  fn make_user_file_relative_path_with_index() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("relative_w_index.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 1 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    let dep_1_path_str = &deps[0];
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      dep_1_path_str,
      PROJECT_A_PATH.join("c").join("index.js").to_str().unwrap()
    );
  }

  #[test]
  fn make_user_file_relative_parent() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("c/relative_parent.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 1 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    let dep_1_path_str = &deps[0];
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      dep_1_path_str,
      PROJECT_A_PATH.join("b.js").to_str().unwrap()
    );
  }

  #[test]
  fn make_user_file_project_path() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("project_path.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 1 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    let dep_1_path_str = &deps[0];
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      dep_1_path_str,
      PROJECT_A_PATH.join("c").join("index.js").to_str().unwrap()
    );
  }

  #[test]
  fn make_user_file_tree() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("x.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 2 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      deps.contains(&PROJECT_A_PATH.join("z.js").to_str().unwrap().to_string()),
      true
    );
    assert_eq!(
      deps.contains(&PROJECT_A_PATH.join("y.js").to_str().unwrap().to_string()),
      true
    );
  }

  #[test]
  fn make_user_file_multiple() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("many.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 2 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      deps.contains(&PROJECT_A_PATH.join("z.js").to_str().unwrap().to_string()),
      true
    );
    assert_eq!(
      deps.contains(&PROJECT_A_PATH.join("b.js").to_str().unwrap().to_string()),
      true
    );
  }

  #[test]
  fn make_user_file_require() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("require.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 1 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    let dep_1_path_str = &deps[0];
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      dep_1_path_str,
      PROJECT_A_PATH.join("b.js").to_str().unwrap()
    );
  }

  #[test]
  fn make_user_file_dyn_import() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("dyn_import.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(res.deps.len(), 1 as usize);

    let deps: Vec<String> = res.deps.iter().map(String::from).collect();
    let path_str = res.path.to_str().unwrap();
    let dep_1_path_str = &deps[0];
    assert_eq!(path_str, path.to_str().unwrap());
    assert_eq!(
      dep_1_path_str,
      PROJECT_A_PATH.join("b.js").to_str().unwrap()
    );
  }

  #[test]
  fn test_find_node_modules_dir() {
    let expected = CWD.join("node_modules");
    {
      let result = find_node_modules_dir(CWD.clone()).unwrap();

      assert_eq!(result.to_str().unwrap(), expected.to_str().unwrap());
    }
    {
      let result = find_node_modules_dir(CWD.join("src")).unwrap();

      assert_eq!(result.to_str().unwrap(), expected.to_str().unwrap());
    }
  }

  #[test]
  fn test_resolve_node_modules() {
    let node_modules = CWD.join("tests/fixtures/fake_node_modules");
    {
      let result = resolve_node_module("exports_str", node_modules.as_path()).unwrap();
      assert_eq!(result, node_modules.join("exports_str/main.js"));
    }
    {
      let result = resolve_node_module("exports_obj", node_modules.as_path()).unwrap();
      assert_eq!(result, node_modules.join("exports_obj/main.js"));
    }
    {
      let result = resolve_node_module("exports_obj/a", node_modules.as_path()).unwrap();
      assert_eq!(result, node_modules.join("exports_obj/a.js"));
    }
    {
      let result = resolve_node_module("main", node_modules.as_path()).unwrap();
      assert_eq!(result, node_modules.join("main/main.js"));
    }
    {
      let result = resolve_node_module("nested/b", node_modules.as_path()).unwrap();
      assert_eq!(result, node_modules.join("nested/b.js"));
    }
    {
      let result = resolve_node_module("nested", node_modules.as_path()).unwrap();
      assert_eq!(result, node_modules.join("nested/a.js"));
    }
    {
      let result = resolve_node_module("nested/c", node_modules.as_path()).unwrap();
      assert_eq!(result, node_modules.join("nested/c.js"));
    }
  }

  #[test]
  fn make_file_item_node_module() {
    let store = DashMap::new();
    let path = PROJECT_A_PATH.join("node_module.js");

    let res = make_file_item(&path, PROJECT_A_PATH.as_path(), &store, &None).unwrap();
    assert_eq!(
      true,
      res.deps.contains(
        CWD
          .join("node_modules")
          .join("fast-glob")
          .join("out")
          .join("index.js")
          .to_str()
          .unwrap()
      )
    );
  }
}
