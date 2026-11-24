use super::*;

#[test]
fn renders_the_corpus_codes_and_the_standard_ones() {
  let r = |code: &str, v: f64| NumberFormat::parse(code).unwrap().render(v, false).unwrap();
  assert_eq!(r("#,##0.00\"/hr\"", 142.5), "142.50/hr");
  assert_eq!(r("0.0", 3.0), "3.0");
  assert_eq!(r("#,##0.00", 1234.5), "1,234.50");
  assert_eq!(r("0.00%", 0.075), "7.50%");
  assert_eq!(r("$#,##0.00;($#,##0.00)", -5.0), "($5.00)");
  assert_eq!(
    r(
      "_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)",
      1234.5
    ),
    "$1,234.50"
  );
  assert_eq!(r("[Red]0.00", -1.5), "-1.50");
  assert_eq!(r("00000", 123.0), "00123");
  assert_eq!(r("mm/dd/yy", 42247.0), "08/31/15");
  assert_eq!(r("m/d/yyyy", 42247.0), "8/31/2015");
  assert_eq!(r("yyyy-mm-dd hh:mm:ss", 45161.5), "2023-08-23 12:00:00");
  assert_eq!(r("h:mm AM/PM", 45161.75), "6:00 PM");
  assert_eq!(r("d-mmm-yy", 42247.0), "31-Aug-15");
  assert_eq!(r("@", 7.0), "7");
  assert!(
    NumberFormat::parse("[h]:mm:ss").is_none(),
    "elapsed time is unsupported"
  );
  assert!(NumberFormat::parse("0.00E+00").is_none());
  assert!(
    NumberFormat::parse("# ?/?").is_none(),
    "fractions are unsupported"
  );
  assert!(
    NumberFormat::parse("# ??/16").is_none(),
    "fixed-denominator fractions are unsupported"
  );
  assert!(
    NumberFormat::parse("m/d/yyyy").is_some(),
    "a slash between date tokens is a separator"
  );
  assert!(
    NumberFormat::parse("0.00\"/hr\"").is_some(),
    "a quoted slash is a literal"
  );
  assert!(
    NumberFormat::parse("[$-407]mmmm d, yyyy").is_none(),
    "a month named in another language is unsupported"
  );
  assert!(NumberFormat::parse("mm/dd/yy").unwrap().is_date());
  assert!(
    !NumberFormat::parse("#,##0.00\"/hr\"").unwrap().is_date(),
    "a quoted h is a literal"
  );
}

#[test]
fn review_findings_hostile_and_unusual_codes() {
  let r = |code: &str, v: f64| NumberFormat::parse(code).unwrap().render(v, false).unwrap();
  // A bare `a` followed by a multi-byte character is a literal, never a byte-sliced panic.
  assert_eq!(r("0 a€", 5.0), "5 a€");
  assert_eq!(r("0 aaé", 5.0), "5 aaé");
  assert_eq!(r("0 A中", 5.0), "5 A中");
  // Digits inside color tags and after padding markers are not placeholders.
  assert_eq!(r("[Color10]0.00", 1.5), "1.50");
  assert_eq!(r("[Color50]#,##0", 1234.0), "1,234");
  assert_eq!(r("_00.00", 1.5), "1.50");
  assert_eq!(r("*00.00", 1.5), "1.50");
  // Scaling commas after the decimals divide by a thousand each.
  assert_eq!(r("0.0,,", 1234567.0), "1.2");
  assert_eq!(r("#,##0.0,", 1234567.0), "1,234.6");
  assert_eq!(r("#,##0,,\"M\"", 2500000.0), "3M");
  // Points and commas inside a date code are separators.
  assert_eq!(r("dd.mm.yyyy", 42247.0), "31.08.2015");
  assert_eq!(r("mmmm d, yyyy", 42247.0), "August 31, 2015");
  assert_eq!(r("yyyy.mm.dd hh:mm", 45161.5), "2023.08.23 12:00");
  assert!(NumberFormat::parse("dd.mm.yyyy").unwrap().is_date());
  // Excel caps a format code at 255 characters; longer codes are unsupported, not slow.
  assert!(NumberFormat::parse(&"0".repeat(255)).is_some());
  assert!(NumberFormat::parse(&"0".repeat(256)).is_none());
  // Random codes over the whole token alphabet, multi-byte characters included, never panic.
  let alphabet: Vec<char> = "0#?.,%;\"\\[]_*@/:-+ ()$€中😀aAmMdDhHsSyYeEgGbBpP"
    .chars()
    .collect();
  let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
  let mut next = || {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
  };
  for _ in 0..4000 {
    let len = (next() % 24) as usize;
    let code: String = (0..len)
      .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
      .collect();
    if let Some(parsed) = NumberFormat::parse(&code) {
      for v in [0.0, -1.5, 0.075, 42247.0, 1e300, -1e-300] {
        let _ = parsed.render(v, false);
        let _ = parsed.render(v, true);
      }
    }
  }
}

