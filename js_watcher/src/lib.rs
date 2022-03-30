#![deny(clippy::all)]

#[path = "./utils/path_clean.rs"]
pub mod path_clean;

pub mod parser;
pub mod file_item;
pub mod watch_info;
pub mod entry;
pub mod watcher;

#[macro_use]
extern crate napi_derive;
extern crate core;
