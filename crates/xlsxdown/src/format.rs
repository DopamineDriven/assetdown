//! Number formats: a cell's format code parsed into sections and tokens, and the display text it
//! produces for a value, in the en-US spellings the design settles on.
//!
//! Supported: General, fixed decimals with and without grouping, zero padding, percent, quoted and
//! escaped literals, padding (`_x`, one space) and fill (`*x`, dropped) tokens, color tags
//! (dropped), currency and accounting codes with negative sections, the date and time tokens
//! `y m d h s AM/PM` (where `m` is minutes next to `h` or `s`), the text section `@`. Unsupported,
//! returning `None` from `parse`: elapsed durations, scientific notation, fractions (a bare slash
//! among digit placeholders), conditions, locale directives, era and Buddhist-year tokens.

#[derive(Debug, Clone, PartialEq)]
enum DateToken {
  Year2,
  Year4,
  Month,
  Month2,
  MonthAbbr,
  MonthName,
  Day,
  Day2,
  DayAbbr,
  DayName,
  Hour,
  Hour2,
  Minute,
  Minute2,
  Second,
  Second2,
  AmPm,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
  Literal(String),
  /// `0`, `#` or `?`.
  Digit(char),
  Decimal,
  Comma,
  Percent,
  General,
  Text,
  Date(DateToken),
  /// A bare `/`: a separator between date tokens, a fraction bar between digit placeholders.
  Slash,
  /// A run of `m` whose meaning (month or minute) is settled once the whole section is read.
  Ambiguous(usize),
}

#[derive(Debug, Clone, PartialEq)]
struct Section {
  tokens: Vec<Token>,
  decimals: usize,
  /// `#` placeholders after the point: shown only when non-zero.
  optional_decimals: usize,
  int_min: usize,
  grouping: bool,
  scale: u32,
  percent: bool,
  has_number: bool,
  is_date: bool,
  has_time: bool,
  /// Year, month or day tokens: a calendar date, not only a clock.
  has_date_part: bool,
  is_text: bool,
}

/// A parsed format code: one to four sections for positive, negative, zero and text values.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NumberFormat {
  sections: Vec<Section>,
}

/// Excel refuses format codes longer than this; so do we, before any work on them.
const MAX_CODE_CHARS: usize = 255;
/// Julian day number of 1899-12-30: in the 1900 system a serial past the phantom leap day is this
/// many days short of its Julian day number.
const JULIAN_DAY_BEFORE_1900_SERIALS: i64 = 2_415_019;
/// Julian day number of 1904-01-01, serial 0 of the 1904 system.
const JULIAN_DAY_OF_1904_EPOCH: i64 = 2_416_481;
/// The serial Excel gives 29 February 1900, a day that never happened.
const PHANTOM_LEAP_DAY: i64 = 60;
const SECONDS_PER_DAY: i64 = 86_400;
/// No serial past this is a date in either system (year 9999 ends near 2.96 million); the cap
/// also keeps the float-to-integer conversions below exact.
const MAX_DATE_SERIAL: f64 = 5_000_000.0;
const MONTHS: [&str; 12] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];
const DAYS: [&str; 7] = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
];
/// Glyphs a format code places next to a number to name its currency.
const CURRENCY_GLYPHS: &[char] = &[
  '$', '€', '£', '¥', '₹', '₽', '₩', '₪', '₺', '฿', '₫', '₦', '₴', '₱', '₡', '₲', '₵', '₸', '₼',
  '¢', '֏', '៛',
];
const COLORS: [&str; 8] = [
  "black", "blue", "cyan", "green", "magenta", "red", "white", "yellow",
];

