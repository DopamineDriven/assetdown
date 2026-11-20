//! An RTF file: the files embedded in it, and its pictures.
//!
//! An embedded file is an object group: its class, its data in hex, and a picture that stands
//! for it on the page. The data opens with the header OLE 1.0 put before an object, and what
//! follows the header is the object's own data, which for most classes is a compound file with
//! the embedded file in one of its streams.

use contentdown::PageLayout;
use contentdown::invoice::{InvoiceOptions, InvoiceResult, extract_from_pages};

use crate::body::{self, Block, Body, Paper, Running};
use crate::cfb;
use crate::error::RtfError;
use crate::layout;
use crate::token::{Token, Tokens};

/// No document nests its groups this deep; a file made to exhaust a reader does.
pub(crate) const MOST_DEPTH: usize = 10_000;

/// A file embedded in the document, as it was before it was embedded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embedded {
  /// The class of the object that holds it, as the program that embedded it names itself:
  /// `Outlook.FileAttach` for a file attached to a mail message, `Package` for a file dropped
  /// into a document, `Word.Document.12`, and so on.
  pub class: Option<String>,
  /// The file's name, where the object keeps one.
  pub name: Option<String>,
  /// The file. For a class this crate does not know, the object's own data as it is.
  pub bytes: Vec<u8>,
}

/// What a picture's bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureFormat {
  Png,
  Jpeg,
  /// An enhanced metafile: drawing commands, not pixels.
  Emf,
  /// A Windows metafile.
  Wmf,
  /// A bitmap with no file header, as Windows keeps one in memory.
  Dib,
  /// A bitmap of the kind Windows had before that.
  Bitmap,
  /// A QuickDraw picture.
  MacPict,
  /// The group does not say.
  Unknown,
}

/// A picture of the document, as its bytes. A metafile is not drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
  pub format: PictureFormat,
  pub bytes: Vec<u8>,
  /// The width and height the picture is to be shown at, in twentieths of a point, where the
  /// group states both.
  pub goal: Option<(i32, i32)>,
}

/// An RTF file.
#[derive(Debug, Clone, PartialEq)]
pub struct Rtf {
  body: Body,
  embedded: Vec<Embedded>,
  pictures: Vec<Picture>,
  warnings: Vec<String>,
}

/// Bytes gathered from hex digits and from `\bin`.
#[derive(Default)]
struct Data {
  bytes: Vec<u8>,
  /// The first digit of a pair whose second has not come yet.
  half: Option<u8>,
}

impl Data {
  fn hex(&mut self, text: &[u8]) {
    for digit in text
      .iter()
      .filter_map(|byte| char::from(*byte).to_digit(16))
    {
      // A hex digit is under 16.
      let digit = digit as u8;
      match self.half.take() {
        Some(high) => self.bytes.push(high << 4 | digit),
        None => self.half = Some(digit),
      }
    }
  }
}

/// An object group as it is gathered.
#[derive(Default)]
struct Object {
  class: String,
  data: Data,
  /// The object marks where an attachment goes, and carries none.
  placeholder: bool,
}

/// A picture group as it is gathered.
struct Drawn {
  format: PictureFormat,
  data: Data,
  goal: (Option<i32>, Option<i32>),
}

/// What the group being read is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum In {
  /// The document's own text.
  Body,
  Object(usize),
  ObjectClass(usize),
  ObjectData(usize),
  Picture(usize),
  /// A group whose contents are not the document's: the picture that stands for an object,
  /// and the older copy of a picture that Word writes beside the newer.
  Passed,
}

/// A length, and then that many bytes of text that end with a zero byte.
fn counted<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
  let length = usize::try_from(u32_of(bytes)?).ok()?;
  let text = bytes.get(..length)?;
  *bytes = &bytes[length..];
  Some(text.split(|byte| *byte == 0).next().unwrap_or(text))
}

fn u32_of(bytes: &mut &[u8]) -> Option<u32> {
  let value = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?);
  *bytes = &bytes[4..];
  Some(value)
}

/// Text up to a zero byte, and the bytes after it.
fn terminated<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
  let end = bytes.iter().position(|byte| *byte == 0)?;
  let text = &bytes[..end];
  *bytes = &bytes[end + 1..];
  Some(text)
}

fn text(bytes: &[u8]) -> Option<String> {
  // Names are kept in the code page of the machine that wrote them; the Western one is the
  // usual, and its letters are these.
  let text: String = bytes.iter().map(|byte| char::from(*byte)).collect();
  let text = text.trim();
  (!text.is_empty()).then(|| text.to_string())
}

/// An object's class and its own data, from what `\objdata` holds: a version, whether the
/// object is embedded or only linked, three names, and the data with its length before it.
fn native(mut data: &[u8]) -> Option<(Option<String>, &[u8])> {
  let _version = u32_of(&mut data)?;
  // 2 is an embedded object. 1 is a link to a file that is somewhere else.
  if u32_of(&mut data)? != 2 {
    return None;
  }
  let class = counted(&mut data)?;
  let _topic = counted(&mut data)?;
  let _item = counted(&mut data)?;
  let length = usize::try_from(u32_of(&mut data)?).ok()?;
  Some((text(class), data.get(..length)?))
}

