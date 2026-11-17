//! Workbooks made from fragments of XML held in memory, for the tests of what is read from more
//! than one part.

use std::collections::HashMap;

use ooxmldown::{DocProps, PackageError, Relationship};

use crate::error::SpreadsheetError;
use crate::source::Source;
use crate::workbook::{Workbook, read_workbook};

const MAIN: &str = "xl/workbook.xml";
const NAMESPACES: &str = r#"xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// One sheet of a workbook under construction.
pub(crate) struct SheetSpec<'a> {
  pub name: &'a str,
  pub hidden: bool,
  /// What stands inside the sheet part's root element.
  pub inner: &'a str,
  /// The part's root element: `worksheet`, or `chartsheet` for a sheet that is only a chart.
  pub root: &'a str,
}

impl<'a> SheetSpec<'a> {
  pub fn visible(name: &'a str, inner: &'a str) -> Self {
    SheetSpec {
      name,
      hidden: false,
      inner,
      root: "worksheet",
    }
  }
}

/// The path of the `n`th sheet's part, counting from 1.
pub(crate) fn sheet_path(n: usize) -> String {
  format!("xl/worksheets/sheet{n}.xml")
}

/// The parts of a workbook, and the relationships between them.
#[derive(Default)]
pub(crate) struct Parts {
  parts: HashMap<String, Vec<u8>>,
  relationships: HashMap<String, Vec<Relationship>>,
}

impl Parts {
  /// A workbook of these sheets, in order. `defined_names` is what stands inside
  /// `definedNames`, or nothing.
  pub fn workbook(sheets: &[SheetSpec<'_>], defined_names: &str) -> Self {
    let mut parts = Parts::default();
    let mut listed = String::new();
    for (at, sheet) in sheets.iter().enumerate() {
      let path = sheet_path(at + 1);
      let id = parts.relate(MAIN, "worksheet", &path);
      let state = if sheet.hidden {
        r#" state="hidden""#
      } else {
        ""
      };
      listed.push_str(&format!(
        r#"<sheet name="{}" sheetId="{}" r:id="{id}"{state}/>"#,
        sheet.name,
        at + 1
      ));
      let xml = format!("<{0} {NAMESPACES}>{1}</{0}>", sheet.root, sheet.inner);
      parts = parts.part(&path, &xml);
    }
    let workbook = format!(
      "<workbook {NAMESPACES}><sheets>{listed}</sheets><definedNames>{defined_names}</definedNames></workbook>"
    );
    parts.part(MAIN, &workbook)
  }

  pub fn part(mut self, path: &str, xml: &str) -> Self {
    self.parts.insert(path.to_string(), xml.as_bytes().to_vec());
    self
  }

  /// A relationship of the part at `from`, of the kind the last segment of its type names
  /// (`comments`, `drawing`, `pivotTable`). Returns its id.
  pub fn relate(&mut self, from: &str, kind: &str, target: &str) -> String {
    self.add(from, kind, target, false)
  }

  /// The same, to a target outside the package.
  pub fn relate_outside(&mut self, from: &str, kind: &str, target: &str) -> String {
    self.add(from, kind, target, true)
  }

  fn add(&mut self, from: &str, kind: &str, target: &str, external: bool) -> String {
    let of_part = self.relationships.entry(from.to_string()).or_default();
    let id = format!("rId{}", of_part.len() + 1);
    of_part.push(Relationship {
      id: id.clone(),
      kind: format!("{RELATIONSHIPS}/{kind}"),
      target: target.to_string(),
      external,
    });
    id
  }

  pub fn open(mut self) -> Result<Workbook, SpreadsheetError> {
    read_workbook(&mut self, MAIN, DocProps::default())
  }
}

impl Source for Parts {
  fn part(&mut self, path: &str) -> Result<Vec<u8>, PackageError> {
    self
      .parts
      .get(path)
      .cloned()
      .ok_or_else(|| PackageError::Malformed(format!("part {path} is missing")))
  }

  fn relationships(&mut self, part: &str) -> Result<Vec<Relationship>, PackageError> {
    self
      .relationships
      .get(part)
      .cloned()
      .ok_or_else(|| PackageError::Malformed(format!("part {part} has no relationships")))
  }
}