/// Split a code on `;` outside quotes and brackets.
fn split_sections(code: &str) -> Vec<String> {
  let mut sections = vec![String::new()];
  let mut quoted = false;
  let mut bracket = false;
  let mut escape = false;
  for c in code.chars() {
    if escape {
      sections.last_mut().expect("section").push(c);
      escape = false;
      continue;
    }
    match c {
      '\\' if !quoted => {
        escape = true;
        sections.last_mut().expect("section").push(c);
      }
      '"' if !bracket => {
        quoted = !quoted;
        sections.last_mut().expect("section").push(c);
      }
      '[' if !quoted => {
        bracket = true;
        sections.last_mut().expect("section").push(c);
      }
      ']' if !quoted => {
        bracket = false;
        sections.last_mut().expect("section").push(c);
      }
      ';' if !quoted && !bracket => sections.push(String::new()),
      _ => sections.last_mut().expect("section").push(c),
    }
  }
  sections
}

fn parse_section(text: &str) -> Option<Section> {
  let chars: Vec<char> = text.chars().collect();
  let mut tokens = Vec::new();
  let mut i = 0;
  while i < chars.len() {
    let c = chars[i];
    match c {
      '"' => {
        let end = chars[i + 1..].iter().position(|&x| x == '"')? + i + 1;
        tokens.push(Token::Literal(chars[i + 1..end].iter().collect()));
        i = end + 1;
      }
      '\\' => {
        tokens.push(Token::Literal(chars.get(i + 1)?.to_string()));
        i += 2;
      }
      '_' => {
        chars.get(i + 1)?;
        tokens.push(Token::Literal(" ".to_string()));
        i += 2;
      }
      '*' => {
        chars.get(i + 1)?;
        i += 2;
      }
      '[' => {
        let end = chars[i + 1..].iter().position(|&x| x == ']')? + i + 1;
        let inner: String = chars[i + 1..end].iter().collect();
        let lower = inner.to_ascii_lowercase();
        if let Some(rest) = inner.strip_prefix('$') {
          // `[$€]` and `[$€-407]` are a currency literal, the latter with a locale id we ignore;
          // `[$-409]` is a bare locale directive that changes month and day names: unsupported.
          let symbol = rest.split('-').next().unwrap_or("");
          if symbol.is_empty() {
            return None;
          }
          tokens.push(Token::Literal(symbol.to_string()));
        } else if COLORS.contains(&lower.as_str()) || lower.starts_with("color") {
          // color tags carry no text
        } else {
          // elapsed time, conditions and anything else bracketed
          return None;
        }
        i = end + 1;
      }
      '0' | '#' | '?' => {
        tokens.push(Token::Digit(c));
        i += 1;
      }
      '.' => {
        tokens.push(Token::Decimal);
        i += 1;
      }
      ',' => {
        tokens.push(Token::Comma);
        i += 1;
      }
      '%' => {
        tokens.push(Token::Percent);
        i += 1;
      }
      '@' => {
        tokens.push(Token::Text);
        i += 1;
      }
      '/' => {
        tokens.push(Token::Slash);
        i += 1;
      }
      'E' | 'e' if matches!(chars.get(i + 1), Some('+' | '-')) => return None,
      'G' | 'g'
        if chars[i..]
          .iter()
          .take(7)
          .collect::<String>()
          .eq_ignore_ascii_case("general") =>
      {
        tokens.push(Token::General);
        i += 7;
      }
      'A' | 'a' => {
        let rest: Vec<char> = chars[i..]
          .iter()
          .take(5)
          .map(|c| c.to_ascii_lowercase())
          .collect();
        if rest == ['a', 'm', '/', 'p', 'm'] {
          tokens.push(Token::Date(DateToken::AmPm));
          i += 5;
        } else if rest.len() >= 3 && rest[..3] == ['a', '/', 'p'] {
          tokens.push(Token::Date(DateToken::AmPm));
          i += 3;
        } else {
          tokens.push(Token::Literal(c.to_string()));
          i += 1;
        }
      }
      'y' | 'Y' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' => {
        let lower = c.to_ascii_lowercase();
        let run = chars[i..]
          .iter()
          .take_while(|x| x.to_ascii_lowercase() == lower)
          .count();
        tokens.push(match (lower, run) {
          ('y', 1..=2) => Token::Date(DateToken::Year2),
          ('y', _) => Token::Date(DateToken::Year4),
          ('m', n) => Token::Ambiguous(n),
          ('d', 1) => Token::Date(DateToken::Day),
          ('d', 2) => Token::Date(DateToken::Day2),
          ('d', 3) => Token::Date(DateToken::DayAbbr),
          ('d', _) => Token::Date(DateToken::DayName),
          ('h', 1) => Token::Date(DateToken::Hour),
          ('h', _) => Token::Date(DateToken::Hour2),
          ('s', 1) => Token::Date(DateToken::Second),
          ('s', _) => Token::Date(DateToken::Second2),
          _ => return None,
        });
        i += run;
      }
      'b' | 'B' | 'g' | 'G' | 'e' => return None,
      _ => {
        tokens.push(Token::Literal(c.to_string()));
        i += 1;
      }
    }
  }
  // An `m` run is minutes next to an hour or a second token, else a month.
  let dates: Vec<usize> = tokens
    .iter()
    .enumerate()
    .filter(|(_, t)| matches!(t, Token::Date(_) | Token::Ambiguous(_)))
    .map(|(i, _)| i)
    .collect();
  for (k, &idx) in dates.iter().enumerate() {
    if let Token::Ambiguous(n) = tokens[idx] {
      let prev_hour = k > 0
        && matches!(
          tokens[dates[k - 1]],
          Token::Date(DateToken::Hour | DateToken::Hour2)
        );
      let next_second = dates.get(k + 1).is_some_and(|&j| {
        matches!(
          tokens[j],
          Token::Date(DateToken::Second | DateToken::Second2)
        )
      });
      tokens[idx] = Token::Date(if prev_hour || next_second {
        if n >= 2 {
          DateToken::Minute2
        } else {
          DateToken::Minute
        }
      } else {
        match n {
          1 => DateToken::Month,
          2 => DateToken::Month2,
          3 => DateToken::MonthAbbr,
          _ => DateToken::MonthName,
        }
      });
    }
  }

  let has_number = tokens.iter().any(|t| matches!(t, Token::Digit(_)));
  let is_date = tokens.iter().any(|t| matches!(t, Token::Date(_)));
  let has_time = tokens.iter().any(|t| {
    matches!(
      t,
      Token::Date(
        DateToken::Hour
          | DateToken::Hour2
          | DateToken::Minute
          | DateToken::Minute2
          | DateToken::Second
          | DateToken::Second2
          | DateToken::AmPm
      )
    )
  });
  if is_date && has_number {
    return None;
  }
  if is_date {
    for t in tokens.iter_mut() {
      match t {
        Token::Decimal => *t = Token::Literal(".".to_string()),
        Token::Comma => *t = Token::Literal(",".to_string()),
        _ => {}
      }
    }
  }
  // A bare slash among digit placeholders is a fraction, which this release does not render.
  if has_number && tokens.iter().any(|t| matches!(t, Token::Slash)) {
    return None;
  }
  let has_date_part = tokens.iter().any(|t| {
    matches!(
      t,
      Token::Date(
        DateToken::Year2
          | DateToken::Year4
          | DateToken::Month
          | DateToken::Month2
          | DateToken::MonthAbbr
          | DateToken::MonthName
          | DateToken::Day
          | DateToken::Day2
          | DateToken::DayAbbr
          | DateToken::DayName
      )
    )
  });
  let is_text = tokens.iter().any(|t| matches!(t, Token::Text)) && !has_number && !is_date;

  // Number shape: digits before and after the point, grouping commas, scaling commas. Every
  // count comes from the tokens, so digits inside tags, quotes and padding markers do not count.
  let decimal_at = tokens.iter().position(|t| matches!(t, Token::Decimal));
  if tokens
    .iter()
    .filter(|t| matches!(t, Token::Decimal))
    .count()
    > 1
  {
    return None;
  }
  let int_tokens = &tokens[..decimal_at.unwrap_or(tokens.len())];
  let frac_tokens = decimal_at.map(|at| &tokens[at + 1..]).unwrap_or(&[]);
  let last_int_digit = int_tokens
    .iter()
    .rposition(|t| matches!(t, Token::Digit(_)));
  let int_min = int_tokens
    .iter()
    .filter(|t| matches!(t, Token::Digit('0')))
    .count();
  let grouping = last_int_digit.is_some_and(|last| {
    int_tokens[..last].iter().enumerate().any(|(i, t)| {
      matches!(t, Token::Comma) && int_tokens[..i].iter().any(|x| matches!(x, Token::Digit(_)))
    })
  });
  let scale = match tokens.iter().rposition(|t| matches!(t, Token::Digit(_))) {
    Some(last) => tokens[last + 1..]
      .iter()
      .take_while(|t| matches!(t, Token::Comma))
      .count() as u32,
    None => 0,
  };
  let decimals = frac_tokens
    .iter()
    .filter(|t| matches!(t, Token::Digit(_)))
    .count();
  let optional_decimals = frac_tokens
    .iter()
    .filter(|t| matches!(t, Token::Digit('#' | '?')))
    .count();
  let percent = tokens.iter().any(|t| matches!(t, Token::Percent));
  Some(Section {
    tokens,
    decimals,
    optional_decimals,
    int_min,
    grouping,
    scale,
    percent,
    has_number,
    is_date,
    has_time,
    has_date_part,
    is_text,
  })
}

