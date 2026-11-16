//! The two property parts an Office package may carry: `docProps/core.xml`, which says who wrote
//! the document and when, and `docProps/app.xml`, which holds what the application recorded when
//! it last saved: its own name, and counts of pages and words.

use roxmltree::Document;

use crate::error::PackageError;
use crate::package::{Package, parse_xml};

/// What the property parts say. Every field is absent when the package does not state it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocProps {
  pub title: Option<String>,
  pub subject: Option<String>,
  /// The author, as `dc:creator`.
  pub creator: Option<String>,
  /// As written: one string, usually separated by commas or semicolons.
  pub keywords: Option<String>,
  pub description: Option<String>,
  pub category: Option<String>,
  pub language: Option<String>,
  pub last_modified_by: Option<String>,
  pub revision: Option<String>,
  /// W3CDTF, as written: `2026-11-15T09:30:00Z`.
  pub created: Option<String>,
  pub modified: Option<String>,
  pub application: Option<String>,
  pub app_version: Option<String>,
  pub company: Option<String>,
  pub template: Option<String>,
  /// Counts as the application last recorded them; nothing here is counted from the content.
  pub pages: Option<u32>,
  pub words: Option<u32>,
  pub characters: Option<u32>,
  pub characters_with_spaces: Option<u32>,
  pub lines: Option<u32>,
  pub paragraphs: Option<u32>,
  /// Editing time in minutes.
  pub total_time: Option<u32>,
}

/// The text of the first element named `name`, whatever its namespace, when it is not empty.
fn text_of(doc: &Document<'_>, name: &str) -> Option<String> {
  doc
    .descendants()
    .find(|n| n.is_element() && n.tag_name().name() == name)
    .and_then(|n| n.text())
    .map(str::trim)
    .filter(|text| !text.is_empty())
    .map(str::to_owned)
}

fn number_of(doc: &Document<'_>, name: &str) -> Option<u32> {
  text_of(doc, name).and_then(|text| text.parse().ok())
}

impl Package {
  /// The document properties, found through the package relationships.
  ///
  /// The property parts are optional and secondary, so a part that is missing or is not readable
  /// XML leaves its fields absent. A limit error is still an error.
  pub fn doc_props(&mut self) -> Result<DocProps, PackageError> {
    let relationships = self.relationships("_rels/.rels")?;
    let target = |suffix: &str| {
      relationships
        .iter()
        .find(|r| !r.external && r.kind.ends_with(suffix))
        .map(|r| r.target.clone())
    };
    let mut props = DocProps::default();
    if let Some(bytes) = self.optional_part(target("/metadata/core-properties"))?
      && let Ok(doc) = parse_xml(&bytes)
    {
      props.title = text_of(&doc, "title");
      props.subject = text_of(&doc, "subject");
      props.creator = text_of(&doc, "creator");
      props.keywords = text_of(&doc, "keywords");
      props.description = text_of(&doc, "description");
      props.category = text_of(&doc, "category");
      props.language = text_of(&doc, "language");
      props.last_modified_by = text_of(&doc, "lastModifiedBy");
      props.revision = text_of(&doc, "revision");
      props.created = text_of(&doc, "created");
      props.modified = text_of(&doc, "modified");
    }
    if let Some(bytes) = self.optional_part(target("/extended-properties"))?
      && let Ok(doc) = parse_xml(&bytes)
    {
      props.application = text_of(&doc, "Application");
      props.app_version = text_of(&doc, "AppVersion");
      props.company = text_of(&doc, "Company");
      props.template = text_of(&doc, "Template");
      props.pages = number_of(&doc, "Pages");
      props.words = number_of(&doc, "Words");
      props.characters = number_of(&doc, "Characters");
      props.characters_with_spaces = number_of(&doc, "CharactersWithSpaces");
      props.lines = number_of(&doc, "Lines");
      props.paragraphs = number_of(&doc, "Paragraphs");
      props.total_time = number_of(&doc, "TotalTime");
    }
    Ok(props)
  }

  /// A part that may be absent: `None` when no path was given or the part is missing or
  /// unreadable, the limit error when reading it passed a cap.
  fn optional_part(&mut self, path: Option<String>) -> Result<Option<Vec<u8>>, PackageError> {
    let Some(path) = path else {
      return Ok(None);
    };
    match self.part(&path) {
      Ok(bytes) => Ok(Some(bytes)),
      Err(PackageError::Malformed(_)) => Ok(None),
      Err(limit) => Err(limit),
    }
  }
}
