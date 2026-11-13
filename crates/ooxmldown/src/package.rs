//! Bounded access to an OOXML package.
//!
//! A package is a zip; its parts are found through relationships (`_rels/.rels` names the main
//! part, each part's own `.rels` names what it refers to) and typed by `[Content_Types].xml`.
//! Every read is capped: the input, the entry count, each inflated part and their total.

use std::fmt;
use std::io::{Cursor, Read};

use roxmltree::{Document, Node, ParsingOptions};
use zip::ZipArchive;

use crate::error::PackageError;

/// Caps enforced on bytes actually read, never on what the zip headers claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
  /// The package as handed in, still compressed.
  pub input_bytes: usize,
  /// Entries in the zip.
  pub entries: usize,
  /// One part, inflated.
  pub part_bytes: usize,
  /// Every part read through one [`Package`], inflated; a part read twice counts twice.
  pub total_bytes: usize,
}

impl Default for Limits {
  fn default() -> Self {
    Limits {
      input_bytes: 64 << 20,
      entries: 10_000,
      part_bytes: 64 << 20,
      total_bytes: 256 << 20,
    }
  }
}

/// XML nodes a single part may hold; roxmltree's default is effectively unbounded.
pub const XML_NODES_LIMIT: u32 = 5_000_000;

/// What the main part of a package says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
  Workbook,
  MacroWorkbook,
  TemplateWorkbook,
  StrictWorkbook,
  WordDocument,
  Presentation,
  Other,
}

impl PackageKind {
  /// The kind in words. Whether a kind is supported is the caller's statement to make, not this
  /// crate's.
  pub fn description(self) -> &'static str {
    match self {
      PackageKind::Workbook => "workbook",
      PackageKind::MacroWorkbook => "macro-enabled workbook",
      PackageKind::TemplateWorkbook => "template workbook",
      PackageKind::StrictWorkbook => "Strict OOXML workbook",
      PackageKind::WordDocument => "Word document package",
      PackageKind::Presentation => "PowerPoint presentation package",
      PackageKind::Other => "zip package that is not an Office document",
    }
  }
}

/// One relationship: `target` is resolved to an absolute part path unless it is external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
  pub id: String,
  /// The relationship type URI.
  pub kind: String,
  pub target: String,
  pub external: bool,
}

const WORKBOOK_MAIN: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const MACRO_WORKBOOK_MAIN: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const TEMPLATE_MAINS: [&str; 2] = [
  "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
  "application/vnd.ms-excel.template.macroEnabled.main+xml",
];
const WORD_PREFIX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";
const PRESENTATION_PREFIX: &str = "application/vnd.openxmlformats-officedocument.presentationml";
const SML_STRICT: &str = "http://purl.oclc.org/ooxml/spreadsheetml/main";

/// Parse one part's XML under the node limit, with DTDs and external entities off (roxmltree's
/// default).
pub fn parse_xml(bytes: &[u8]) -> Result<Document<'_>, PackageError> {
  let text = std::str::from_utf8(bytes)
    .map_err(|e| PackageError::Malformed(format!("part is not UTF-8: {e}")))?;
  Document::parse_with_options(
    text,
    ParsingOptions {
      nodes_limit: XML_NODES_LIMIT,
      ..ParsingOptions::default()
    },
  )
  .map_err(|e| PackageError::Malformed(format!("part is not well-formed XML: {e}")))
}

/// The relationships part that describes `part`: `xl/workbook.xml` → `xl/_rels/workbook.xml.rels`.
/// The package itself is the empty path, described by `_rels/.rels`.
pub fn rels_path(part: &str) -> String {
  match part.rsplit_once('/') {
    Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
    None => format!("_rels/{part}.rels"),
  }
}

fn attr<'a>(node: Node<'a, 'a>, name: &str) -> &'a str {
  node.attribute(name).unwrap_or("")
}

/// The directory a `.rels` part describes: `xl/_rels/workbook.xml.rels` → `xl/`, `_rels/.rels` → ``.
fn rels_base(rels_path: &str) -> &str {
  rels_path
    .rsplit_once("_rels/")
    .map(|(dir, _)| dir)
    .unwrap_or("")
}

/// Join a relationship target onto its base and normalise `.` and `..`; a leading `/` is absolute.
fn resolve_target(base: &str, target: &str) -> String {
  let joined = match target.strip_prefix('/') {
    Some(absolute) => absolute.to_string(),
    None => format!("{base}{target}"),
  };
  let mut segments: Vec<&str> = Vec::new();
  for segment in joined.split('/') {
    match segment {
      "" | "." => {}
      ".." => {
        segments.pop();
      }
      other => segments.push(other),
    }
  }
  segments.join("/")
}

/// An open package: the zip archive plus the running count of bytes inflated so far.
pub struct Package {
  archive: ZipArchive<Cursor<Vec<u8>>>,
  limits: Limits,
  read_total: usize,
}

impl fmt::Debug for Package {
  // The archive holds the whole input; printing it would dump the file.
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Package")
      .field("entries", &self.archive.len())
      .field("limits", &self.limits)
      .field("read_total", &self.read_total)
      .finish_non_exhaustive()
  }
}

