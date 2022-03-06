#![deny(clippy::all)]

pub mod entry;
pub mod parser;
pub mod watcher;

#[macro_use]
extern crate napi_derive;
extern crate core;
