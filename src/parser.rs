use nom::branch::alt;
use nom::bytes::complete::{take, take_until};
use nom::character::complete::{anychar, one_of, space0, space1};
use nom::combinator::{cond, verify};
use nom::multi::{many0, many_till, separated_list1};
use nom::{bytes::complete::tag, IResult};
use std::ops::Add;

#[derive(Debug, PartialEq)]
/// Import type with their import names
pub enum ImportDep {
  ESM(String),
  REQUIRE(String),
  CSS(String)
}

fn parse_esm_statement(input: &str) -> IResult<&str, Vec<ImportDep>> {
  // let (input, _) = nom::bytes::streaming::take_until("import")(input)?;
  let (input, _) = tag("import")(input)?;
  let (input, _) = space1(input)?;

  fn parse_named(input: &str) -> IResult<&str, ()> {
    let (input, _) = take_until("from")(input)?;
    let (input, _) = tag("from")(input)?;
    let (input, _) = space1(input)?;

    Ok((input, ()))
  }

  if one_of::<_, _, (&str, nom::error::ErrorKind)>("\"'")(input).is_ok() {
    let (input, _) = one_of("\"'")(input)?;
    let (input, (str_tab, _)): (&str, (Vec<&str>, char)) =
      many_till(take(1usize), one_of("\"'"))(input)?;

    let path = str_tab.join("");
    return Ok((input, vec![ImportDep::ESM(path)]));
  }

  let (input, _) = parse_named(input).unwrap_or((input, ()));
  let (input, _) = one_of("\"'")(input)?;
  let (input, (str_tab, _)): (&str, (Vec<&str>, char)) =
    many_till(take(1usize), one_of("\"'"))(input)?;

  let path = str_tab.join("");
  Ok((input, vec![ImportDep::ESM(path)]))
}

fn parse_lazy_esm_statement(input: &str) -> IResult<&str, Vec<ImportDep>> {
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
  Ok((input, vec![ImportDep::ESM(path)]))
}

fn parse_require_statement(input: &str) -> IResult<&str, Vec<ImportDep>> {
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
  Ok((input, vec![ImportDep::REQUIRE(path)]))
}

fn parse_css_import_statement(input: &str) -> IResult<&str, Vec<ImportDep>> {
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
          ImportDep::CSS(x)
        } else {
          ImportDep::CSS(String::from("./").add(&x))
        }
      })
      .collect(),
  ))
}

pub struct ParseConditions {
  pub esm: bool,
  pub require: bool,
  pub lazy_esm: bool,
  pub css: bool,
}