#[test]
fn currency_symbols_and_time_only_codes() {
  let f = |code: &str| NumberFormat::parse(code).unwrap();
  // A bracketed currency with a locale id is that currency; the id names a language we ignore.
  assert_eq!(
    f("[$€-407]#,##0.00").render(1234.5, false).as_deref(),
    Some("€1,234.50")
  );
  assert_eq!(
    f("[$€-407]#,##0.00").currency_symbol().as_deref(),
    Some("€")
  );
  assert_eq!(f("[$USD] #,##0").currency_symbol().as_deref(), Some("USD"));
  assert_eq!(
    f("\"USD \"#,##0.00").currency_symbol().as_deref(),
    Some("USD")
  );
  assert_eq!(
    f("$#,##0.00;($#,##0.00)").currency_symbol().as_deref(),
    Some("$")
  );
  assert_eq!(f("£#,##0").currency_symbol().as_deref(), Some("£"));
  assert_eq!(f("#,##0.00\"/hr\"").currency_symbol(), None);
  assert_eq!(f("0.00%").currency_symbol(), None);
  assert_eq!(
    f("_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)")
      .currency_symbol()
      .as_deref(),
    Some("$")
  );
  assert_eq!(
    f("[$-409]mmmm d, yyyy").currency_symbol(),
    None,
    "a bare locale id names no currency"
  );
  // Time-only codes carry a clock and no date.
  assert!(f("h:mm").is_date());
  assert!(!f("h:mm").has_date_part());
  assert!(!f("h:mm AM/PM").has_date_part());
  assert!(!f("mm:ss").has_date_part());
  assert!(f("m/d/yyyy h:mm").has_date_part());
  assert_eq!(f("h:mm").render(0.75, false).as_deref(), Some("18:00"));
  assert_eq!(
    f("h:mm AM/PM").render(60.5, false).as_deref(),
    Some("12:00 PM"),
    "the clock needs no valid date"
  );
  assert_eq!(iso_time(0.5), "12:00:00");
  assert_eq!(iso_time(45161.75), "18:00:00");
  assert_eq!(
    iso_time(0.999_999),
    "00:00:00",
    "rounds up to the next day like Excel"
  );
}

#[test]
fn general_and_serials() {
  assert_eq!(render_general(3.0), "3");
  assert_eq!(render_general(1234.567), "1234.567");
  assert_eq!(render_general(0.1 + 0.2), "0.3");
  assert_eq!(render_general(-0.0), "0");
  assert_eq!(render_general(999_999_999_999_999.0), "999999999999999");
  assert_eq!(render_general(1e15), "1E+15");
  assert_eq!(render_general(123_456_789_012_345_680_000.0), "1.23457E+20");
  assert_eq!(render_general(-2.5e20), "-2.5E+20");
  assert_eq!(render_general(1e-10), "0.0000000001");
  assert_eq!(render_general(1.5e-12), "1.5E-12");
  assert_eq!(render_general(9.9999999e-12), "1E-11");
  assert_eq!(
    iso_date(42247.0, false, false).as_deref(),
    Some("2015-08-31")
  );
  assert_eq!(
    iso_date(45161.5, false, true).as_deref(),
    Some("2023-08-23T12:00:00")
  );
  assert_eq!(iso_date(59.0, false, false).as_deref(), Some("1900-02-28"));
  assert_eq!(iso_date(60.0, false, false), None);
  assert_eq!(iso_date(61.0, false, false).as_deref(), Some("1900-03-01"));
  assert_eq!(iso_date(0.0, true, false).as_deref(), Some("1904-01-01"));
  assert_eq!(iso_date(-1.0, false, false), None);
}

