#![forbid(unsafe_code)]

use std::path::{Component, Path, PathBuf};

/// The Clean trait implements a `clean` method.
pub trait PathClean {
  fn clean(&self) -> PathBuf;
}

/// PathClean implemented for `Path`
impl PathClean for Path {
  fn clean(&self) -> PathBuf {
    clean(self)
  }
}

/// The core implementation. It performs the following, lexically:
/// 1. Reduce multiple slashes to a single slash.
/// 2. Eliminate `.` path name elements (the current directory).
/// 3. Eliminate `..` path name elements (the parent directory) and the non-`.` non-`..`, element that precedes them.
/// 4. Eliminate `..` elements that begin a rooted path, that is, replace `/..` by `/` at the beginning of a path.
/// 5. Leave intact `..` elements that begin a non-rooted path.
///
/// If the result of this process is an empty string, return the string `"."`, representing the current directory.
pub fn clean<P>(path: P) -> PathBuf
where
  P: AsRef<Path>,
{
  let mut out = Vec::new();

  for comp in path.as_ref().components() {
    match comp {
      Component::CurDir => (),
      Component::ParentDir => match out.last() {
        Some(Component::RootDir) => (),
        Some(Component::Normal(_)) => {
          out.pop();
        }
        None
        | Some(Component::CurDir)
        | Some(Component::ParentDir)
        | Some(Component::Prefix(_)) => out.push(comp),
      },
      comp => out.push(comp),
    }
  }

  if !out.is_empty() {
    out.iter().collect()
  } else {
    PathBuf::from(".")
  }
}