pub fn parse_deps(input: &str, conditions: ParseConditions) -> Vec<ImportDep> {
  let (_, res) = many0(many_till(
    anychar,
    alt((
      verify(cond(conditions.esm, parse_esm_statement), |x| x.is_some()),
      verify(cond(conditions.require, parse_require_statement), |x| {
        x.is_some()
      }),
      verify(cond(conditions.lazy_esm, parse_lazy_esm_statement), |x| {
        x.is_some()
      }),
      verify(cond(conditions.css, parse_css_import_statement), |x| {
        x.is_some()
      }),
    )),
  ))(input)
  .unwrap_or_default();

  res
    .into_iter()
    .map(|(_, path)| path)
    .flatten()
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests {
  use crate::parser::{
    parse_css_import_statement, parse_deps, parse_esm_statement, parse_lazy_esm_statement,
    parse_require_statement,ImportDep
  };

  #[test]
  fn esm_statement() {
    // single quotes
    {
      let (_, path) = parse_esm_statement("import * as foo from 'foo.js'")
        .ok()
        .unwrap();
      assert_eq!(path, vec![ImportDep::ESM("foo.js".to_string())]);
    }
    // double quotes
    {
      let (_, path) = parse_esm_statement(r#"import * as foo from "foo.js""#)
        .ok()
        .unwrap();
      assert_eq!(path, vec![ImportDep::ESM("foo.js".to_string())]);
    }
    // named import
    {
      let (_, path) = parse_esm_statement("import { foo } from 'foo.js'")
        .ok()
        .unwrap();
      assert_eq!(path, vec![ImportDep::ESM("foo.js".to_string())]);
    }
    // default export
    {
      let (_, path) = parse_esm_statement("import foo from 'foo.js'")
        .ok()
        .unwrap();
      assert_eq!(path, vec![ImportDep::ESM("foo.js".to_string())]);
    }
    // unnamed
    {
      let (_, path) = parse_esm_statement("import 'foo.js'").ok().unwrap();
      assert_eq!(path, vec![ImportDep::ESM("foo.js".to_string())]);
    }
  }

  #[test]
  fn lazy_esm_statement() {
    {
      let (_, path) = parse_lazy_esm_statement("import('foo.js')").ok().unwrap();
      assert_eq!(path, vec![ImportDep::ESM("foo.js".to_string())]);
    }
    // handle whitespaces
    {
      let (_, path) = parse_lazy_esm_statement("import ( 'foo.js' )")
        .ok()
        .unwrap();
      assert_eq!(path, vec![ImportDep::ESM("foo.js".to_string())]);
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
      assert_eq!(path, vec![ImportDep::REQUIRE("foo.js".to_string())]);
    }
    // handle whitespaces
    {
      let (_, path) = parse_require_statement("require ( 'foo.js' )")
        .ok()
        .unwrap();
      assert_eq!(path, vec![ImportDep::REQUIRE("foo.js".to_string())]);
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
      assert_eq!(paths, vec![ImportDep::CSS("./foo.css".to_string())]);
    }
    // url
    {
      let (_, paths) = parse_css_import_statement("@import url('foo.css')")
        .ok()
        .unwrap();
      assert_eq!(paths, vec![ImportDep::CSS("./foo.css".to_string())]);
    }
    // multiple
    {
      let (_, paths) = parse_css_import_statement(r#"@import 'foo.css', "../bar.css""#)
        .ok()
        .unwrap();
      assert_eq!(paths, vec![ImportDep::CSS("./foo.css".to_string()), ImportDep::CSS("../bar.css".to_string())]);
    }
    // with noise
    {
      let (_, paths) = parse_css_import_statement(r#"@import "common.css" screen;"#)
        .ok()
        .unwrap();
      assert_eq!(paths, vec![ImportDep::CSS("./common.css".to_string())]);
    }
  }

  #[test]
  fn test_parse_all() {
    {
      let res = parse_deps(
        r#"require('before.js');
        blahblah
        import foo from 'foo.js'
        blabhlah
        require('bar.js')
        blahblah
        import { foo2 } from 'foo2.js'
        import './foo3.js';
        blahblah
        require('baz.js');
        blah
        import("foo3.js")
        blah
        @import "style.css", "style2.scss"
        blahblah
        require('end.js')
      "#,
        crate::parser::ParseConditions {
          esm: true,
          require: true,
          lazy_esm: true,
          css: true,
        },
      );

      assert_eq!(
        res,
        vec![
          ImportDep::REQUIRE("before.js".to_string()),
          ImportDep::ESM("foo.js".to_string()),
          ImportDep::REQUIRE("bar.js".to_string()),
          ImportDep::ESM("foo2.js".to_string()),
          ImportDep::ESM("./foo3.js".to_string()),
          ImportDep::REQUIRE("baz.js".to_string()),
          ImportDep::ESM("foo3.js".to_string()),
          ImportDep::CSS("./style.css".to_string()),
          ImportDep::CSS("./style2.scss".to_string()),
          ImportDep::REQUIRE("end.js".to_string())
        ]
      );
    }

    {
      let res = parse_deps(
        r#"import * as B from './b.js';
        import { FILE_1 } from './file1.js';
        import file2 from './file2.js';
        import './file3.js';
        import { FILE_4 } from './file4';
        import { FILE_5 } from './file5';
      "#,
        crate::parser::ParseConditions {
          esm: true,
          require: false,
          lazy_esm: false,
          css: false,
        },
      );

      assert_eq!(
        res,
        vec![
          ImportDep::ESM("./b.js".to_string()),
          ImportDep::ESM("./file1.js".to_string()),
          ImportDep::ESM("./file2.js".to_string()),
          ImportDep::ESM("./file3.js".to_string()),
          ImportDep::ESM("./file4".to_string()),
          ImportDep::ESM("./file5".to_string()),
        ]
      );
    }
  }
}
