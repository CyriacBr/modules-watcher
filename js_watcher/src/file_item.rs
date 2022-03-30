use std::{collections::HashSet, path::PathBuf};

use dashmap::DashMap;
use napi::{
  bindgen_prelude::{Array, FromNapiValue, Object, ToNapiValue},
  sys::{napi_env, napi_value},
  Env,
};

#[derive(Debug)]
pub struct FileItem {
  pub path: PathBuf,
  pub deps: HashSet<String>,
}

impl napi::bindgen_prelude::TypeName for FileItem {
  fn type_name() -> &'static str {
    "FileItem"
  }
  fn value_type() -> napi::ValueType {
    napi::ValueType::Object
  }
}

impl ToNapiValue for FileItem {
  unsafe fn to_napi_value(
    env: napi_env,
    val: FileItem,
  ) -> napi::bindgen_prelude::Result<napi_value> {
    let env_wrapper = Env::from(env);
    let mut obj = env_wrapper.create_object()?;
    let Self { path, deps } = val;
    obj.set("path", path.to_str().unwrap())?;
    let mut deps_arr = env_wrapper.create_array(deps.len() as u32)?;
    for (i, dep) in deps.iter().enumerate() {
      deps_arr.set(i as u32, dep.clone()).unwrap();
    }
    obj.set("deps", deps_arr)?;
    Object::to_napi_value(env, obj)
  }
}

impl FromNapiValue for FileItem {
  unsafe fn from_napi_value(
    env: napi_env,
    napi_val: napi_value,
  ) -> napi::bindgen_prelude::Result<Self> {
    let obj = Object::from_napi_value(env, napi_val)?;
    let path_str: String = obj.get("path").unwrap().unwrap();
    let deps_arr: Array = obj.get("deps").unwrap().unwrap();

    let mut deps: HashSet<String> = HashSet::new();
    for _ in 0..deps_arr.len() {
      deps.insert(deps_arr.get(0).unwrap().unwrap());
    }

    let val = Self {
      path: PathBuf::from(path_str),
      deps,
    };
    Ok(val)
  }
}

impl FileItem {
  pub fn clone_item(&self) -> FileItem {
    FileItem {
      path: PathBuf::from(&self.path),
      deps: self.deps.iter().map(String::from).collect(),
    }
  }

  pub fn get_usage(&self, store: &DashMap<String, FileItem>) -> Vec<String> {
    let self_path = self.path.to_str().unwrap().to_string();
    let res: Vec<String> = store
      .iter()
      .filter(|item| {
        if item.path.to_str().unwrap() == self_path {
          return true;
        }
        for dep in &item.deps {
          if dep.eq(&self_path) {
            return true;
          }
        }
        false
      })
      .map(|item| item.value().path.to_str().unwrap().to_string())
      .collect();

    res
  }
}
