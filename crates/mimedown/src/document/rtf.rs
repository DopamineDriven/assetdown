//! RTF: text with its formatting written between the words as groups in braces and control
//! words after backslashes. What the document says of itself is in one group near the start;
//! its text is whatever is left once the groups that are not text are set aside.

use super::{DocumentDetail, DocumentFacts};
use crate::image::Reading;
use crate::source::Source;

/// What only an RTF says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtfDetail {
  /// The code page the file names for the bytes it writes above 127: 1252 for Western Europe.
  pub code_page: Option<u32>,
  /// The program that wrote the file, as it names itself.
  pub generator: Option<String>,
}

/// No more groups than this are nested, and no more of the text than this is kept to show.
const MOST_DEPTH: usize = 10_000;
const PREVIEW_CHARACTERS: usize = 500;

/// What the text of a group is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
  /// The document's own text.
  Body,
  /// Set aside: a table of fonts or colors, a picture, a header, an instruction.
  Aside,
  /// A shape's description: names and values that are not text, among which its picture is
  /// one and the words of a text box another.
  Shape,
  /// The group that holds what the document says of itself. Its own text is nothing; the
  /// groups inside it are the title, the author and the rest.
  Info,
  Title,
  Author,
  Subject,
  Keywords,
  Created,
  Revised,
  Generator,
}

/// What a group carries from where it opens to where it closes.
#[derive(Clone, Copy)]
struct Group {
  destination: Destination,
  /// What the group that holds this one was for.
  outer: Destination,
  /// How many characters after one written by its number stand in for it, for a reader that
  /// cannot show it.
  stand_ins: u32,
  /// The language its text is said to be in, as Windows numbers languages.
  language: Option<i32>,
  /// The group holds the same text twice, plainly and then with every character it has: the
  /// plain one is set aside.
  twice: bool,
}

/// The document's text as it is gathered: how it begins, and how many words it has.
#[derive(Default)]
struct Body {
  preview: String,
  kept: usize,
  words: u64,
  in_word: bool,
}

impl Body {
  fn push(&mut self, character: char) {
    let space = character.is_whitespace();
    if !space && !self.in_word {
      self.words += 1;
    }
    self.in_word = !space;
    // The text shown begins at its first word.
    if self.kept < PREVIEW_CHARACTERS && !(space && self.kept == 0) {
      self.preview.push(character);
      self.kept += 1;
    }
  }
}

/// A moment as the file writes it: a year, a month, a day, and the time when there is one.
#[derive(Default)]
struct Moment {
  parts: [Option<i32>; 6],
}

impl Moment {
  fn written(&self) -> Option<String> {
    let [Some(year), Some(month), Some(day), hour, minute, second] = self.parts else {
      return None;
    };
    let [hour, minute, second] = [hour, minute, second].map(|part| part.unwrap_or(0));
    // A writer that does not know the moment writes zeros.
    let real = year >= 1
      && (1..=12).contains(&month)
      && (1..=31).contains(&day)
      && (0..24).contains(&hour)
      && (0..60).contains(&minute)
      && (0..60).contains(&second);
    real.then(|| format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}"))
  }
}

/// A character of Windows code page 1252 above 127: Latin-1, but for the 32 places where it
/// has typographic marks of its own. Five of those places hold nothing.
fn western(byte: u8) -> Option<char> {
  const MARKS: [char; 32] = [
    '\u{20AC}', '\0', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\0', '\u{017D}', '\0', '\0',
    '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}',
    '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\0', '\u{017E}', '\u{0178}',
  ];
  match byte {
    0x80..=0x9F => MARKS
      .get(usize::from(byte - 0x80))
      .copied()
      .filter(|mark| *mark != '\0'),
    _ => Some(char::from(byte)),
  }
}