/// What a format code makes of a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SerialKind {
  /// A number, however it is dressed.
  Number,
  /// A calendar date, with a clock time when the code shows one.
  Date { with_time: bool },
  /// A clock with no year, month or day.
  Clock,
  /// An elapsed duration such as `[h]:mm`: a count of days, not a moment.
  Duration,
}

/// A date or time letter of a code, in the order the code spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
  /// A year, a day, or an era or Buddhist-year letter.
  Calendar,
  Hour,
  Second,
  Meridiem,
  /// A run of `m`: minutes beside an hour or a second, else a month.
  MonthOrMinute,
}

/// What a code makes of a number, read off the code without rendering it. This is for codes
/// `NumberFormat::parse` turns down: it only has to tell a date from a clock from a number, so
/// it skips everything else and never fails. Only the first section counts, as for `parse`.
pub(crate) fn serial_kind(code: &str) -> SerialKind {
  let section = split_sections(code).into_iter().next().unwrap_or_default();
  let chars: Vec<char> = section.chars().collect();
  let mut marks: Vec<Mark> = Vec::new();
  let mut i = 0;
  while let Some(&c) = chars.get(i) {
    let rest = chars.get(i..).unwrap_or_default();
    let spells = |word: &str| {
      rest
        .iter()
        .map(char::to_ascii_lowercase)
        .take(word.len())
        .eq(word.chars())
    };
    match c.to_ascii_lowercase() {
      // Quoted text, to its closing quote.
      '"' => {
        i += 1 + rest.iter().skip(1).take_while(|&&x| x != '"').count() + 1;
      }
      // An escaped, padded or repeated character says nothing about the value.
      '\\' | '_' | '*' => i += 2,
      '[' => {
        let inner: String = rest.iter().skip(1).take_while(|&&x| x != ']').collect();
        let lower = inner.to_ascii_lowercase();
        let elapsed = ["h", "m", "s"]
          .iter()
          .any(|unit| !lower.is_empty() && lower.chars().all(|x| unit.starts_with(x)));
        if elapsed {
          return SerialKind::Duration;
        }
        // Colors, conditions, currency symbols and locale directives.
        i += inner.chars().count() + 2;
      }
      'g' if spells("general") => i += 7,
      'e' if matches!(rest.get(1), Some('+' | '-')) => i += 2,
      'a' if spells("am/pm") => {
        marks.push(Mark::Meridiem);
        i += 5;
      }
      'a' if spells("a/p") => {
        marks.push(Mark::Meridiem);
        i += 3;
      }
      // The era year is a small `e`; a capital on its own is a literal.
      'e' if c == 'E' => i += 1,
      letter @ ('y' | 'd' | 'e' | 'g' | 'b' | 'h' | 's' | 'm') => {
        marks.push(match letter {
          'h' => Mark::Hour,
          's' => Mark::Second,
          'm' => Mark::MonthOrMinute,
          _ => Mark::Calendar,
        });
        i += rest
          .iter()
          .take_while(|x| x.to_ascii_lowercase() == letter)
          .count();
        // Seconds may carry decimals (`ss.000`): the only digit placeholders a date code has.
        if letter == 's' && chars.get(i) == Some(&'.') {
          let decimals = chars
            .get(i + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|&&x| x == '0')
            .count();
          if decimals > 0 {
            i += 1 + decimals;
          }
        }
      }
      // Any other digit placeholder makes the code a number format, whatever letters it holds.
      '0' | '#' | '?' => return SerialKind::Number,
      _ => i += 1,
    }
  }
  let is_minute = |at: usize| {
    let before = at.checked_sub(1).and_then(|prev| marks.get(prev));
    before == Some(&Mark::Hour) || marks.get(at + 1) == Some(&Mark::Second)
  };
  let mut calendar = false;
  let mut clock = false;
  for (at, mark) in marks.iter().enumerate() {
    match mark {
      Mark::Calendar => calendar = true,
      Mark::Hour | Mark::Second | Mark::Meridiem => clock = true,
      Mark::MonthOrMinute if is_minute(at) => clock = true,
      Mark::MonthOrMinute => calendar = true,
    }
  }
  match (calendar, clock) {
    (true, with_time) => SerialKind::Date { with_time },
    (false, true) => SerialKind::Clock,
    (false, false) => SerialKind::Number,
  }
}

