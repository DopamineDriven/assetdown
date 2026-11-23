//! What a compound file says about the document in it: its title, who wrote it, when, and
//! with what. An Office file of before 2007 keeps these in two streams of properties, each
//! property a number and a value.

use std::collections::HashMap;

use encoding_rs::Encoding;

use crate::file::CompoundFile;

const SUMMARY: &str = "\u{5}SummaryInformation";
const DOCUMENT_SUMMARY: &str = "\u{5}DocumentSummaryInformation";
/// The code page of a stream that states none, and the one whose text is two bytes to a
/// character.
const WESTERN: u16 = 1252;
const TWO_BYTE: u16 = 1200;

/// What the two property streams say. Every field is absent when the file does not state it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
  pub title: Option<String>,
  pub subject: Option<String>,
  pub author: Option<String>,
  /// As written: one string, usually separated by commas or semicolons.
  pub keywords: Option<String>,
  pub comments: Option<String>,
  pub template: Option<String>,
  /// Who saved the file last.
  pub last_author: Option<String>,
  pub revision: Option<String>,
  /// When the file was created and last saved, to the second: `2026-11-15T09:30:00Z`. The
  /// file keeps both in universal time.
  pub created: Option<String>,
  pub modified: Option<String>,
  /// The application that saved the file, and its version: `12.0000`.
  pub application: Option<String>,
  pub app_version: Option<String>,
  pub category: Option<String>,
  pub manager: Option<String>,
  pub company: Option<String>,
  pub language: Option<String>,
}

/// A property's value, of the kinds read here.
enum Value {
  Text(String),
  /// A moment, in tenths of a microsecond since the start of 1601.
  Time(u64),
  Number(i32),
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn size_at(bytes: &[u8], at: usize) -> Option<usize> {
  usize::try_from(u32_at(bytes, at)?).ok()
}

/// The encoding a code page is, by the number Windows gives it.
fn encoding_of(page: u16) -> Option<&'static Encoding> {
  let label = match page {
    874 | 1250..=1258 => format!("windows-{page}"),
    932 => "shift_jis".to_string(),
    936 => "gbk".to_string(),
    949 => "euc-kr".to_string(),
    950 => "big5".to_string(),
    10000 => "macintosh".to_string(),
    20866 => "koi8-r".to_string(),
    21866 => "koi8-u".to_string(),
    // Plain ASCII and Latin-1 are both read as the Western page, which holds them.
    20127 | 28591 => "windows-1252".to_string(),
    28592..=28606 => format!("iso-8859-{}", page - 28590),
    51932 => "euc-jp".to_string(),
    54936 => "gb18030".to_string(),
    65001 => "utf-8".to_string(),
    _ => return None,
  };
  Encoding::for_label(label.as_bytes())
}

/// Text two bytes to a character, up to the nought that ends it.
fn two_byte(bytes: &[u8]) -> String {
  let units: Vec<u16> = bytes
    .as_chunks::<2>()
    .0
    .iter()
    .map(|pair| u16::from_le_bytes(*pair))
    .take_while(|unit| *unit != 0)
    .collect();
  String::from_utf16_lossy(&units)
}

/// Text in the code page `page`, up to the nought that ends it. None for bytes that are not
/// text in that page, and for a page that is not known when the text is more than plain
/// ASCII: text is left out before it is given wrong.
fn text_in(page: u16, bytes: &[u8]) -> Option<String> {
  if page == TWO_BYTE {
    return Some(two_byte(bytes));
  }
  let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or(bytes);
  match encoding_of(page) {
    Some(encoding) => encoding
      .decode_without_bom_handling_and_without_replacement(bytes)
      .map(|text| text.into_owned()),
    None => bytes
      .is_ascii()
      .then(|| bytes.iter().map(|byte| char::from(*byte)).collect()),
  }
}