/// The tag of a language, from the number Windows gives it. The numbers here are the ones
/// documents carry most; any other is no tag.
fn language(number: i32) -> Option<&'static str> {
  Some(match number {
    1025 => "ar-SA",
    1026 => "bg-BG",
    1027 => "ca-ES",
    1028 => "zh-TW",
    1029 => "cs-CZ",
    1030 => "da-DK",
    1031 => "de-DE",
    1032 => "el-GR",
    1033 => "en-US",
    1034 | 3082 => "es-ES",
    1035 => "fi-FI",
    1036 => "fr-FR",
    1037 => "he-IL",
    1038 => "hu-HU",
    1040 => "it-IT",
    1041 => "ja-JP",
    1042 => "ko-KR",
    1043 => "nl-NL",
    1044 => "nb-NO",
    1045 => "pl-PL",
    1046 => "pt-BR",
    1048 => "ro-RO",
    1049 => "ru-RU",
    1050 => "hr-HR",
    1051 => "sk-SK",
    1053 => "sv-SE",
    1054 => "th-TH",
    1055 => "tr-TR",
    1057 => "id-ID",
    1058 => "uk-UA",
    1060 => "sl-SI",
    1066 => "vi-VN",
    1081 => "hi-IN",
    2052 => "zh-CN",
    2055 => "de-CH",
    2057 => "en-GB",
    2058 => "es-MX",
    2060 => "fr-BE",
    2067 => "nl-BE",
    2070 => "pt-PT",
    3076 => "zh-HK",
    3079 => "de-AT",
    3081 => "en-AU",
    3084 => "fr-CA",
    4105 => "en-CA",
    4108 => "fr-CH",
    5129 => "en-NZ",
    6153 => "en-IE",
    7177 => "en-ZA",
    _ => return None,
  })
}

/// The number that says text is not to be checked for spelling, in place of a language's.
const UNCHECKED: i32 = 1024;

/// Where the text of a fact goes: to the fact, or aside when it has already been stated.
fn first(stated: &str, destination: Destination) -> Destination {
  if stated.is_empty() {
    destination
  } else {
    Destination::Aside
  }
}

/// The file, read once from its start.
struct Reader<'a> {
  bytes: &'a [u8],
  at: usize,
  /// The groups that are open around the current one.
  open: Vec<Group>,
  group: Group,
  /// The group opened with the mark that says a reader that does not know it may skip it.
  skippable: bool,
  /// How many stand-ins for the last character written by its number are still to come.
  standing_in: u32,
  /// The first half of a character written as two numbers.
  first_half: Option<u16>,
  body: Body,
  title: String,
  author: String,
  subject: String,
  keywords: String,
  generator: String,
  created: Moment,
  revised: Moment,
  pages: Option<i32>,
  language: Option<i32>,
  paper_width: Option<i32>,
  paper_height: Option<i32>,
  code_page: Option<u32>,
  /// How many characters of the text are in each language.
  spoken: Vec<(i32, u64)>,
  pictures: u32,
  /// A byte above 127 was written in a code page that is not read here.
  unread: bool,
  too_deep: bool,
}

impl<'a> Reader<'a> {
  fn new(bytes: &'a [u8]) -> Self {
    Reader {
      bytes,
      at: 0,
      open: Vec::new(),
      group: Group {
        destination: Destination::Body,
        outer: Destination::Body,
        stand_ins: 1,
        language: None,
        twice: false,
      },
      skippable: false,
      standing_in: 0,
      first_half: None,
      body: Body::default(),
      title: String::new(),
      author: String::new(),
      subject: String::new(),
      keywords: String::new(),
      generator: String::new(),
      created: Moment::default(),
      revised: Moment::default(),
      pages: None,
      language: None,
      paper_width: None,
      paper_height: None,
      code_page: None,
      spoken: Vec::new(),
      pictures: 0,
      unread: false,
      too_deep: false,
    }
  }

  /// A character of text, sent to whatever the current group is for.
  fn put(&mut self, character: char) {
    match self.group.destination {
      Destination::Body => {
        self.body.push(character);
        // Text that names no language of its own is in the document's.
        if let Some(language) = self.group.language.or(self.language)
          && !character.is_whitespace()
        {
          match self
            .spoken
            .iter_mut()
            .find(|(spoken, _)| *spoken == language)
          {
            Some((_, characters)) => *characters += 1,
            None => self.spoken.push((language, 1)),
          }
        }
      }
      Destination::Title => self.title.push(character),
      Destination::Author => self.author.push(character),
      Destination::Subject => self.subject.push(character),
      Destination::Keywords => self.keywords.push(character),
      Destination::Generator => self.generator.push(character),
      Destination::Aside
      | Destination::Shape
      | Destination::Info
      | Destination::Created
      | Destination::Revised => {}
    }
  }