impl NumberFormat {
  /// Parse a format code; `None` when any section uses something this release does not render.
  pub(crate) fn parse(code: &str) -> Option<NumberFormat> {
    if code.chars().count() > MAX_CODE_CHARS {
      return None;
    }
    let sections = split_sections(code);
    if sections.is_empty() || sections.len() > 4 {
      return None;
    }
    let sections = sections
      .iter()
      .map(|s| parse_section(s))
      .collect::<Option<Vec<_>>>()?;
    Some(NumberFormat { sections })
  }

  /// Whether a serial under this code reads as a date or time.
  pub(crate) fn is_date(&self) -> bool {
    self.sections[0].is_date
  }

  /// Whether the date rendering carries a clock time.
  pub(crate) fn has_time(&self) -> bool {
    self.sections[0].has_time
  }

  /// Whether the code shows a calendar date (year, month or day tokens) rather than only a clock.
  pub(crate) fn has_date_part(&self) -> bool {
    self.sections[0].has_date_part
  }

  /// What this code makes of a number: itself, a calendar date, or a clock.
  pub(crate) fn serial_kind(&self) -> SerialKind {
    if !self.is_date() {
      SerialKind::Number
    } else if self.has_date_part() {
      SerialKind::Date {
        with_time: self.has_time(),
      }
    } else {
      SerialKind::Clock
    }
  }