/// The properties of a stream's first set, by their numbers. A stream opens with 28 bytes
/// (its byte order, a version, and who wrote it, and then how many sets it has), and for
/// each set says what it is and where it starts. A set says how long it is and how many
/// properties it has, and for each its number and where its value is, from the set's start.
/// A value opens with its kind.
fn properties(stream: &[u8]) -> HashMap<u32, Value> {
  let mut found = HashMap::new();
  let listed = (|| {
    if u32_at(stream, 24)? == 0 {
      return None;
    }
    let set = stream.get(size_at(stream, 44)?..)?;
    let count = size_at(set, 4)?;
    let places: Vec<(u32, usize)> = (0..count)
      .map_while(|nth| Some((u32_at(set, 8 + nth * 8)?, size_at(set, 12 + nth * 8)?)))
      .collect();
    Some((set, places))
  })();
  let Some((set, places)) = listed else {
    return found;
  };
  // The code page is a property like any other, and may come after the text it is for.
  let page = places
    .iter()
    .find(|(id, _)| *id == 1)
    .filter(|(_, at)| u16_at(set, *at) == Some(0x02))
    .and_then(|(_, at)| u16_at(set, at + 4))
    .unwrap_or(WESTERN);
  for (id, at) in places {
    let value = match u16_at(set, at) {
      // Text in the stream's code page, with its length in bytes before it.
      Some(0x1E) => size_at(set, at + 4)
        .and_then(|length| set.get(at + 8..(at + 8).checked_add(length)?))
        .and_then(|bytes| text_in(page, bytes))
        .map(Value::Text),
      // Text two bytes to a character, with its length in characters before it.
      Some(0x1F) => size_at(set, at + 4)
        .and_then(|length| set.get(at + 8..(at + 8).checked_add(length.checked_mul(2)?)?))
        .map(|bytes| Value::Text(two_byte(bytes))),
      Some(0x40) => set
        .get(at + 4..at + 12)
        .and_then(|ticks| ticks.try_into().ok())
        .map(|ticks| Value::Time(u64::from_le_bytes(ticks))),
      Some(0x03) => u32_at(set, at + 4).map(|number| Value::Number(number.cast_signed())),
      _ => None,
    };
    if let Some(value) = value {
      found.insert(id, value);
    }
  }
  found
}

/// A moment as a date and a time of day in universal time, to the second. None for nought,
/// which is how a file states that it has no such moment.
fn moment(ticks: u64) -> Option<String> {
  if ticks == 0 {
    return None;
  }
  let seconds = ticks / 10_000_000;
  let (days, of_day) = (seconds / 86_400, seconds % 86_400);
  // Days since the first of March of the year nought, from days since the start of 1601:
  // counted from March, a leap day is the last day of its year.
  let days = i64::try_from(days).ok()? + 584_694;
  let (era, of_era) = (days.div_euclid(146_097), days.rem_euclid(146_097));
  let year_of_era = (of_era - of_era / 1_460 + of_era / 36_524 - of_era / 146_096) / 365;
  let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let months_from_march = (5 * of_year + 2) / 153;
  let day = of_year - (153 * months_from_march + 2) / 5 + 1;
  let month = match months_from_march {
    0..10 => months_from_march + 3,
    _ => months_from_march - 9,
  };
  let year = year_of_era + era * 400 + i64::from(month <= 2);
  Some(format!(
    "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
    of_day / 3_600,
    of_day % 3_600 / 60,
    of_day % 60
  ))
}

impl CompoundFile<'_> {
  /// What the file says about the document in it, from its two property streams. A stream
  /// that is missing or cannot be read states nothing, and neither does a property whose
  /// value cannot be read.
  pub fn summary(&self) -> Summary {
    let read = |name: &str| {
      self
        .stream(name)
        .map_or_else(HashMap::new, |stream| properties(&stream))
    };
    let (mut first, mut second) = (read(SUMMARY), read(DOCUMENT_SUMMARY));
    let text = |of: &mut HashMap<u32, Value>, id: u32| match of.remove(&id) {
      Some(Value::Text(text)) => Some(text.trim().to_string()).filter(|text| !text.is_empty()),
      _ => None,
    };
    let title = text(&mut first, 2);
    let subject = text(&mut first, 3);
    let author = text(&mut first, 4);
    let keywords = text(&mut first, 5);
    let comments = text(&mut first, 6);
    let template = text(&mut first, 7);
    let last_author = text(&mut first, 8);
    let revision = text(&mut first, 9);
    let application = text(&mut first, 18);
    let category = text(&mut second, 2);
    let manager = text(&mut second, 14);
    let company = text(&mut second, 15);
    let language = text(&mut second, 28);
    let time = |of: &HashMap<u32, Value>, id: u32| match of.get(&id) {
      Some(Value::Time(ticks)) => moment(*ticks),
      _ => None,
    };
    Summary {
      title,
      subject,
      author,
      keywords,
      comments,
      template,
      last_author,
      revision,
      created: time(&first, 12),
      modified: time(&first, 13),
      application,
      // The version is two halves of one number: `12.0000` is twelve, and nought.
      app_version: match second.get(&23) {
        Some(Value::Number(version)) => {
          let version = version.cast_unsigned();
          Some(format!("{}.{:04}", version >> 16, version & 0xFFFF))
        }
        _ => None,
      },
      category,
      manager,
      company,
      language,
    }
  }
}

#[cfg(test)]
mod tests;