  /// A byte of text, in the file's code page. One that stands in for a character already
  /// written by its number is passed over.
  fn byte(&mut self, byte: u8) {
    if self.standing_in > 0 {
      self.standing_in -= 1;
      return;
    }
    if byte < 0x80 {
      return self.put(char::from(byte));
    }
    match self.code_page {
      None | Some(1252) => {
        if let Some(character) = western(byte) {
          self.put(character);
        }
      }
      Some(_) => self.unread |= self.group.destination != Destination::Aside,
    }
  }

  /// A character written by its number: sixteen bits, signed, so that the upper half of the
  /// range is written below zero; and a character beyond that range as two of them.
  fn numbered(&mut self, number: i32) {
    self.standing_in = self.group.stand_ins;
    let Ok(unit) = u16::try_from(number.rem_euclid(0x1_0000)) else {
      return;
    };
    let character = match (self.first_half.take(), unit) {
      (_, 0xD800..=0xDBFF) => {
        self.first_half = Some(unit);
        return;
      }
      (Some(first), 0xDC00..=0xDFFF) => {
        let beyond = (u32::from(first) - 0xD800) << 10 | (u32::from(unit) - 0xDC00);
        char::from_u32(0x1_0000 + beyond)
      }
      _ => char::from_u32(u32::from(unit)),
    };
    if let Some(character) = character {
      self.put(character);
    }
  }