  /// The currency the positive section names: a currency glyph, a bracketed `[$USD]` or
  /// `[$€-407]` symbol, or a quoted three-letter code such as `"USD "`.
  pub(crate) fn currency_symbol(&self) -> Option<String> {
    self.sections[0].tokens.iter().find_map(|t| match t {
      Token::Literal(text) => {
        let trimmed = text.trim();
        let mut chars = trimmed.chars();
        match (chars.next(), chars.next()) {
          (Some(c), None) if CURRENCY_GLYPHS.contains(&c) => Some(c.to_string()),
          _ if trimmed.len() == 3 && trimmed.chars().all(|c| c.is_ascii_uppercase()) => {
            Some(trimmed.to_string())
          }
          _ => None,
        }
      }
      _ => None,
    })
  }

  /// The display text for `value`; `None` when a date code meets a serial that is not a date.
  pub(crate) fn render(&self, value: f64, date1904: bool) -> Option<String> {
    let (section, negative) = match self.sections.len() {
      n if n >= 2 && value < 0.0 => (&self.sections[1], false),
      n if n >= 3 && value == 0.0 => (&self.sections[2], false),
      _ => (&self.sections[0], value < 0.0),
    };
    let magnitude = value.abs();
    let rendered = if section.is_text {
      render_general(value)
    } else if section.is_date {
      render_date(section, value, date1904)?
    } else {
      render_number(section, magnitude)
    };
    let rendered = rendered.trim().to_string();
    Some(if negative && !section.is_date && !section.is_text {
      format!("-{rendered}")
    } else {
      rendered
    })
  }
}

