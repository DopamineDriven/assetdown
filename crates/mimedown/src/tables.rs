//! Between a type and the file extensions in use for it, both ways.
//!
//! Nothing here looks at a file's bytes: a name says what a file claims to be. These are for
//! giving a file a name once its type is known, and for the text whose kind only a name can
//! settle. `sniff` never consults them.

use crate::media_type::MediaType;

/// Every type `extension` is in use for, the usual one first. Letter case and a leading dot are
/// ignored.
pub fn media_types_of(extension: &str) -> Vec<MediaType> {
  let extension = extension.strip_prefix('.').unwrap_or(extension);
  MediaType::ALL
    .iter()
    .copied()
    .filter(|media_type| {
      media_type
        .extensions()
        .iter()
        .any(|known| known.eq_ignore_ascii_case(extension))
    })
    .collect()
}

/// The type a path's extension is usually used for. The extension is what follows the last dot
/// of the last segment. The path is read as an address first, whose name ends at a `?` or a
/// `#`. One that names no type that way and has no scheme in front of it is then read as a
/// local name, in which both are characters like any other.
pub fn media_type_of_path(path: &str) -> Option<MediaType> {
  let usual = |path: &str| {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let (_, extension) = name.rsplit_once('.')?;
    media_types_of(extension).first().copied()
  };
  let address = path.split(['?', '#']).next().unwrap_or(path);
  usual(address).or_else(|| match path.contains("://") {
    true => None,
    false => usual(path),
  })
}

/// The extensions in use for the type `mime` names, the usual one first. Empty when the string
/// names no type here.
pub fn extensions_of(mime: &str) -> &'static [&'static str] {
  MediaType::from_mime(mime).map_or(&[], MediaType::extensions)
}

#[cfg(test)]
mod tests;
