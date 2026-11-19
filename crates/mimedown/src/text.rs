//! Text: what decodes as text, and what kind of text it is. Bytes can say that a file is JSON,
//! markup with a certain root, a script for a certain interpreter. They cannot tell Markdown
//! from a Rust source file, and for that a file's name is taken as a hint.

use crate::media_type::MediaType;
use crate::tables::media_types_of;

mod json;
pub(crate) mod markup;

/// How a text's characters are written as bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
  Utf8,
  Utf16Le,
  Utf16Be,
  /// One byte to a character, in an encoding the bytes do not name.
  EightBit,
}

impl Encoding {
  pub(crate) fn as_str(self) -> &'static str {
    match self {
      Encoding::Utf8 => "utf-8",
      Encoding::Utf16Le => "utf-16le",
      Encoding::Utf16Be => "utf-16be",
      Encoding::EightBit => "unknown-8bit",
    }
  }
}

/// A control character that text does not hold: anything below a space but a tab, a line
/// ending, a form feed and an escape.
fn is_binary_control(byte: u8) -> bool {
  byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r' | 0x0C | 0x1B)
}

/// The units of UTF-16 that begin a pair: a character outside the first 65,536 is two units.
const FIRST_OF_A_PAIR: std::ops::RangeInclusive<u16> = 0xD800..=0xDBFF;

/// How many units of UTF-16 with no byte-order mark it takes to tell them by.
const FEWEST_UNMARKED_UNITS: usize = 8;

/// `bytes` as text, when it is text. `cut` says the bytes stop short of the file's end, so a
/// character may be cut in two at the end of them.
pub(crate) fn decode(bytes: &[u8], cut: bool) -> Option<(Encoding, String)> {
  let utf16 = |rest: &[u8], encoding: Encoding| {
    let (pairs, _) = rest.as_chunks::<2>();
    let mut units: Vec<u16> = pairs
      .iter()
      .map(|pair| match encoding {
        Encoding::Utf16Le => u16::from_le_bytes(*pair),
        _ => u16::from_be_bytes(*pair),
      })
      .collect();
    // The first unit of a pair, with its second past the bytes given, is not a fault either.
    if cut
      && units
        .last()
        .is_some_and(|unit| FIRST_OF_A_PAIR.contains(unit))
    {
      units.pop();
    }
    let text: String = char::decode_utf16(units).collect::<Result<_, _>>().ok()?;
    // What text does not hold in one byte it does not hold in two.
    let holds_binary = text
      .chars()
      .any(|c| u8::try_from(c).is_ok_and(is_binary_control));
    (!holds_binary).then_some((encoding, text))
  };
  match bytes {
    [] => None,
    [0xFF, 0xFE, rest @ ..] => utf16(rest, Encoding::Utf16Le),
    [0xFE, 0xFF, rest @ ..] => utf16(rest, Encoding::Utf16Be),
    _ if bytes.iter().any(|byte| is_binary_control(*byte)) => None,
    _ => {
      let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
      match std::str::from_utf8(bytes) {
        Ok(text) => Some((Encoding::Utf8, text.to_string())),
        // A character cut in two by the end of the bytes given is not a fault of the text.
        Err(error) if cut && error.error_len().is_none() => {
          let whole = &bytes[..error.valid_up_to()];
          Some((Encoding::Utf8, String::from_utf8_lossy(whole).into_owned()))
        }
        // Not UTF-8, and nothing in it that text does not hold: one byte to a character, when
        // nearly all of them are plain ASCII, as they are in a text of western Europe.
        Err(_) => {
          let plain = bytes.iter().filter(|byte| byte.is_ascii()).count();
          (plain * 10 >= bytes.len() * 9).then(|| {
            let text = bytes.iter().map(|byte| char::from(*byte)).collect();
            (Encoding::EightBit, text)
          })
        }
      }
    }
  }
}

/// `bytes` as text, read with more allowance than `decode` gives: UTF-16 with no byte-order
/// mark in front of it, or text with a stray control character in it, no more than one byte
/// in a thousand.
pub(crate) fn decode_leniently(bytes: &[u8], cut: bool) -> Option<(Encoding, String)> {
  // In UTF-16 every other byte of a western text is zero.
  let (pairs, _) = bytes.as_chunks::<2>();
  if pairs.len() >= FEWEST_UNMARKED_UNITS {
    let most = pairs.len() * 9 / 10;
    let zero_second = pairs
      .iter()
      .filter(|pair| pair[1] == 0 && pair[0] != 0)
      .count();
    let zero_first = pairs
      .iter()
      .filter(|pair| pair[0] == 0 && pair[1] != 0)
      .count();
    let mark: Option<[u8; 2]> = if zero_second >= most {
      Some([0xFF, 0xFE])
    } else if zero_first >= most {
      Some([0xFE, 0xFF])
    } else {
      None
    };
    if let Some(mark) = mark {
      return decode(&[&mark[..], bytes].concat(), cut);
    }
  }
  let strays = bytes
    .iter()
    .filter(|byte| is_binary_control(**byte))
    .count();
  if strays == 0 || strays * 1000 > bytes.len() {
    return None;
  }
  let cleaned: Vec<u8> = bytes
    .iter()
    .copied()
    .filter(|byte| !is_binary_control(*byte))
    .collect();
  decode(&cleaned, cut)
}

/// What kind of text it is, and whether the bytes or the name said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  /// The text itself says what it is.
  Content(MediaType),
  /// The text does not say, and the file's name does.
  Named(MediaType),
}