fn render_number(section: &Section, magnitude: f64) -> String {
  let mut v = magnitude;
  if section.percent {
    v *= 100.0;
  }
  for _ in 0..section.scale {
    v /= 1000.0;
  }
  let number = if section.has_number {
    let factor = 10f64.powi(section.decimals as i32);
    let rounded = (v * factor).round() / factor;
    let text = format!("{:.*}", section.decimals, rounded);
    let (int_part, frac_part) = text.split_once('.').unwrap_or((text.as_str(), ""));
    let mut int_digits = int_part.to_string();
    if int_digits == "0" && section.int_min == 0 {
      int_digits.clear();
    }
    if int_digits.len() < section.int_min {
      int_digits = format!(
        "{}{int_digits}",
        "0".repeat(section.int_min - int_digits.len())
      );
    }
    let grouped = if section.grouping {
      group_thousands(&int_digits)
    } else {
      int_digits
    };
    let mut frac = frac_part.to_string();
    if section.optional_decimals > 0 {
      let required = section.decimals - section.optional_decimals;
      while frac.len() > required && frac.ends_with('0') {
        frac.pop();
      }
    }
    if frac.is_empty() {
      grouped
    } else {
      format!("{grouped}.{frac}")
    }
  } else {
    String::new()
  };
  let mut out = String::new();
  let mut number_emitted = false;
  for token in &section.tokens {
    match token {
      Token::Literal(text) => out.push_str(text),
      Token::Slash => out.push('/'),
      Token::General => out.push_str(&render_general(magnitude)),
      Token::Percent => out.push('%'),
      Token::Digit(_) | Token::Decimal | Token::Comma => {
        if !number_emitted {
          out.push_str(&number);
          number_emitted = true;
        }
      }
      Token::Text | Token::Date(_) | Token::Ambiguous(_) => {}
    }
  }
  out
}

fn group_thousands(digits: &str) -> String {
  let mut out = String::new();
  for (i, c) in digits.chars().enumerate() {
    if i > 0 && (digits.len() - i).is_multiple_of(3) {
      out.push(',');
    }
    out.push(c);
  }
  out
}