/// A calendar that only knows how long each month is, stepped one day at a time.
fn day_after((year, month, day): (i32, u32, u32)) -> (i32, u32, u32) {
  let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
  let length = match month {
    4 | 6 | 9 | 11 => 30,
    2 if leap => 29,
    2 => 28,
    _ => 31,
  };
  if day < length {
    (year, month, day + 1)
  } else if month < 12 {
    (year, month + 1, 1)
  } else {
    (year + 1, 1, 1)
  }
}

#[test]
fn every_serial_from_march_1900_to_the_end_of_9999_names_the_day_a_stepped_calendar_reaches() {
  let mut date = (1900, 3, 1);
  let mut serial = 61u32;
  while date != (9999, 12, 31) {
    let (year, month, day) = date;
    assert_eq!(
      serial_to_datetime(f64::from(serial), false),
      Some((year, month, day, 0, 0, 0)),
      "serial {serial}"
    );
    date = day_after(date);
    serial += 1;
  }
  assert_eq!(serial, 2_958_465, "the serial Excel gives 9999-12-31");
  assert_eq!(
    iso_date(f64::from(serial), false, false).as_deref(),
    Some("9999-12-31")
  );
  assert_eq!(serial_to_datetime(f64::from(serial + 1), false), None);
}

#[test]
fn the_first_two_months_of_1900_sit_before_a_day_that_never_was() {
  let mut date = (1899, 12, 31);
  for serial in 0u32..60 {
    let (year, month, day) = date;
    assert_eq!(
      serial_to_datetime(f64::from(serial), false),
      Some((year, month, day, 0, 0, 0)),
      "serial {serial}"
    );
    date = day_after(date);
  }
  assert_eq!(date, (1900, 3, 1), "the day after serial 59 is 1 March");
  assert_eq!(serial_to_datetime(60.0, false), None);
  assert_eq!(serial_to_datetime(60.5, false), None);
  assert_eq!(
    serial_to_datetime(59.999_999_999, false),
    None,
    "a fraction that rounds up lands on day 60"
  );
}

#[test]
fn the_1904_system_runs_1462_days_behind_the_1900_system() {
  for serial in (1462u32..2_958_465).step_by(997) {
    assert_eq!(
      serial_to_datetime(f64::from(serial - 1462), true),
      serial_to_datetime(f64::from(serial), false),
      "serial {serial}"
    );
  }
  assert_eq!(
    serial_to_datetime(0.0, true),
    Some((1904, 1, 1, 0, 0, 0)),
    "day 60 is an ordinary day here"
  );
  assert_eq!(serial_to_datetime(60.0, true), Some((1904, 3, 1, 0, 0, 0)));
}

#[test]
fn the_clock_rounds_to_the_second_and_carries_into_the_next_day() {
  assert_eq!(
    serial_to_datetime(45161.75, false),
    Some((2023, 8, 23, 18, 0, 0))
  );
  assert_eq!(
    serial_to_datetime(45161.0 + 86_399.4 / 86_400.0, false),
    Some((2023, 8, 23, 23, 59, 59))
  );
  assert_eq!(
    serial_to_datetime(45161.0 + 86_399.6 / 86_400.0, false),
    Some((2023, 8, 24, 0, 0, 0))
  );
  for not_a_date in [f64::NAN, f64::INFINITY, -0.5, 5_000_001.0] {
    assert_eq!(serial_to_datetime(not_a_date, false), None, "{not_a_date}");
  }
}

#[test]
fn weekdays_are_the_real_ones() {
  let day_name = |serial: f64, date1904: bool| {
    NumberFormat::parse("dddd")
      .unwrap()
      .render(serial, date1904)
      .unwrap()
  };
  assert_eq!(day_name(25_569.0, false), "Thursday", "1970-01-01");
  assert_eq!(day_name(44_927.0, false), "Sunday", "2023-01-01");
  assert_eq!(day_name(1.0, false), "Monday", "1900-01-01");
  assert_eq!(day_name(61.0, false), "Thursday", "1900-03-01");
  assert_eq!(day_name(0.0, true), "Friday", "1904-01-01");
  // Seven consecutive serials name seven different days, in order.
  let week: Vec<String> = (0..7)
    .map(|i| day_name(44_927.0 + f64::from(i), false))
    .collect();
  assert_eq!(
    week,
    [
      "Sunday",
      "Monday",
      "Tuesday",
      "Wednesday",
      "Thursday",
      "Friday",
      "Saturday"
    ]
  );
}