  /// A control word and the number after it, when it has one.
  fn word(&mut self, word: &[u8], number: Option<i32>) {
    use Destination::{
      Aside, Author, Body, Created, Generator, Info, Keywords, Revised, Shape, Subject, Title,
    };
    let skippable = std::mem::take(&mut self.skippable);
    // Any control word but the one for a character ends the run of stand-ins.
    if word != b"u" {
      self.standing_in = 0;
    }
    let within_info = matches!(self.group.outer, Info | Title | Author | Subject | Keywords)
      || self.group.destination == Info;
    match word {
      // Characters.
      b"u" => self.numbered(number.unwrap_or(0)),
      b"uc" => self.group.stand_ins = number.and_then(|n| u32::try_from(n).ok()).unwrap_or(1),
      b"par" | b"line" | b"sect" | b"page" | b"row" => self.put('\n'),
      b"tab" | b"cell" => self.put('\t'),
      b"emdash" => self.put('\u{2014}'),
      b"endash" => self.put('\u{2013}'),
      b"lquote" => self.put('\u{2018}'),
      b"rquote" => self.put('\u{2019}'),
      b"ldblquote" => self.put('\u{201C}'),
      b"rdblquote" => self.put('\u{201D}'),
      b"bullet" => self.put('\u{2022}'),
      b"emspace" | b"enspace" | b"qmspace" => self.put(' '),
      // Bytes kept as they are, which may hold anything.
      b"bin" => {
        let length = number.and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
        self.at = self.at.saturating_add(length);
      }
      // What the file says of itself outside its text.
      b"ansicpg" => self.code_page = number.and_then(|n| u32::try_from(n).ok()),
      b"mac" => self.code_page = Some(10_000),
      b"pc" => self.code_page = Some(437),
      b"pca" => self.code_page = Some(850),
      b"deflang" => self.language = number,
      // The language of the text from here on; one number says only that it is not to be
      // checked, and names none.
      b"lang" => self.group.language = number.filter(|language| *language != UNCHECKED),
      b"paperw" => self.paper_width = number,
      b"paperh" => self.paper_height = number,
      b"nofpages" => self.pages = number,
      // The parts of a moment.
      b"yr" | b"mo" | b"dy" | b"hr" | b"min" | b"sec" => {
        let part = match word {
          b"yr" => 0,
          b"mo" => 1,
          b"dy" => 2,
          b"hr" => 3,
          b"min" => 4,
          _ => 5,
        };
        match self.group.destination {
          Created => self.created.parts[part] = number,
          Revised => self.revised.parts[part] = number,
          _ => {}
        }
      }
      // What a group is for.
      b"info" => self.group.destination = Info,
      // A fact stated a second time is set aside: the first is the one to go by.
      b"title" if within_info => self.group.destination = first(&self.title, Title),
      b"author" if within_info => self.group.destination = first(&self.author, Author),
      b"subject" if within_info => self.group.destination = first(&self.subject, Subject),
      b"keywords" if within_info => self.group.destination = first(&self.keywords, Keywords),
      b"creatim" => self.group.destination = Created,
      b"revtim" => self.group.destination = Revised,
      b"generator" => self.group.destination = Generator,
      // A picture of the document's own is counted, and what it holds is not text.
      b"pict" => {
        if matches!(self.group.destination, Body | Shape) {
          self.pictures += 1;
        }
        self.group.destination = Aside;
      }
      // A shape is described once for readers that know shapes, and what an older reader is
      // to show in its place follows. The words of a text box are the document's.
      b"shpinst" if self.group.destination == Body => self.group.destination = Shape,
      b"shptxt" if self.group.destination == Shape => self.group.destination = Body,
      b"shprslt" => self.group.destination = Aside,
      // A picture written a second time for older readers is in the second of these two.
      b"shppict" => {}
      // The same text twice: plainly in the first group inside, and then in a group marked
      // as holding every character.
      b"upr" => self.group.twice = true,
      b"ud" => self.group.destination = self.group.outer,
      // The result of a field is its text; the instruction that made it is not.
      b"fldrslt" => {}
      b"fonttbl"
      | b"colortbl"
      | b"stylesheet"
      | b"listtable"
      | b"listoverridetable"
      | b"rsidtbl"
      | b"nonshppict"
      | b"header"
      | b"headerl"
      | b"headerr"
      | b"headerf"
      | b"footer"
      | b"footerl"
      | b"footerr"
      | b"footerf"
      | b"footnote"
      | b"annotation"
      | b"fldinst"
      | b"object"
      | b"objdata"
      | b"themedata"
      | b"colorschememapping"
      | b"latentstyles"
      | b"datastore"
      | b"datafield"
      | b"xmlnstbl"
      | b"filetbl"
      | b"revtbl"
      | b"listtext"
      | b"pntext"
      | b"operator"
      | b"company"
      | b"doccomm"
      | b"category"
      | b"manager"
      | b"printim"
      | b"buptim"
      | b"comment"
      | b"hlinkbase" => {
        self.group.destination = Aside;
      }
      // A group a reader may skip, of a kind that is not known here, is skipped.
      _ if skippable => self.group.destination = Aside,
      _ => {}
    }
  }