fn render_date(section: &Section, serial: f64, date1904: bool) -> Option<String> {
  // A clock-only code needs no valid date: the fraction of the serial is the time of day.
  let (year, month, day, hour, minute, second, weekday) = if section.has_date_part {
    let (y, mo, d, h, mi, s) = serial_to_datetime(serial, date1904)?;
    (y, mo, d, h, mi, s, weekday_of(serial, date1904)?)
  } else {
    let (h, mi, s) = clock_of(serial);
    (0, 0, 0, h, mi, s, 0)
  };
  let twelve_hour = section
    .tokens
    .iter()
    .any(|t| matches!(t, Token::Date(DateToken::AmPm)));
  let mut out = String::new();
  for token in &section.tokens {
    match token {
      Token::Literal(text) => out.push_str(text),
      Token::Slash => out.push('/'),
      Token::Date(t) => out.push_str(&match t {
        DateToken::Year2 => format!("{:02}", year.rem_euclid(100)),
        DateToken::Year4 => format!("{year:04}"),
        DateToken::Month => month.to_string(),
        DateToken::Month2 => format!("{month:02}"),
        DateToken::MonthAbbr => MONTHS[month as usize - 1][..3].to_string(),
        DateToken::MonthName => MONTHS[month as usize - 1].to_string(),
        DateToken::Day => day.to_string(),
        DateToken::Day2 => format!("{day:02}"),
        DateToken::DayAbbr => DAYS[weekday][..3].to_string(),
        DateToken::DayName => DAYS[weekday].to_string(),
        DateToken::Hour | DateToken::Hour2 => {
          let h = if twelve_hour {
            match hour % 12 {
              0 => 12,
              h => h,
            }
          } else {
            hour
          };
          if matches!(t, DateToken::Hour2) {
            format!("{h:02}")
          } else {
            h.to_string()
          }
        }
        DateToken::Minute => minute.to_string(),
        DateToken::Minute2 => format!("{minute:02}"),
        DateToken::Second => second.to_string(),
        DateToken::Second2 => format!("{second:02}"),
        DateToken::AmPm => if hour < 12 { "AM" } else { "PM" }.to_string(),
      }),
      _ => {}
    }
  }
  Some(out)
}

/// General: integers without a point, otherwise up to 11 significant digits with trailing zeros
/// trimmed, which is how Excel shows an unformatted number in a wide enough cell.
pub(crate) fn render_general(value: f64) -> String {
  if !value.is_finite() {
    return value.to_string();
  }
  if value == 0.0 {
    return "0".to_string();
  }
  let exponent = decimal_exponent(value.abs());
  if !(-10..15).contains(&exponent) {
    return render_scientific(value, exponent);
  }
  if value.fract() == 0.0 {
    return format!("{}", value as i64);
  }
  let decimals = (10 - exponent).clamp(0, 20) as usize;
  let text = format!("{:.*}", decimals, value);
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() || trimmed == "-" {
    "0".to_string()
  } else {
    trimmed.to_string()
  }
}

/// The power of ten at or below `magnitude`, corrected for the rounding `log10` can introduce
/// just below a power of ten.
fn decimal_exponent(magnitude: f64) -> i32 {
  let mut exponent = magnitude.log10().floor() as i32;
  if 10f64.powi(exponent) > magnitude {
    exponent -= 1;
  } else if 10f64.powi(exponent + 1) <= magnitude {
    exponent += 1;
  }
  exponent
}

/// `1.23457E+20`: a mantissa of up to six significant digits and a signed two-digit exponent.
fn render_scientific(value: f64, exponent: i32) -> String {
  let mut exponent = exponent;
  let mut mantissa = value / 10f64.powi(exponent);
  let rounded = (mantissa * 1e5).round() / 1e5;
  if rounded.abs() >= 10.0 {
    exponent += 1;
    mantissa /= 10.0;
  }
  let text = format!("{:.5}", mantissa);
  let mantissa = text.trim_end_matches('0').trim_end_matches('.');
  let sign = if exponent < 0 { '-' } else { '+' };
  format!("{mantissa}E{sign}{:02}", exponent.abs())
}

