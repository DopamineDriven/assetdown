//! Workbooks made from fragments of XML held in memory, for the tests of what is read from more
//! than one part.

use std::collections::HashMap;

use ooxmldown::{DocProps, PackageError, Relationship, rels_path};

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

  /// The bytes of a file holding these parts, its main part of the content type `main_type`.
  pub fn package(&self, main_type: &str) -> Vec<u8> {
    let types = format!(
      r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/{MAIN}" ContentType="{main_type}"/></Types>"#
    );
    let listed = |relationships: &[Relationship]| {
      let each: String = relationships
        .iter()
        .map(|r| {
          let mode = if r.external {
            r#" TargetMode="External""#
          } else {
            ""
          };
          let root = if r.external { "" } else { "/" };
          format!(
            r#"<Relationship Id="{}" Type="{}" Target="{root}{}"{mode}/>"#,
            r.id, r.kind, r.target
          )
        })
        .collect();
      format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{each}</Relationships>"#
      )
    };
    let main = Relationship {
      id: "rId1".to_string(),
      kind: format!("{RELATIONSHIPS}/officeDocument"),
      target: MAIN.to_string(),
      external: false,
    };
    let mut entries = vec![
      ("[Content_Types].xml".to_string(), types.into_bytes()),
      ("_rels/.rels".to_string(), listed(&[main]).into_bytes()),
    ];
    let mut paths: Vec<&String> = self.parts.keys().collect();
    paths.sort();
    for path in paths {
      entries.push((path.clone(), self.parts[path].clone()));
      // A part with nothing to point at still has its list, as `Parts` gives one when asked.
      let relationships = self
        .relationships
        .get(path)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
      entries.push((rels_path(path), listed(relationships).into_bytes()));
    }
    stored_zip(&entries)
  }
}

/// The checksum a zip keeps of an entry's bytes.
fn crc32(bytes: &[u8]) -> u32 {
  let mut crc = !0u32;
  for byte in bytes {
    crc ^= u32::from(*byte);
    for _ in 0..8 {
      crc = if crc & 1 == 1 {
        (crc >> 1) ^ 0xEDB8_8320
      } else {
        crc >> 1
      };
    }
  }
  !crc
}

/// A zip of `entries`, each a name and its bytes, kept as they are without compression.
fn stored_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
  let le16 = |value: usize| u16::try_from(value).unwrap().to_le_bytes();
  let le32 = |value: usize| u32::try_from(value).unwrap().to_le_bytes();
  let mut file = Vec::new();
  let mut directory = Vec::new();
  for (name, data) in entries {
    let offset = file.len();
    // What an entry's own header and its line in the directory both state: version, flags,
    // method, time, date, checksum, the two sizes, and the lengths of the name and the extra
    // field.
    let mut stated = vec![20, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    stated.extend_from_slice(&crc32(data).to_le_bytes());
    stated.extend_from_slice(&le32(data.len()));
    stated.extend_from_slice(&le32(data.len()));
    stated.extend_from_slice(&le16(name.len()));
    stated.extend_from_slice(&[0, 0]);
    file.extend_from_slice(b"PK\x03\x04");
    file.extend_from_slice(&stated);
    file.extend_from_slice(name.as_bytes());
    file.extend_from_slice(data);
    // The directory's line adds the version that wrote the entry in front, and after the lengths
    // a comment's length, the disk, two sets of attributes, and where the entry starts.
    directory.extend_from_slice(b"PK\x01\x02");
    directory.extend_from_slice(&[20, 0]);
    directory.extend_from_slice(&stated);
    directory.extend_from_slice(&[0; 10]);
    directory.extend_from_slice(&le32(offset));
    directory.extend_from_slice(name.as_bytes());
  }
  let start = file.len();
  file.extend_from_slice(&directory);
  file.extend_from_slice(b"PK\x05\x06");
  file.extend_from_slice(&[0; 4]);
  file.extend_from_slice(&le16(entries.len()));
  file.extend_from_slice(&le16(entries.len()));
  file.extend_from_slice(&le32(directory.len()));
  file.extend_from_slice(&le32(start));
  file.extend_from_slice(&[0, 0]);
  file
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
