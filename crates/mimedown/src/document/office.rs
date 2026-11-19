//! Office files: a zip of parts, one of which is the document and says in its content type
//! exactly what kind it is. What the file says of itself (its title, its author, when it was
//! made) is in two small parts of its own, which `ooxmldown` reads. The counts are the
//! parsers', read when the `office` feature is on.

use ooxmldown::{DocProps, Package, PackageKind};

use super::{DocumentDetail, DocumentFacts};
use crate::image::Reading;
use crate::media_type::MediaType;
use crate::source::Source;

/// What only an Office file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficeDetail {
  /// The file can carry macros. They are never read or run.
  pub macro_enabled: bool,
  /// The file is a template for others, or a deck saved to open straight into its show.
  pub template: bool,
  /// The application that wrote the file, as it names itself.
  pub application: Option<String>,
}

/// The exact type of an Office package: what the part that lists its content types says its
/// main part is. `main` is that part's name, with the slash a part name begins with. None
/// when the list does not read, does not name the part, or names it as none of these.
pub(crate) fn exact(content_types: &[u8], main: &str) -> Option<MediaType> {
  let listed = ooxmldown::parse_xml(content_types).ok()?;
  let content_type = listed
    .root_element()
    .children()
    .filter(|node| node.has_tag_name((CONTENT_TYPES, "Override")))
    .find(|node| {
      node
        .attribute("PartName")
        .is_some_and(|part| part.eq_ignore_ascii_case(main))
    })?
    .attribute("ContentType")?;
  Some(match PackageKind::of_main_type(content_type) {
    PackageKind::WordDocument => MediaType::Docx,
    PackageKind::MacroDocument => MediaType::Docm,
    PackageKind::TemplateDocument => MediaType::Dotx,
    PackageKind::Workbook | PackageKind::StrictWorkbook => MediaType::Xlsx,
    PackageKind::MacroWorkbook => MediaType::Xlsm,
    PackageKind::TemplateWorkbook => MediaType::Xltx,
    PackageKind::Presentation => MediaType::Pptx,
    PackageKind::MacroPresentation => MediaType::Pptm,
    PackageKind::TemplatePresentation => MediaType::Potx,
    PackageKind::SlideShow => MediaType::Ppsx,
    PackageKind::Other => return None,
  })
}

/// The namespace of the part that lists a package's content types.
const CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

/// How many characters of text a preview holds.
#[cfg(feature = "office")]
const PREVIEW_CHARACTERS: usize = 500;

#[cfg(feature = "office")]
fn preview(text: &str) -> Option<String> {
  let start: String = text.trim_start().chars().take(PREVIEW_CHARACTERS).collect();
  let start = start.trim_end();
  (!start.is_empty()).then(|| start.to_string())
}

pub(crate) fn read(
  media_type: MediaType,
  source: &Source<'_>,
  reading: &mut Reading,
) -> Option<DocumentFacts> {
  // The parts of a package are compressed and lie wherever the zip put them.
  let Some(bytes) = source.whole_file() else {
    reading
      .needs
      .get_or_insert(source.rest_from(source.head_len()));
    return None;
  };
  let props = match Package::open(bytes.to_vec()).and_then(|mut package| package.doc_props()) {
    Ok(props) => props,
    Err(error) => {
      reading.warn(format!("the file's properties could not be read: {error}"));
      DocProps::default()
    }
  };
  let deck = matches!(
    media_type,
    MediaType::Pptx | MediaType::Pptm | MediaType::Potx | MediaType::Ppsx
  );
  let detail = DocumentDetail::Office(OfficeDetail {
    macro_enabled: matches!(
      media_type,
      MediaType::Docm | MediaType::Xlsm | MediaType::Pptm
    ),
    template: matches!(
      media_type,
      MediaType::Dotx | MediaType::Xltx | MediaType::Potx | MediaType::Ppsx
    ),
    application: props.application,
  });
  // The counts here are what the application last wrote down, and may be out of date. A
  // parser's own count replaces them when there is one.
  let mut facts = DocumentFacts {
    page_count: if deck { props.slides } else { props.pages },
    word_count: props.words.map(u64::from),
    language: props.language,
    title: props.title,
    author: props.creator,
    subject: props.subject,
    keywords: props
      .keywords
      .iter()
      .flat_map(|written| written.split([',', ';']))
      .map(str::trim)
      .filter(|keyword| !keyword.is_empty())
      .map(str::to_string)
      .collect(),
    created_date: props.created,
    modified_date: props.modified,
    ..DocumentFacts::unread(detail)
  };
  counted(media_type, bytes, &mut facts, reading);
  Some(facts)
}