/// A file dropped into a document, as the packager keeps it: its label, the path it came
/// from, a path it was copied to, and then its bytes with their length before them.
fn packaged(mut data: &[u8]) -> Option<(Option<String>, &[u8])> {
  data = data.get(2..)?;
  let label = terminated(&mut data)?;
  let _from = terminated(&mut data)?;
  let _kind = u32_of(&mut data)?;
  let _length_of_path = u32_of(&mut data)?;
  let _copied_to = terminated(&mut data)?;
  let length = usize::try_from(u32_of(&mut data)?).ok()?;
  Some((text(label), data.get(..length)?))
}

/// The streams of a compound file that hold a file whole, in the order they are looked for:
/// a mail attachment, a document of 2007 or later, and what Acrobat embeds.
const HOLDING: [&str; 3] = ["AttachContents", "Package", "CONTENTS"];

/// The embedded file, from an object's class and its data.
fn embedded(stated: &str, data: &[u8]) -> Embedded {
  let stated = text(stated.as_bytes());
  let Some((class, native)) = native(data) else {
    return Embedded {
      class: stated,
      name: None,
      bytes: data.to_vec(),
    };
  };
  let class = class.or(stated);
  if let Some(mut streams) = cfb::streams(native) {
    let held = HOLDING
      .iter()
      .find_map(|name| streams.iter().position(|stream| stream.name == *name));
    // An object the packager made, kept in a compound file as later programs keep it: its
    // stream has the data's length in front.
    let packed = streams
      .iter()
      .find(|stream| stream.name == "\u{1}Ole10Native")
      .and_then(|stream| packaged(stream.bytes.get(4..)?));
    if let Some((name, bytes)) = packed {
      return Embedded {
        class,
        name,
        bytes: bytes.to_vec(),
      };
    }
    return Embedded {
      class,
      name: None,
      // With no stream that holds a file, the compound file is the file: a document or a
      // workbook in the format before 2007 is one.
      bytes: match held {
        Some(index) => streams.swap_remove(index).bytes,
        None => native.to_vec(),
      },
    };
  }
  if class.as_deref() == Some("Package")
    && let Some((name, bytes)) = packaged(native)
  {
    return Embedded {
      class,
      name,
      bytes: bytes.to_vec(),
    };
  }
  Embedded {
    class,
    name: None,
    bytes: native.to_vec(),
  }
}

/// A picture's format: what its bytes open as, where they open as a PNG or a JPEG, and
/// otherwise what its group says. A group that says PNG or JPEG over bytes that are neither is
/// not believed: one writer marks the pattern a shape is filled with that way.
fn format_of(stated: PictureFormat, bytes: &[u8]) -> PictureFormat {
  if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
    return PictureFormat::Png;
  }
  if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
    return PictureFormat::Jpeg;
  }
  match stated {
    PictureFormat::Png | PictureFormat::Jpeg => PictureFormat::Unknown,
    other => other,
  }
}

