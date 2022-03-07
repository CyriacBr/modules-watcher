#![deny(clippy::all)]

pub mod parser;
pub mod entry;
pub mod watcher;

#[macro_use]
extern crate napi_derive;
extern crate core;
