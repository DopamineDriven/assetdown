//! The files a document carries. Each embedded object is a storage of its own under
//! `ObjectPool`, and holds the file as the program that embedded it left it: as the packager
//! keeps a file dropped in, or whole in a stream of a known name, or as the streams of the
//! storage itself, which is what a workbook or a document of before 2007 is.

use cfbdown::{CompoundFile, HOLDING, packaged};

/// The storage that holds one storage for each embedded object.
const POOL: &str = "ObjectPool";

/// A file embedded in the document, as it was before it was embedded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embedded {
  /// The class of the object that holds it, as the program that embedded it names itself:
  /// `Package` for a file dropped into a document, `Excel.Sheet.8`, `AcroExch.Document.7`,
  /// and so on.
  pub class: Option<String>,
  /// The file's name, where the object keeps one.
  pub name: Option<String>,
  /// The file.
  pub bytes: Vec<u8>,
}

/// A count, and then that many bytes of text that end with a zero byte.
fn counted<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
  let length = usize::try_from(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?)).ok()?;
  let text = bytes.get(4..4 + length)?;
  *bytes = &bytes[4 + length..];
  Some(text.split(|byte| *byte == 0).next().unwrap_or(text))
}

/// The name programs use for the object, from what it says of itself: after 28 bytes, the
/// name a person sees, the format it puts on the clipboard, and then the name wanted.
fn class(says: &[u8]) -> Option<String> {
  let mut rest = says.get(28..)?;
  let _shown = counted(&mut rest)?;
  // The format is a number where the mark before it is all ones or one less, text with its
  // length where the mark is a length, and nothing where it is nought.
  let mark = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?);
  match mark {
    0 => rest = &rest[4..],
    0xFFFF_FFFE | 0xFFFF_FFFF => rest = rest.get(8..)?,
    _ => {
      counted(&mut rest)?;
    }
  }
  let class: String = counted(&mut rest)?
    .iter()
    .map(|byte| char::from(*byte))
    .collect();
  let class = class.trim();
  (!class.is_empty()).then(|| class.to_string())
}

/// The files the document carries, in the order of their storages' names, which is the order
/// they were put in.
pub(crate) fn read(file: &CompoundFile<'_>) -> Vec<Embedded> {
  let mut storages: Vec<&str> = file
    .children(POOL)
    .into_iter()
    .filter(|child| child.is_storage)
    .map(|child| child.name)
    .collect();
  storages.sort_unstable();
  storages
    .into_iter()
    .filter_map(|storage| {
      let within = |name: &str| file.stream(&format!("{POOL}/{storage}/{name}"));
      let class = within("\u{1}CompObj").and_then(|says| class(&says));
      // A file the packager left has its length before it.
      let native = within("\u{1}Ole10Native");
      let dropped = native
        .as_deref()
        .and_then(|native| packaged(native.get(4..)?));
      if let Some((name, bytes)) = dropped {
        return Some(Embedded {
          class,
          name,
          bytes: bytes.to_vec(),
        });
      }
      let held = HOLDING.iter().find_map(|name| within(name));
      // With no stream that holds a file, the storage is the file.
      let bytes = held.or_else(|| {
        let holds_any = file
          .children(&format!("{POOL}/{storage}"))
          .iter()
          .any(|child| child.is_storage || !child.name.starts_with(['\u{1}', '\u{3}']));
        holds_any
          .then(|| file.storage(&format!("{POOL}/{storage}")))
          .flatten()
      })?;
      Some(Embedded {
        class,
        name: None,
        bytes,
      })
    })
    .collect()
}