impl Package {
  pub fn open(bytes: Vec<u8>) -> Result<Package, PackageError> {
    Package::open_with_limits(bytes, Limits::default())
  }

  pub fn open_with_limits(bytes: Vec<u8>, limits: Limits) -> Result<Package, PackageError> {
    if bytes.len() > limits.input_bytes {
      return Err(PackageError::LimitExceeded(format!(
        "the package is {} bytes; the limit is {}",
        bytes.len(),
        limits.input_bytes
      )));
    }
    let archive = ZipArchive::new(Cursor::new(bytes))
      .map_err(|e| PackageError::Malformed(format!("not a readable zip package: {e}")))?;
    if archive.len() > limits.entries {
      return Err(PackageError::LimitExceeded(format!(
        "the package has {} entries; the limit is {}",
        archive.len(),
        limits.entries
      )));
    }
    Ok(Package {
      archive,
      limits,
      read_total: 0,
    })
  }

  /// One part, inflated, or a limit error once it passes the per-part or total cap.
  pub fn part(&mut self, path: &str) -> Result<Vec<u8>, PackageError> {
    let limit = self.limits.part_bytes;
    let total_limit = self.limits.total_bytes;
    let mut file = self
      .archive
      .by_name(path)
      .map_err(|_| PackageError::Malformed(format!("part {path} is missing")))?;
    // One byte past the cap is enough to know the part exceeds it.
    let read_cap = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    file
      .by_ref()
      .take(read_cap)
      .read_to_end(&mut bytes)
      .map_err(|e| PackageError::Malformed(format!("part {path} could not be read: {e}")))?;
    if bytes.len() > limit {
      return Err(PackageError::LimitExceeded(format!(
        "part {path} inflates past {limit} bytes"
      )));
    }
    self.read_total = self.read_total.saturating_add(bytes.len());
    if self.read_total > total_limit {
      return Err(PackageError::LimitExceeded(format!(
        "the package's parts inflate past {total_limit} bytes in total"
      )));
    }
    Ok(bytes)
  }

  /// The relationships a `.rels` part declares, targets resolved against the part it describes.
  pub fn relationships(&mut self, rels_path: &str) -> Result<Vec<Relationship>, PackageError> {
    let bytes = self.part(rels_path)?;
    let doc = parse_xml(&bytes)?;
    let base = rels_base(rels_path);
    Ok(
      doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Relationship")
        .map(|n| {
          let external = n.attribute("TargetMode") == Some("External");
          let target = attr(n, "Target");
          Relationship {
            id: attr(n, "Id").to_string(),
            kind: attr(n, "Type").to_string(),
            target: if external {
              target.to_string()
            } else {
              resolve_target(base, target)
            },
            external,
          }
        })
        .collect(),
    )
  }

  /// The main part named by the package relationships.
  pub fn officedocument(&mut self) -> Result<String, PackageError> {
    self
      .relationships("_rels/.rels")?
      .into_iter()
      .find(|r| !r.external && r.kind.ends_with("/officeDocument"))
      .map(|r| r.target)
      .ok_or_else(|| PackageError::Malformed("no officeDocument relationship".to_string()))
  }

  /// A part's content type from `[Content_Types].xml`: an override by name, else a default by
  /// extension.
  pub fn content_type(&mut self, part: &str) -> Result<Option<String>, PackageError> {
    let bytes = self.part("[Content_Types].xml")?;
    let doc = parse_xml(&bytes)?;
    let wanted = format!("/{part}");
    let extension = part.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let mut by_default = None;
    for n in doc.descendants().filter(|n| n.is_element()) {
      match n.tag_name().name() {
        "Override" if attr(n, "PartName") == wanted => {
          return Ok(Some(attr(n, "ContentType").to_string()));
        }
        "Default" if attr(n, "Extension").eq_ignore_ascii_case(extension) => {
          by_default = Some(attr(n, "ContentType").to_string());
        }
        _ => {}
      }
    }
    Ok(by_default)
  }

  /// What the package is, by its main part's content type and, for workbooks, its namespace.
  pub fn kind(&mut self) -> Result<PackageKind, PackageError> {
    let main = self.officedocument()?;
    let content_type = self.content_type(&main)?.unwrap_or_default();
    Ok(match content_type.as_str() {
      WORKBOOK_MAIN => {
        let bytes = self.part(&main)?;
        let doc = parse_xml(&bytes)?;
        if doc.root_element().tag_name().namespace() == Some(SML_STRICT) {
          PackageKind::StrictWorkbook
        } else {
          PackageKind::Workbook
        }
      }
      MACRO_WORKBOOK_MAIN => PackageKind::MacroWorkbook,
      t if TEMPLATE_MAINS.contains(&t) => PackageKind::TemplateWorkbook,
      t if t.starts_with(WORD_PREFIX) => PackageKind::WordDocument,
      t if t.starts_with(PRESENTATION_PREFIX) => PackageKind::Presentation,
      _ => PackageKind::Other,
    })
  }
}

#[cfg(test)]
mod tests;