  /// What follows a backslash: a control word, a byte written as two hex digits, or a mark.
  fn escape(&mut self) {
    let Some(&next) = self.bytes.get(self.at) else {
      return;
    };
    if next.is_ascii_alphabetic() {
      let rest = &self.bytes[self.at..];
      let letters = rest
        .iter()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
      let after = &rest[letters..];
      let sign = usize::from(after.first() == Some(&b'-'));
      let digits = after[sign..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
      let number = (digits > 0)
        .then(|| {
          std::str::from_utf8(&after[..sign + digits])
            .ok()?
            .parse::<i32>()
            .ok()
        })
        .flatten();
      let length = letters + if digits > 0 { sign + digits } else { 0 };
      // One space after a control word ends it and is not text.
      let space = usize::from(rest.get(length) == Some(&b' '));
      self.at += length + space;
      return self.word(&rest[..letters], number);
    }
    self.at += 1;
    match next {
      b'\'' => {
        let digits = self.bytes.get(self.at..self.at + 2).unwrap_or_default();
        let value = std::str::from_utf8(digits)
          .ok()
          .and_then(|digits| u8::from_str_radix(digits, 16).ok());
        self.at += 2;
        if let Some(value) = value {
          self.byte(value);
        }
      }
      b'\\' | b'{' | b'}' => self.byte(next),
      b'~' => self.put('\u{A0}'),
      b'_' => self.put('-'),
      b'*' => self.skippable = true,
      // A line ending after a backslash ends a paragraph.
      b'\r' | b'\n' => self.put('\n'),
      _ => {}
    }
  }

  fn run(&mut self) {
    while let Some(&byte) = self.bytes.get(self.at) {
      self.at += 1;
      match byte {
        b'{' => {
          if self.open.len() >= MOST_DEPTH {
            self.too_deep = true;
            return;
          }
          self.open.push(self.group);
          self.standing_in = 0;
          self.skippable = false;
          let outer = self.group.destination;
          self.group = Group {
            // The plain copy of text that is written twice.
            destination: if self.group.twice {
              Destination::Aside
            } else {
              outer
            },
            outer,
            stand_ins: self.group.stand_ins,
            language: self.group.language,
            twice: false,
          };
        }
        b'}' => {
          self.standing_in = 0;
          match self.open.pop() {
            Some(group) => self.group = group,
            // The group that holds the whole document has closed.
            None => return,
          }
          if self.open.is_empty() {
            return;
          }
        }
        b'\\' => self.escape(),
        // Line endings in the file are not text.
        b'\r' | b'\n' | 0 => {}
        _ => self.byte(byte),
      }
    }
  }
}

fn filled(text: &str) -> Option<String> {
  let text = text.trim();
  (!text.is_empty()).then(|| text.to_string())
}

/// How many bytes stand in front of the document: a byte-order mark and blank space, which a
/// program that reads RTF as text may have written before it. Zero when it opens the file.
pub(crate) fn lead(source: &Source<'_>) -> u64 {
  let head = source.head(4096);
  let after_mark = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
  let opening = after_mark.trim_ascii_start();
  match opening.starts_with(b"{\\rtf") {
    true => u64::try_from(head.len() - opening.len()).unwrap_or(0),
    false => 0,
  }
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<DocumentFacts> {
  let whole = source.whole_file();
  let bytes = whole.unwrap_or_else(|| source.head(usize::MAX));
  let mut reader = Reader::new(bytes);
  reader.run();
  // The counts are over the bytes given, and the rest is asked for.
  if whole.is_none() {
    reading
      .needs
      .get_or_insert(source.rest_from(source.head_len()));
  }
  if reader.too_deep {
    reading.warn("its groups are nested deeper than is read");
  }
  if reader.unread {
    let page = reader.code_page.unwrap_or(0);
    reading.warn(format!(
      "characters above 127 written in code page {page} are not read"
    ));
  }
  // A page's sides are written in twentieths of a point.
  let points = |twentieths: Option<i32>| {
    twentieths
      .filter(|side| *side > 0)
      .map(|side| f64::from(side) / 20.0)
  };
  let generator = reader.generator.trim().trim_end_matches(';');
  // The language most of the text is in, and the document's own when its text names none.
  let most_spoken = reader
    .spoken
    .iter()
    .rev()
    .max_by_key(|(_, characters)| *characters)
    .map(|(spoken, _)| *spoken)
    .or(reader.language);
  let preview = reader.body.preview.trim_end();
  let detail = DocumentDetail::Rtf(RtfDetail {
    code_page: reader.code_page,
    generator: filled(generator),
  });
  Some(DocumentFacts {
    page_count: reader.pages.and_then(|pages| u32::try_from(pages).ok()),
    word_count: Some(reader.body.words),
    language: most_spoken.and_then(language).map(str::to_string),
    title: filled(&reader.title),
    author: filled(&reader.author),
    subject: filled(&reader.subject),
    keywords: reader
      .keywords
      .split([',', ';'])
      .filter_map(filled)
      .collect(),
    text_preview: filled(preview),
    page_width: points(reader.paper_width),
    page_height: points(reader.paper_height),
    image_count: Some(reader.pictures),
    created_date: reader.created.written(),
    modified_date: reader.revised.written(),
    ..DocumentFacts::unread(detail)
  })
}

#[cfg(test)]
mod tests;
