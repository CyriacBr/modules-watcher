#![deny(clippy::all)]

mod watcher;
mod entry;

#[macro_use]
extern crate napi_derive;

#[napi]
fn sum(a: i32, b: i32) -> i32 {
  a + b
}
