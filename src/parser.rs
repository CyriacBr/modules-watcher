use nom::branch::{alt};
use nom::bytes::complete::{take, take_until};
use nom::character::complete::{anychar, one_of, space1, space0};
use nom::multi::{many0, many_till, separated_list1};
use nom::{bytes::complete::tag, IResult};
use std::ops::Add;

fn parse_esm_statement(input: &str) -> IResult<&str, Vec<String>> {
  let (input, _) = tag("import")(input)?;
  let (input, _) = space1(input)?;

  fn parse_named(input: &str) -> IResult<&str, ()> {
    let (input, _) = take_until("from")(input)?;
    let (input, _) = tag("from")(input)?;
    let (input, _) = space1(input)?;

    Ok((input, ()))
  }
  let (input, _) = parse_named(input).unwrap_or((input, ()));

  let (input, _) = one_of("\"'")(input)?;
  let (input, (str_tab, _)): (&str, (Vec<&str>, char)) =
    many_till(take(1usize), one_of("\"'"))(input)?;

  let path = str_tab.join("");
  Ok((input, vec![path]))
}

fn parse_lazy_esm_statement(input: &str) -> IResult<&str, Vec<String>> {
  let (input, _) = tag("import")(input)?;
  let (input, _) = space0(input)?;
  let (input, _) = tag("(")(input)?;
  let (input, _) = space0(input)?;
  let (input, _) = one_of("\"'")(input)?;
  let (input, (str_tab, _)): (&str, (Vec<&str>, char)) =
    many_till(take(1usize), one_of("\"'"))(input)?;
  let (input, _) = space0(input)?;
  let (input, _) = tag(")")(input)?;

  let path = str_tab.join("");
  Ok((input, vec![path]))
}

fn parse_require_statement(input: &str) -> IResult<&str, Vec<String>> {
  let (input, _) = tag("require")(input)?;
  let (input, _) = space0(input)?;
  let (input, _) = tag("(")(input)?;
  let (input, _) = space0(input)?;
  let (input, _) = one_of("\"'")(input)?;
  let (input, (str_tab, _)): (&str, (Vec<&str>, char)) =
    many_till(take(1usize), one_of("\"'"))(input)?;
  let (input, _) = space0(input)?;
  let (input, _) = tag(")")(input)?;

  let path = str_tab.join("");
  Ok((input, vec![path]))
}

fn parse_css_import_statement(input: &str) -> IResult<&str, Vec<String>> {
  let (input, _) = tag("@import")(input)?;
  let (input, _) = space1(input)?;

  fn parse_literal(input: &str) -> IResult<&str, String> {
    let (input, _) = space0(input)?;
    let (input, _) = one_of("\"'")(input)?;
    let (input, (str_tab, _)): (&str, (Vec<&str>, char)) =
      many_till(take(1usize), one_of("\"'"))(input)?;

    let path = str_tab.join("");
    Ok((input, path))
  }

  fn parse_url(input: &str) -> IResult<&str, String> {
    let (input, _) = space0(input)?;
    let (input, _) = take_until("url")(input)?;
    let (input, _) = tag("url")(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = tag("(")(input)?;
    let (input, path) = parse_literal(input)?;
    let (input, _) = tag(")")(input)?;

    Ok((input, path))
  }

  let (input, paths) = separated_list1(tag(","), alt((parse_literal, parse_url)))(input)?;
  Ok((
    input,
    paths
      .into_iter()
      .map(|x| {
        if x.starts_with("./") || x.starts_with("../") {
          x
        } else {
          String::from("./").add(&x)
        }
      })
      .collect(),
  ))
}

pub fn parse_deps(input: &str) -> Vec<String> {
  let (_, res) = many0(many_till(
    anychar,
    alt((
      parse_esm_statement,
      parse_require_statement,
      parse_lazy_esm_statement,
      parse_css_import_statement,
    )),
  ))(input)
  .unwrap_or_default();

  res
    .into_iter()
    .map(|(_, path)| path)
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests {
  use crate::parser::{
    parse_deps, parse_css_import_statement, parse_esm_statement, parse_lazy_esm_statement,
    parse_require_statement,
  };

  #[test]
  fn esm_statement() {
    // single quotes
    {
      let (_, path) = parse_esm_statement("import * as foo from 'foo.js'")
        .ok()
        .unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // double quotes
    {
      let (_, path) = parse_esm_statement(r#"import * as foo from "foo.js""#)
        .ok()
        .unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // named import
    {
      let (_, path) = parse_esm_statement("import { foo } from 'foo.js'")
        .ok()
        .unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // default export
    {
      let (_, path) = parse_esm_statement("import foo from 'foo.js'")
        .ok()
        .unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // unnamed
    {
      let (_, path) = parse_esm_statement("import 'foo.js'").ok().unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
  }

  #[test]
  fn lazy_esm_statement() {
    {
      let (_, path) = parse_lazy_esm_statement("import('foo.js')").ok().unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // handle whitespaces
    {
      let (_, path) = parse_lazy_esm_statement("import ( 'foo.js' )")
        .ok()
        .unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // parser expects a whole and complete import statement
    {
      assert_eq!(true, parse_lazy_esm_statement("import('foo.js").is_err());
    }
  }

  #[test]
  fn require_statement() {
    {
      let (_, path) = parse_require_statement("require('foo.js')").ok().unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // handle whitespaces
    {
      let (_, path) = parse_require_statement("require ( 'foo.js' )")
        .ok()
        .unwrap();
      assert_eq!(path, vec!["foo.js"]);
    }
    // parser expects a whole and complete import statement
    {
      assert_eq!(true, parse_require_statement("require('foo.js").is_err());
    }
  }

  #[test]
  fn one_css_import() {
    {
      let (_, paths) = parse_css_import_statement("@import 'foo.css'")
        .ok()
        .unwrap();
      assert_eq!(paths, vec!["./foo.css"]);
    }
    // url
    {
      let (_, paths) = parse_css_import_statement("@import url('foo.css')")
        .ok()
        .unwrap();
      assert_eq!(paths, vec!["./foo.css"]);
    }
    // multiple
    {
      let (_, paths) = parse_css_import_statement(r#"@import 'foo.css', "../bar.css""#)
        .ok()
        .unwrap();
      assert_eq!(paths, vec!["./foo.css", "../bar.css"]);
    }
    // with noise
    {
      let (_, paths) = parse_css_import_statement(r#"@import "common.css" screen;"#)
        .ok()
        .unwrap();
      assert_eq!(paths, vec!["./common.css"]);
    }
  }

  #[test]
  fn test_parse_all() {
    let res = parse_deps(
      r#"require('before.js');
      blahblah
      import foo from 'foo.js'
      blabhlah
      require('bar.js')
      blahblah
      import { foo2 } from 'foo2.js'
      blahblah
      require('baz.js');
      blah
      import("foo3.js")
      blah
      @import "style.css", "style2.scss"
      blahblah
      require('end.js')
    "#,
    );

    assert_eq!(
      res,
      vec![
        "before.js",
        "foo.js",
        "bar.js",
        "foo2.js",
        "baz.js",
        "foo3.js",
        "./style.css",
        "./style2.scss",
        "end.js"
      ]
    )
  }
}