#[test]
fn a_code_says_what_it_makes_of_a_number_without_being_rendered() {
  let date = |with_time| SerialKind::Date { with_time };
  for (code, expected) in [
    ("General", SerialKind::Number),
    ("0.00", SerialKind::Number),
    ("#,##0.00_);[Red](#,##0.00)", SerialKind::Number),
    ("0.00E+00", SerialKind::Number),
    ("# ?/?", SerialKind::Number),
    ("@", SerialKind::Number),
    (
      "[<=9999999]###\\-####;\\(###\\)\\ ###\\-####",
      SerialKind::Number,
    ),
    ("0 \"days\"", SerialKind::Number),
    ("0\\d", SerialKind::Number),
    ("[$USD] #,##0.00", SerialKind::Number),
    ("#,##0 d", SerialKind::Number),
    ("m/d/yyyy", date(false)),
    ("d-mmm-yy", date(false)),
    ("mmmm", date(false)),
    ("yyyy\"\u{5e74}\"m\"\u{6708}\"d\"\u{65e5}\"", date(false)),
    ("[$-409]mmmm\\ d\\,\\ yyyy;@", date(false)),
    ("[$-F800]dddd\\,\\ mmmm\\ dd\\,\\ yyyy", date(false)),
    (
      "[$-411]ggge\"\u{5e74}\"m\"\u{6708}\"d\"\u{65e5}\"",
      date(false),
    ),
    ("m/d/yyyy h:mm", date(true)),
    ("yyyy-mm-dd\\ hh:mm:ss.000", date(true)),
    ("d/m/yy h:mm AM/PM", date(true)),
    ("h:mm", SerialKind::Clock),
    ("h:mm:ss AM/PM", SerialKind::Clock),
    ("mm:ss", SerialKind::Clock),
    ("mmss.0", SerialKind::Clock),
    ("mm:ss.0;@", SerialKind::Clock),
    ("[$-F400]h:mm:ss\\ AM/PM", SerialKind::Clock),
    ("[h]:mm:ss", SerialKind::Duration),
    ("[mm]:ss", SerialKind::Duration),
    ("[ss]", SerialKind::Duration),
    ("", SerialKind::Number),
    ("\"unclosed y", SerialKind::Number),
    ("[unclosed y", SerialKind::Number),
  ] {
    assert_eq!(serial_kind(code), expected, "{code:?}");
  }
}

#[test]
fn the_two_ways_of_reading_a_code_agree_wherever_both_can_read_it() {
  for code in [
    "0",
    "0.00",
    "#,##0",
    "0%",
    "\"$\"#,##0.00",
    "#,##0.00\"/hr\"",
    "$#,##0.00_);($#,##0.00)",
    "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
    "@",
    "General",
    "m/d/yyyy",
    "mm/dd/yy",
    "m/d/yy;@",
    "d-mmm-yy",
    "d-mmm",
    "mmm-yy",
    "dddd, mmmm d, yyyy",
    "m/d/yyyy h:mm",
    "h:mm AM/PM",
    "h:mm:ss AM/PM",
    "h:mm",
    "h:mm:ss",
    "mm:ss",
    "yyyy-mm-dd hh:mm:ss",
  ] {
    let parsed = NumberFormat::parse(code).unwrap_or_else(|| panic!("{code:?} should parse"));
    assert_eq!(parsed.serial_kind(), serial_kind(code), "{code:?}");
  }
}