impl Rtf {
  /// Reads a file. Fails with `unsupported format: …` for bytes that are not RTF, and with
  /// `limit exceeded: …` for groups nested deeper than any document nests them.
  pub fn open(bytes: &[u8]) -> Result<Rtf, RtfError> {
    // A byte-order mark and blank space may stand before the opening.
    let from = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let blank = from
      .iter()
      .take_while(|byte| byte.is_ascii_whitespace())
      .count();
    let from = &from[blank..];
    if !from.starts_with(br"{\rtf") {
      return Err(RtfError::UnsupportedFormat(
        "the file does not open as RTF does".to_string(),
      ));
    }
    let mut objects: Vec<Object> = Vec::new();
    let mut drawn: Vec<Drawn> = Vec::new();
    let mut stack = vec![In::Body];
    let mut now = In::Body;
    for token in Tokens::over(from) {
      match token {
        Token::Open => {
          if stack.len() > MOST_DEPTH {
            return Err(RtfError::LimitExceeded(format!(
              "groups are nested more than {MOST_DEPTH} deep"
            )));
          }
          stack.push(now);
          // A group inside a picture or inside an object's data says something of it (a
          // shape's properties, an identifier) and is not its data.
          if matches!(now, In::Picture(_) | In::ObjectData(_) | In::ObjectClass(_)) {
            now = In::Passed;
          }
        }
        // A closing brace with no group open is passed over.
        Token::Close => now = stack.pop().unwrap_or(In::Body),
        Token::Word { name, parameter } => match (name, now) {
          ("object", In::Body) => {
            objects.push(Object::default());
            now = In::Object(objects.len() - 1);
          }
          ("objclass", In::Object(index)) => now = In::ObjectClass(index),
          ("objdata", In::Object(index)) => now = In::ObjectData(index),
          ("objattph", In::Object(index)) => objects[index].placeholder = true,
          ("result", In::Object(_)) | ("nonshppict", _) => now = In::Passed,
          ("pict", In::Body) => {
            drawn.push(Drawn {
              format: PictureFormat::Unknown,
              data: Data::default(),
              goal: (None, None),
            });
            now = In::Picture(drawn.len() - 1);
          }
          (_, In::Picture(index)) => {
            let picture = &mut drawn[index];
            match name {
              "pngblip" => picture.format = PictureFormat::Png,
              "jpegblip" => picture.format = PictureFormat::Jpeg,
              "emfblip" => picture.format = PictureFormat::Emf,
              "wmetafile" => picture.format = PictureFormat::Wmf,
              "dibitmap" => picture.format = PictureFormat::Dib,
              "wbitmap" => picture.format = PictureFormat::Bitmap,
              "macpict" => picture.format = PictureFormat::MacPict,
              "picwgoal" => picture.goal.0 = parameter,
              "pichgoal" => picture.goal.1 = parameter,
              _ => {}
            }
          }
          _ => {}
        },
        Token::Text(text) => match now {
          In::ObjectClass(index) => objects[index]
            .class
            .push_str(&String::from_utf8_lossy(text)),
          In::ObjectData(index) => objects[index].data.hex(text),
          In::Picture(index) => drawn[index].data.hex(text),
          _ => {}
        },
        Token::Binary(held) => match now {
          In::ObjectData(index) => objects[index].data.bytes.extend_from_slice(held),
          In::Picture(index) => drawn[index].data.bytes.extend_from_slice(held),
          _ => {}
        },
        Token::Symbol(_) | Token::Hex(_) => {}
      }
    }
    let mut warnings = Vec::new();
    let mut warn = |warning: &str| {
      if !warnings.iter().any(|said| said == warning) {
        warnings.push(warning.to_string());
      }
    };
    let mut files = Vec::new();
    for object in objects {
      if object.data.bytes.is_empty() {
        if object.placeholder {
          warn("an attachment has its place marked in the text, and is not in the file");
        }
        continue;
      }
      files.push(embedded(&object.class, &object.data.bytes));
    }
    let pictures = drawn
      .into_iter()
      .filter(|picture| !picture.data.bytes.is_empty())
      .map(|picture| Picture {
        format: format_of(picture.format, &picture.data.bytes),
        bytes: picture.data.bytes,
        goal: picture.goal.0.zip(picture.goal.1),
      })
      .collect();
    Ok(Rtf {
      body: body::read(from),
      embedded: files,
      pictures,
      warnings,
    })
  }

  /// The files embedded in the document, in the order they come in it. Each is given as it
  /// was before it was embedded, and is not read: it goes to the reader of what it is.
  pub fn embedded(&self) -> &[Embedded] {
    &self.embedded
  }

  /// The document's pictures, in the order they come in it. The picture that stands for an
  /// embedded object is not among them, and of the two copies Word writes of a picture, the
  /// newer is the one given.
  pub fn pictures(&self) -> &[Picture] {
    &self.pictures
  }

  /// The document's text: a line to a paragraph, and a line to a table's row with a tab
  /// between its cells.
  pub fn text(&self) -> String {
    self.body.text()
  }

  /// The document's paragraphs, the rows of its tables, and its page breaks, in order.
  pub fn blocks(&self) -> &[Block] {
    &self.body.blocks
  }

  /// The paper the file states, with its margins; letter when it states none.
  pub fn paper(&self) -> Paper {
    self.body.paper
  }

  /// The header and the footer of the pages, and those of the first page where the file says
  /// it has its own. They are on the pages `layout` gives, and not in `text`.
  pub fn running(&self) -> &Running {
    &self.body.running
  }

  /// The document set on pages, by estimate: a paragraph wrapped between the margins the
  /// file states, the cells of a row between the edges the file gives them, and a page turned
  /// at a page break and where the text reaches the foot of one. Every page says that its
  /// positions are estimated.
  pub fn layout(&self) -> Vec<PageLayout> {
    layout::pages(&self.body)
  }

  /// Invoice fields read from the pages as [`Rtf::layout`] sets them. The files the document
  /// carries are not read here, and an invoice may be one of them: when it carries any, the
  /// first of the warnings says so.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    let mut invoice = extract_from_pages(&self.layout(), options);
    let carried = match self.embedded.len() {
      0 => None,
      1 => Some(
        "the document carries 1 embedded file, which is not read here: the invoice may be in it"
          .to_string(),
      ),
      count => Some(format!(
        "the document carries {count} embedded files, which are not read here: the invoice may be in one of them"
      )),
    };
    if let Some(carried) = carried {
      invoice.warnings.insert(0, carried);
    }
    invoice
  }

  /// What to know of the reading: an attachment whose place is marked and that is not there.
  pub fn warnings(&self) -> &[String] {
    &self.warnings
  }
}

#[cfg(test)]
mod tests;
