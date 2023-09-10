#![deny(clippy::all)]

#[path = "./utils/path_clean.rs"]
pub mod path_clean;

pub mod entry;
pub mod file_item;
pub mod parser;
pub mod watcher;

#[macro_use]
extern crate napi_derive;
extern crate core;