/// The Julian day number a serial falls on and the second of that day, or `None` where the
/// serial is not a date: not finite, negative, past the cap, or the 1900 system's day 60.
fn julian_day(serial: f64, date1904: bool) -> Option<(i64, i64)> {
  if !serial.is_finite() || !(0.0..=MAX_DATE_SERIAL).contains(&serial) {
    return None;
  }
  let whole = serial.floor();
  // Both values are whole and within the cap checked above, so the casts are exact.
  let second = ((serial - whole) * 86_400.0).round() as i64;
  let day = whole as i64;
  // A fraction that rounds up to a full day belongs to the next day, at midnight.
  let (day, second) = if second >= SECONDS_PER_DAY {
    (day + 1, 0)
  } else {
    (day, second)
  };
  let julian = if date1904 {
    day + JULIAN_DAY_OF_1904_EPOCH
  } else {
    match day.cmp(&PHANTOM_LEAP_DAY) {
      // Before the phantom day the count has not yet been pushed one too far.
      std::cmp::Ordering::Less => day + JULIAN_DAY_BEFORE_1900_SERIALS + 1,
      std::cmp::Ordering::Equal => return None,
      std::cmp::Ordering::Greater => day + JULIAN_DAY_BEFORE_1900_SERIALS,
    }
  };
  Some((julian, second))
}

/// A Julian day number to its Gregorian `(year, month, day)`: the integer algorithm of the
/// Explanatory Supplement to the Astronomical Almanac (Richards, 2013), valid for the positive
/// day numbers a serial produces.
fn gregorian_date(julian: i64) -> (i64, i64, i64) {
  let f = julian + 1401 + (((4 * julian + 274_277) / 146_097) * 3) / 4 - 38;
  let e = 4 * f + 3;
  let g = (e % 1461) / 4;
  let h = 5 * g + 2;
  let day = (h % 153) / 5 + 1;
  let month = (h / 153 + 2) % 12 + 1;
  let year = e / 1461 - 4716 + (14 - month) / 12;
  (year, month, day)
}

/// A serial to civil date and clock time, or `None` for a negative serial, the phantom 1900
/// leap day, or a date past year 9999.
pub(crate) fn serial_to_datetime(
  serial: f64,
  date1904: bool,
) -> Option<(i32, u32, u32, u32, u32, u32)> {
  let (julian, second) = julian_day(serial, date1904)?;
  let (year, month, day) = gregorian_date(julian);
  let year = i32::try_from(year)
    .ok()
    .filter(|y| (1..=9999).contains(y))?;
  let second = u32::try_from(second).ok()?;
  Some((
    year,
    u32::try_from(month).ok()?,
    u32::try_from(day).ok()?,
    second / 3600,
    (second % 3600) / 60,
    second % 60,
  ))
}

/// The time of day a serial's fraction holds, rounded to the second (a fraction that rounds up
/// to a whole day is midnight, as Excel shows it).
fn clock_of(serial: f64) -> (u32, u32, u32) {
  let seconds = ((serial.rem_euclid(1.0) * 86_400.0).round() as u32) % 86_400;
  (seconds / 3600, (seconds % 3600) / 60, seconds % 60)
}

/// `HH:MM:SS` of a serial's fraction, for a cell under a clock-only code.
pub(crate) fn iso_time(serial: f64) -> String {
  let (h, m, s) = clock_of(serial);
  format!("{h:02}:{m:02}:{s:02}")
}

/// `YYYY-MM-DD`, or `YYYY-MM-DDTHH:MM:SS` with `with_time`.
pub(crate) fn iso_date(serial: f64, date1904: bool, with_time: bool) -> Option<String> {
  let (y, mo, d, h, mi, s) = serial_to_datetime(serial, date1904)?;
  Some(if with_time {
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
  } else {
    format!("{y:04}-{mo:02}-{d:02}")
  })
}

/// The day of the week a serial falls on, Sunday as 0, to index `DAYS`.
fn weekday_of(serial: f64, date1904: bool) -> Option<usize> {
  let (julian, _) = julian_day(serial, date1904)?;
  // Julian day number 0 was a Monday.
  usize::try_from((julian + 1).rem_euclid(7)).ok()
}

#[cfg(test)]
mod tests;