/// Without the parsers, the file's own properties are all that is read.
#[cfg(not(feature = "office"))]
fn counted(
  _media_type: MediaType,
  _bytes: &[u8],
  _facts: &mut DocumentFacts,
  _reading: &mut Reading,
) {
}

/// The counts, from the parser that reads the type.
#[cfg(feature = "office")]
fn counted(media_type: MediaType, bytes: &[u8], facts: &mut DocumentFacts, reading: &mut Reading) {
  let paper = |size: &dyn std::fmt::Debug| Some(format!("{size:?}").to_ascii_uppercase());
  match media_type {
    MediaType::Docx | MediaType::Docm | MediaType::Dotx => {
      match docxdown::Document::open(bytes.to_vec()) {
        Ok(document) => {
          let meta = document.metadata();
          facts.page_count = Some(meta.page_count);
          facts.word_count = Some(u64::from(meta.word_count));
          facts.image_count = Some(meta.image_count);
          facts.annot_count = Some(meta.annotation_count);
          facts.page_size_uniform = Some(meta.page_size_uniform);
          if let Some(section) = meta.sections.first() {
            facts.page_width = section.width;
            facts.page_height = section.height;
            facts.paper_size = paper(&section.paper_size);
          }
          facts.language = document.language.clone().or(facts.language.take());
          facts.text_preview = preview(&document.text());
        }
        Err(error) => reading.warn(format!("the document could not be read: {error}")),
      }
    }
    MediaType::Pptx | MediaType::Pptm | MediaType::Potx | MediaType::Ppsx => {
      match pptxdown::Presentation::open(bytes.to_vec()) {
        Ok(deck) => {
          let meta = deck.metadata();
          facts.page_count = Some(meta.slide_count);
          facts.word_count = Some(u64::from(meta.word_count));
          facts.image_count = Some(meta.image_count);
          facts.annot_count = Some(meta.annotation_count);
          facts.page_width = Some(meta.width);
          facts.page_height = Some(meta.height);
          // Every slide of a deck is the same size.
          facts.page_size_uniform = Some(true);
          facts.text_preview = preview(&deck.text());
        }
        Err(error) => reading.warn(format!("the presentation could not be read: {error}")),
      }
    }
    _ => match xlsxdown::Workbook::open(bytes.to_vec()) {
      Ok(workbook) => {
        let meta = workbook.metadata();
        facts.page_count = Some(meta.sheet_count);
        facts.word_count = Some(u64::from(meta.word_count));
        facts.image_count = Some(meta.image_count);
        facts.annot_count = Some(meta.annotation_count);
        // A workbook has no text of its own to begin with: the first cells that hold text.
        let cells: String = workbook
          .sheets
          .iter()
          .flat_map(|sheet| sheet.cells.values())
          .filter_map(|cell| match &cell.value {
            xlsxdown::CellValue::Text(text) => Some(text.as_str()),
            _ => None,
          })
          .flat_map(|text| text.chars().chain(std::iter::once(' ')))
          .take(PREVIEW_CHARACTERS)
          .collect();
        facts.text_preview = preview(&cells);
      }
      Err(error) => reading.warn(format!("the workbook could not be read: {error}")),
    },
  }
}

#[cfg(test)]
mod tests;