/// The types text can be, beyond those the registry files under `text`. A name is believed
/// only when it names one of these.
fn can_be_text(media_type: MediaType) -> bool {
  media_type.mime().starts_with("text/")
    || matches!(
      media_type,
      MediaType::Json
        | MediaType::Jsonc
        | MediaType::Json5
        | MediaType::Jsonl
        | MediaType::Ndjson
        | MediaType::JsonLd
        | MediaType::WebManifest
        | MediaType::Yaml
        | MediaType::Toml
        | MediaType::Sql
        | MediaType::Shell
        | MediaType::CShell
        | MediaType::Node
        | MediaType::Mdx
        | MediaType::Xml
        | MediaType::Xhtml
        | MediaType::Xul
        | MediaType::Svg
        | MediaType::HlsPlaylist
        | MediaType::DashManifest
        | MediaType::Gltf
        | MediaType::ModelObj
        | MediaType::Mtl
        | MediaType::Stl
        | MediaType::Collada
        | MediaType::X3d
        | MediaType::Postscript
    )
}

/// The type a file's name stands for, when it is one that text can be.
fn named(file_name: Option<&str>) -> Option<MediaType> {
  let (_, extension) = file_name?.rsplit_once('.')?;
  let could_be: Vec<MediaType> = media_types_of(extension)
    .into_iter()
    .filter(|media_type| can_be_text(*media_type))
    .collect();
  // The type whose own extension this is, before a type that merely takes it in: `.jsonc`
  // is JSON to most programs, and JSON with comments by its name.
  could_be
    .iter()
    .copied()
    .find(|media_type| media_type.ext().eq_ignore_ascii_case(extension))
    .or_else(|| could_be.first().copied())
}

/// The interpreter a script's first line asks for.
fn script(first_line: &str) -> Option<MediaType> {
  let command = first_line.strip_prefix("#!")?.trim();
  let mut words = command.split_whitespace();
  let program = words.next()?.rsplit('/').next()?;
  // `env` is asked to find the interpreter, which is the word after it and its options.
  let program = if program == "env" {
    words.find(|word| !word.starts_with('-'))?
  } else {
    program
  };
  let name = program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
  Some(match name {
    "sh" | "bash" | "zsh" | "dash" | "ksh" | "ash" => MediaType::Shell,
    "csh" | "tcsh" => MediaType::CShell,
    "python" | "pypy" => MediaType::Python,
    "node" | "nodejs" | "deno" | "bun" => MediaType::JavaScript,
    "ruby" => MediaType::Ruby,
    "php" => MediaType::Php,
    _ => return None,
  })
}

/// What kind of text `text` is. `cut` says it stops short of the file's end.
pub(crate) fn kind(text: &str, cut: bool, file_name: Option<&str>) -> Kind {
  let named = named(file_name);
  let start = text.trim_start();
  let first_line = start.lines().next().unwrap_or_default();
  // How a text begins, for the kinds that begin one way.
  let begins: [(&str, MediaType); 6] = [
    ("BEGIN:VCARD", MediaType::Vcard),
    ("BEGIN:VCALENDAR", MediaType::Calendar),
    ("#EXTM3U", MediaType::HlsPlaylist),
    ("WEBVTT", MediaType::Vtt),
    ("%!PS", MediaType::Postscript),
    ("{\\rtf", MediaType::Rtf),
  ];
  if let Some((_, media_type)) = begins
    .iter()
    .find(|(opening, _)| start.starts_with(opening))
  {
    return Kind::Content(*media_type);
  }
  if let Some(media_type) = script(first_line) {
    return Kind::Content(media_type);
  }
  if let Some(media_type) = markup::root(start).and_then(|root| root.media_type()) {
    return Kind::Content(media_type);
  }
  if start.starts_with('{') || start.starts_with('[') {
    // The kinds of JSON look alike, and the name says which this is.
    let family = [
      MediaType::Gltf,
      MediaType::JsonLd,
      MediaType::WebManifest,
      MediaType::Jsonc,
      MediaType::Json5,
    ];
    match json::read(start, cut) {
      json::Reading::Value => {
        return match named.filter(|named| family.contains(named)) {
          Some(named) => Kind::Named(named),
          None => Kind::Content(MediaType::Json),
        };
      }
      // A value on every line.
      json::Reading::Lines => {
        return match named.filter(|named| *named == MediaType::Ndjson) {
          Some(named) => Kind::Named(named),
          None => Kind::Content(MediaType::Jsonl),
        };
      }
      json::Reading::Neither => {}
    }
  }
  match named {
    Some(named) => Kind::Named(named),
    None => Kind::Content(MediaType::Txt),
  }
}

/// The counts of a text.
pub(crate) struct Counts {
  pub lines: u64,
  pub words: u64,
  pub preview: Option<String>,
}

/// How many characters of text a preview holds.
const PREVIEW_CHARACTERS: usize = 500;

pub(crate) fn counts(text: &str) -> Counts {
  let preview: String = text.trim_start().chars().take(PREVIEW_CHARACTERS).collect();
  let preview = preview.trim_end();
  Counts {
    lines: u64::try_from(text.lines().count()).unwrap_or(u64::MAX),
    words: u64::try_from(text.split_whitespace().count()).unwrap_or(u64::MAX),
    preview: (!preview.is_empty()).then(|| preview.to_string()),
  }
}

#[cfg(test)]
mod tests;
