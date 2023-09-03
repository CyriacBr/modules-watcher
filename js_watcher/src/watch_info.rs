use std::path::PathBuf;

use napi::{
  bindgen_prelude::{Array, FromNapiValue, Object, ToNapiValue},
  sys::{napi_env, napi_value},
  Env, JsObject,
};
use notify::DebouncedEvent;

use crate::file_item::FileItem;

#[derive(Debug)]
pub struct WatchInfo {
  pub event: notify::DebouncedEvent,
  pub affected_file: String,
  pub affected_entries: Option<Vec<FileItem>>,
}

impl napi::bindgen_prelude::TypeName for WatchInfo {
  fn type_name() -> &'static str {
    "WatchInfo"
  }
  fn value_type() -> napi::ValueType {
    napi::ValueType::Object
  }
}

impl WatchInfo {
  pub fn event_to_string(&self) -> String {
    match self.event {
      DebouncedEvent::Create(_) => String::from("created"),
      DebouncedEvent::Remove(_) => String::from("deleted"),
      DebouncedEvent::Write(_) => String::from("modified"),
      DebouncedEvent::Rename(_, _) => String::from("renamed"),
      _ => String::new(),
    }
  }

  pub fn to_napi_obj(&self, env_wrapper: Env) -> napi::bindgen_prelude::Result<JsObject> {
    let mut obj = env_wrapper.create_object()?;
    let affected_file = self.affected_file.clone();
    let affected_entries: Option<Vec<FileItem>> = self
      .affected_entries
      .as_ref()
      .map(|x| x.iter().map(|i| i.clone_item()).collect());

    obj.set("event", self.event_to_string())?;
    obj.set("affectedFile", affected_file)?;
    if let Some(affected_entries) = affected_entries {
      let mut entries_arr = env_wrapper.create_array(affected_entries.len() as u32)?;
      for (i, entry) in affected_entries.iter().enumerate() {
        entries_arr.set(i as u32, entry.clone_item()).unwrap();
      }
      obj.set("affectedEntries", entries_arr)?;
    } else {
      obj.set("affectedEntries", env_wrapper.create_array(0u32))?;
    }

    Ok(obj)
  }
}

impl ToNapiValue for WatchInfo {
  unsafe fn to_napi_value(
    env: napi_env,
    val: WatchInfo,
  ) -> napi::bindgen_prelude::Result<napi_value> {
    let env_wrapper = Env::from(env);
    let obj = val.to_napi_obj(env_wrapper)?;
    Object::to_napi_value(env, obj)
  }
}

impl FromNapiValue for WatchInfo {
  unsafe fn from_napi_value(
    env: napi_env,
    napi_val: napi_value,
  ) -> napi::bindgen_prelude::Result<Self> {
    let obj = Object::from_napi_value(env, napi_val)?;
    let event: String = obj.get("event").unwrap().unwrap();
    let affected_file: String = obj.get("affectedFile").unwrap().unwrap();
    let maybe_entries_arr: Option<Array> = obj.get("affectedEntries").unwrap();

    let mut affected_entries: Option<Vec<FileItem>> = None;
    if let Some(entries_arr) = maybe_entries_arr {
      let mut arr: Vec<FileItem> = Vec::new();
      for _ in 0..entries_arr.len() {
        arr.push(entries_arr.get(0).unwrap().unwrap());
      }
      affected_entries = Some(arr);
    }

    let val = Self {
      event: match event.as_str() {
        "created" => DebouncedEvent::Create(PathBuf::from(affected_file.clone())),
        "deleted" => DebouncedEvent::Remove(PathBuf::from(affected_file.clone())),
        "modified" => DebouncedEvent::Write(PathBuf::from(affected_file.clone())),
        "renamed" => DebouncedEvent::Rename(PathBuf::from(affected_file.clone()), PathBuf::new()),
        _ => DebouncedEvent::Error(notify::Error::Generic("incorrect event type".into()), None),
      },
      affected_file,
      affected_entries,
    };
    Ok(val)
  }
}