#[test]
fn a_code_that_only_says_which_language_it_is_in_renders_where_the_language_changes_nothing() {
  let r = |code: &str, v: f64| NumberFormat::parse(code).unwrap().render(v, false).unwrap();
  // English, wherever it is spoken: the names of months and days are the ones written here.
  assert_eq!(r("[$-409]mmmm d, yyyy", 42247.0), "August 31, 2015");
  assert_eq!(r("[$-409]mmmm\\-yy;@", 42247.0), "August-15");
  assert_eq!(r("[$-409]d\\-mmm\\-yy;@", 42247.0), "31-Aug-15");
  assert_eq!(r("[$-809]dd/mm/yyyy", 42247.0), "31/08/2015");
  assert_eq!(
    r("[$-C09]dddd, d mmmm yyyy", 42247.0),
    "Monday, 31 August 2015"
  );
  assert_eq!(r("[$-409]h:mm AM/PM", 0.75), "6:00 PM");
  assert_eq!(r("[$-en-US]mmmm d, yyyy", 42247.0), "August 31, 2015");
  assert_eq!(r("[$-en-GB]d mmmm yyyy", 42247.0), "31 August 2015");
  // The long date and the time of whoever opens the file: the code beside the mark is what
  // any reader but Excel shows.
  assert_eq!(
    r("[$-F800]dddd\\,\\ mmmm\\ dd\\,\\ yyyy", 42247.0),
    "Monday, August 31, 2015"
  );
  assert_eq!(r("[$-F400]h:mm:ss\\ AM/PM", 0.25), "6:00:00 AM");
  // Another language changes nothing where the code names no month, day, or half of the day.
  assert_eq!(r("[$-407]dd.mm.yyyy", 42247.0), "31.08.2015");
  assert_eq!(r("[$-40C]dd/mm/yyyy hh:mm", 42247.5), "31/08/2015 12:00");
  assert_eq!(r("[$-fr-FR]dd/mm/yyyy", 42247.0), "31/08/2015");
  // Where it does name one, the name would be in that language, which is not written here.
  for code in [
    "[$-407]d. mmmm yyyy",
    "[$-40C]dddd d mmmm yyyy",
    "[$-407]d. mmm yy",
    "[$-411]h:mm AM/PM",
    "[$-de-DE]d. mmmm yyyy",
  ] {
    assert!(NumberFormat::parse(code).is_none(), "{code:?}");
  }
  // Another calendar or another set of digits is not this release's to write.
  assert!(NumberFormat::parse("[$-1070409]d/m/yyyy").is_none());
  assert!(NumberFormat::parse("[$-D00041E]d/m/yyyy").is_none());
  // And what is not a language at all is refused.
  for code in ["[$-]d/m/yyyy", "[$-xyz]d/m/yyyy", "[$]d/m/yyyy"] {
    assert!(NumberFormat::parse(code).is_none(), "{code:?}");
  }
  // A number under such a mark is still a number.
  assert_eq!(r("[$-409]#,##0.00", 1234.5), "1,234.50");
}

#[test]
fn a_language_mark_that_also_names_the_everyday_calendar_and_digits_renders_like_one_that_does_not()
{
  let r = |code: &str, v: f64| NumberFormat::parse(code).unwrap().render(v, false).unwrap();
  // The digits before the language say which calendar and which digits: 1 is the calendar
  // the code is written in anyway, and so is 2; 1 again, further left, is the plain digits.
  assert_eq!(r("[$-10409]m/d/yyyy", 42247.0), "8/31/2015");
  assert_eq!(
    r("[$-10409]m/d/yyyy\\ h:mm\\ AM/PM", 42247.5),
    "8/31/2015 12:00 PM"
  );
  assert_eq!(
    r("[$-10409]m/d/yyyy\\ h:mm:ss\\ AM/PM", 42247.75),
    "8/31/2015 6:00:00 PM"
  );
  assert_eq!(r("[$-10409]#,##0;\\(#,##0\\)", 2468.4), "2,468");
  assert_eq!(r("[$-10409]#,##0;\\(#,##0\\)", -2468.4), "(2,468)");
  assert_eq!(r("[$-10409]#,##0.0", 6.2744), "6.3");
  assert_eq!(r("[$-20409]mmmm d, yyyy", 42247.0), "August 31, 2015");
  assert_eq!(r("[$-1010409]d-mmm-yy", 42247.0), "31-Aug-15");
  assert_eq!(r("[$-10407]dd.mm.yyyy", 42247.0), "31.08.2015");
  // Another calendar counts the years another way, and other digits are other characters.
  for code in [
    "[$-30411]m/d/yyyy",
    "[$-60401]m/d/yyyy",
    "[$-70409]m/d/yyyy",
    "[$-1070409]d/m/yyyy",
    "[$-2010409]m/d/yyyy",
    "[$-D00041E]d/m/yyyy",
  ] {
    assert!(NumberFormat::parse(code).is_none(), "{code:?}");
  }
  // A month named in another language is still not written here, whatever the calendar.
  assert!(NumberFormat::parse("[$-10407]d. mmmm yyyy").is_none());
}
