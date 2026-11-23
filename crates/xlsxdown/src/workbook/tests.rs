use super::*;
use crate::testing::{Parts, SheetSpec};

#[test]
fn styles_count_every_xf_and_resolve_format_codes() {
  let xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00&quot;/hr&quot;"/></numFmts><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="10"/><name val="Arial"/></font></fonts><cellXfs count="2"><xf numFmtId="4" fontId="1"><alignment vertical="center" wrapText="1" indent="1"/></xf><xf numFmtId="164" fontId="0"/></cellXfs></styleSheet>"##;
  let styles = parse_styles(xml).unwrap();
  assert_eq!(styles.xfs.len(), 2);
  assert_eq!(styles.xfs[0].vertical, Vertical::Center);
  assert!(styles.xfs[0].wrap);
  assert_eq!(styles.xfs[0].indent, 1);
  assert_eq!(styles.xfs[0].font_id, 1);
  assert!(styles.fonts[1].bold);
  assert_eq!(styles.fonts[1].name, "Arial");
  assert_eq!(format_code(&styles, 0).as_deref(), Some("#,##0.00"));
  assert_eq!(format_code(&styles, 1).as_deref(), Some("#,##0.00\"/hr\""));
  assert_eq!(format_code(&styles, 9), None);
}

fn sheet_of(xml: &[u8], styles: &Styles) -> Sheet {
  parse_sheet(
    xml,
    "S",
    0,
    false,
    WorkbookContext {
      shared: &[],
      styles,
      date1904: false,
    },
    None,
  )
  .unwrap()
}

#[test]
fn references_beyond_the_grid_are_malformed_and_rows_never_wrap() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1048576"><c><v>1</v></c></row><row><c><v>2</v></c><c r="A999999999"><v>3</v></c></row><row r="4294967295"><c><v>4</v></c></row><row><c><v>5</v></c></row></sheetData></worksheet>"##;
  let sheet = sheet_of(xml, &Styles::default());
  assert_eq!(
    sheet.cells.keys().collect::<Vec<_>>(),
    [&(1_048_575, 0)],
    "only the last row of the grid holds a cell: {:?}",
    sheet.warnings
  );
  assert!(sheet.cells.values().all(|c| c.text == "1"));
  assert!(
    sheet
      .warnings
      .iter()
      .filter(|w| w.contains("beyond"))
      .count()
      >= 3,
    "{:?}",
    sheet.warnings
  );
}

#[test]
fn general_cells_render_through_general_and_values_are_finite() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>0.30000000000000004</v></c><c r="B1"><v>427.49999999999994</v></c><c r="C1"><v>1e300</v></c><c r="D1"><v>NaN</v></c><c r="E1"><v>inf</v></c></row></sheetData></worksheet>"##;
  let sheet = sheet_of(xml, &Styles::default());
  assert_eq!(sheet.cells[&(0, 0)].text, "0.3");
  assert_eq!(sheet.cells[&(0, 0)].raw, "0.30000000000000004");
  assert_eq!(sheet.cells[&(0, 1)].text, "427.5");
  assert_eq!(sheet.cells[&(0, 2)].text, "1E+300");
  assert_eq!(
    sheet.cells[&(0, 3)].value,
    CellValue::Text("NaN".to_string())
  );
  assert_eq!(
    sheet.cells[&(0, 4)].value,
    CellValue::Text("inf".to_string())
  );
}

#[test]
fn builtin_ids_render_or_are_unsupported_and_a_files_own_code_wins() {
  let xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="4" formatCode="0.0"/></numFmts><cellXfs count="4"><xf numFmtId="44"/><xf numFmtId="27"/><xf numFmtId="4"/><xf numFmtId="7"/></cellXfs></styleSheet>"##;
  let styles = parse_styles(xml).unwrap();
  assert_eq!(
    format_code(&styles, 0).as_deref(),
    Some("_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)")
  );
  assert_eq!(
    format_code(&styles, 2).as_deref(),
    Some("0.0"),
    "the file's own code for id 4 wins"
  );
  assert_eq!(
    format_code(&styles, 3).as_deref(),
    Some("$#,##0.00_);($#,##0.00)")
  );
  let sheet_xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" s="0"><v>1234.5</v></c><c r="B1" s="1"><v>42247</v></c><c r="C1" s="1"><v>7</v></c></row></sheetData></worksheet>"##;
  let sheet = sheet_of(sheet_xml, &styles);
  assert_eq!(sheet.cells[&(0, 0)].text, "$1,234.50");
  let unknown = &sheet.cells[&(0, 1)];
  assert!(
    !unknown.format_supported,
    "built-in 27 has no en-US code this release knows"
  );
  assert_eq!(unknown.text, "42247", "the text stays the stored spelling");
  assert_eq!(
    unknown.value,
    CellValue::Date("2015-08-31".to_string()),
    "ids 27 to 36 and 50 to 58 are dates, whatever their spelling"
  );
  assert!(
    sheet
      .warnings
      .iter()
      .any(|w| w.contains("built-in") && w.contains("27")),
    "{:?}",
    sheet.warnings
  );
}

#[test]
fn a_multi_range_print_area_is_no_print_area() {
  assert_eq!(print_area_range("S!$A$1:$B$2,S!$D$1:$E$2"), None);
  assert_eq!(
    print_area_range("'My Sheet'!$A$1:$F$12").unwrap().a1(),
    "A1:F12"
  );
}

#[test]
fn geometry_from_the_file_is_finite_and_within_excels_maxima() {
  let styles_xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="1e999"/></font><font><sz val="NaN"/></font></fonts><cellXfs count="1"><xf numFmtId="0" fontId="0"/></cellXfs></styleSheet>"##;
  let styles = parse_styles(styles_xml).unwrap();
  assert_eq!(styles.fonts[0].size, 409.0);
  assert_eq!(styles.fonts[1].size, 11.0);
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetFormatPr defaultRowHeight="inf" defaultColWidth="-5"/><cols><col min="1" max="1" width="1e999"/><col min="2" max="2" width="NaN"/></cols><sheetData><row r="1" ht="1e999"><c r="A1"><v>1</v></c></row><row r="2" ht="-3"><c r="A2"><v>2</v></c></row></sheetData></worksheet>"##;
  let sheet = sheet_of(xml, &styles);
  assert_eq!(
    sheet.default_row_height, 409.5,
    "an overflowing spelling is the maximum"
  );
  assert_eq!(
    sheet.default_col_width, None,
    "a negative width is no width"
  );
  assert_eq!(sheet.cols[0].width, 255.0);
  assert_eq!(
    sheet.cols[1].width, 0.0,
    "an unparsable width means the default"
  );
  assert_eq!(sheet.rows[&0].height, Some(409.5));
  assert_eq!(sheet.rows.get(&1).and_then(|r| r.height), None);
}

#[test]
fn time_only_codes_type_the_cell_as_time_and_numbers_carry_their_currency() {
  let styles_xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="[$&#8364;-407]#,##0.00"/></numFmts><cellXfs count="5"><xf numFmtId="0"/><xf numFmtId="20"/><xf numFmtId="22"/><xf numFmtId="164"/><xf numFmtId="4"/></cellXfs></styleSheet>"##;
  let styles = parse_styles(styles_xml).unwrap();
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" s="1"><v>0.75</v></c><c r="B1" s="2"><v>45161.5</v></c><c r="C1" s="3"><v>1234.5</v></c><c r="D1" s="4"><v>3</v></c><c r="E1"><v>2.5</v></c></row></sheetData></worksheet>"##;
  let sheet = sheet_of(xml, &styles);
  assert_eq!(
    sheet.cells[&(0, 0)].value,
    CellValue::Time("18:00:00".to_string())
  );
  assert_eq!(sheet.cells[&(0, 0)].text, "18:00");
  assert_eq!(
    sheet.cells[&(0, 1)].value,
    CellValue::Date("2023-08-23T12:00:00".to_string())
  );
  let number = |position: (u32, u32)| {
    let cell = &sheet.cells[&position];
    (cell.value.clone(), cell.currency_symbol.clone())
  };
  assert_eq!(
    number((0, 2)),
    (CellValue::Number(1234.5), Some("\u{20ac}".to_string()))
  );
  assert_eq!(number((0, 3)), (CellValue::Number(3.0), None));
  assert_eq!(number((0, 4)), (CellValue::Number(2.5), None));
}

#[test]
fn malformed_references_indices_and_merges_warn_and_skip() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="ZZZZ"><v>1</v></c><c r="B1" t="s"><v>99</v></c><c r="C1" t="s"><v>1</v></c></row></sheetData><mergeCells><mergeCell ref="nope"/></mergeCells></worksheet>"##;
  let shared = vec!["zero".to_string(), "one".to_string()];
  let sheet = parse_sheet(
    xml,
    "S",
    0,
    false,
    WorkbookContext {
      shared: &shared,
      styles: &Styles::default(),
      date1904: false,
    },
    None,
  )
  .unwrap();
  assert_eq!(sheet.warnings.len(), 3, "{:?}", sheet.warnings);
  assert_eq!(sheet.cells.len(), 1);
  assert_eq!(sheet.cells[&(0, 2)].text, "one");
}

#[test]
fn a_code_this_release_cannot_render_still_types_its_dates() {
  let styles_xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="5"><numFmt numFmtId="164" formatCode="[$-409]mmmm\ d\,\ yyyy;@"/><numFmt numFmtId="165" formatCode="[h]:mm:ss"/><numFmt numFmtId="166" formatCode="yyyy-mm-dd\ hh:mm:ss.000"/><numFmt numFmtId="167" formatCode="0.00E+00"/><numFmt numFmtId="168" formatCode="[$-F400]h:mm:ss\ AM/PM"/></numFmts><cellXfs count="6"><xf numFmtId="47"/><xf numFmtId="164"/><xf numFmtId="165"/><xf numFmtId="166"/><xf numFmtId="167"/><xf numFmtId="168"/></cellXfs></styleSheet>"##;
  let styles = parse_styles(styles_xml).unwrap();
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" s="0"><v>45161.5</v></c><c r="B1" s="1"><v>45161</v></c><c r="C1" s="2"><v>1.25</v></c><c r="D1" s="3"><v>45161.75</v></c><c r="E1" s="4"><v>45161</v></c><c r="F1" s="5"><v>0.25</v></c><c r="G1" s="0"><v>0.5</v></c><c r="H1" s="1"><v>60</v></c></row></sheetData></worksheet>"##;
  let sheet = sheet_of(xml, &styles);
  let value = |col: u32| sheet.cells[&(0, col)].value.clone();
  assert_eq!(
    value(0),
    CellValue::Date("2023-08-23T12:00:00".to_string()),
    "mm:ss.0 shows only a clock, but a serial that holds a day keeps it"
  );
  assert_eq!(
    value(1),
    CellValue::Date("2023-08-23".to_string()),
    "a locale directive does not change what day it is"
  );
  assert_eq!(
    value(2),
    CellValue::Number(1.25),
    "an elapsed duration is a number"
  );
  assert_eq!(value(3), CellValue::Date("2023-08-23T18:00:00".to_string()));
  assert_eq!(value(4), CellValue::Number(45161.0), "scientific notation");
  assert_eq!(value(5), CellValue::Time("06:00:00".to_string()));
  assert_eq!(value(6), CellValue::Time("12:00:00".to_string()));
  assert_eq!(value(7), CellValue::Number(60.0), "the day that never was");
  // None of these codes is rendered: the text is the stored spelling and the cell says so.
  for (col, stored) in [
    (0, "45161.5"),
    (1, "45161"),
    (2, "1.25"),
    (3, "45161.75"),
    (4, "45161"),
  ] {
    let cell = &sheet.cells[&(0, col)];
    assert_eq!(cell.text, stored);
    assert!(!cell.format_supported, "column {col}");
  }
  assert!(
    sheet
      .warnings
      .iter()
      .any(|w| w.contains("H1") && w.contains("serial 60")),
    "{:?}",
    sheet.warnings
  );
}

#[test]
fn a_clock_only_code_that_renders_keeps_the_day_of_a_serial_that_has_one() {
  let styles_xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cellXfs count="1"><xf numFmtId="20"/></cellXfs></styleSheet>"##;
  let styles = parse_styles(styles_xml).unwrap();
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" s="0"><v>0.75</v></c><c r="B1" s="0"><v>45161.75</v></c></row></sheetData></worksheet>"##;
  let sheet = sheet_of(xml, &styles);
  assert_eq!(
    sheet.cells[&(0, 0)].value,
    CellValue::Time("18:00:00".to_string())
  );
  assert_eq!(
    sheet.cells[&(0, 1)].value,
    CellValue::Date("2023-08-23T18:00:00".to_string())
  );
  // Both show the same clock, as Excel does.
  assert_eq!(sheet.cells[&(0, 0)].text, "18:00");
  assert_eq!(sheet.cells[&(0, 1)].text, "18:00");
  assert!(sheet.cells.values().all(|c| c.format_supported));
}

const SHARED: &[u8] = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="2"><c r="A2"><v>2</v></c><c r="B2"><f t="shared" ref="B2:C4" si="0">$A2*B$1+SUM(A:A)</f><v>4</v></c><c r="C2"><f t="shared" si="0"/><v>6</v></c></row><row r="3"><c r="B3"><f t="shared" si="0"/><v>8</v></c><c r="C3"><f t="shared" si="0">A3&amp;"kept"</f><v>9</v></c></row><row r="4"><c r="B4"><f t="shared" si="0"/></c><c r="D4"><f t="shared" si="7"/><v>1</v></c></row></sheetData></worksheet>"##;

fn formula_at(sheet: &Sheet, position: (u32, u32)) -> Option<&str> {
  sheet.cells[&position].formula.as_deref()
}

#[test]
fn followers_of_a_shared_formula_get_the_masters_formula_moved_to_them() {
  let mut sheet = sheet_of(SHARED, &Styles::default());
  assert_eq!(
    formula_at(&sheet, (2, 1)),
    None,
    "a part alone names the master"
  );
  let mut budget = MAX_EXPANDED_FORMULA_BYTES;
  expand_shared_formulas(&mut sheet, &mut budget);
  assert_eq!(
    formula_at(&sheet, (1, 1)),
    Some("$A2*B$1+SUM(A:A)"),
    "the master"
  );
  assert_eq!(
    formula_at(&sheet, (1, 2)),
    Some("$A2*C$1+SUM(B:B)"),
    "one column right"
  );
  assert_eq!(
    formula_at(&sheet, (2, 1)),
    Some("$A3*B$1+SUM(A:A)"),
    "one row down"
  );
  assert_eq!(
    formula_at(&sheet, (3, 1)),
    Some("$A4*B$1+SUM(A:A)"),
    "two rows down"
  );
  assert_eq!(
    formula_at(&sheet, (2, 2)),
    Some("A3&\"kept\""),
    "a follower that carries text keeps it"
  );
  for follower in [(1, 2), (2, 1), (2, 2), (3, 1)] {
    assert_eq!(sheet.cells[&follower].shared_master, Some((1, 1)));
  }
  assert_eq!(sheet.cells[&(1, 1)].shared_master, None);
  assert_eq!(sheet.cells[&(3, 1)].value, CellValue::Blank);
  assert!(
    !sheet.cells[&(3, 1)].cached,
    "a formula with no stored result"
  );
  assert_eq!(
    MAX_EXPANDED_FORMULA_BYTES - budget,
    3 * "$A2*B$1+SUM(A:A)".len(),
    "three followers were written out"
  );
  // D4 names a group that has no master: it is a cell with a warning and no formula.
  assert_eq!(formula_at(&sheet, (3, 3)), None);
  assert_eq!(sheet.cells[&(3, 3)].shared_master, None);
  assert!(
    sheet
      .warnings
      .iter()
      .any(|w| w.contains("D4") && w.contains("no master")),
    "{:?}",
    sheet.warnings
  );
}

#[test]
fn expansion_stops_at_the_budget_and_says_how_many_cells_went_without() {
  let mut sheet = sheet_of(SHARED, &Styles::default());
  let one = "$A2*B$1+SUM(A:A)".len();
  let mut budget = one + 3;
  expand_shared_formulas(&mut sheet, &mut budget);
  let written = [(1, 2), (2, 1), (3, 1)]
    .into_iter()
    .filter(|position| formula_at(&sheet, *position).is_some())
    .count();
  assert_eq!(written, 1, "the budget held one follower's text");
  assert_eq!(budget, 3);
  assert!(
    sheet
      .warnings
      .iter()
      .any(|w| w.starts_with("2 cell(s) follow a shared formula")),
    "{:?}",
    sheet.warnings
  );
}

#[test]
fn a_master_longer_than_a_formula_may_be_is_not_written_out() {
  let long = "1+".repeat(MAX_FORMULA_CHARS / 2) + "A1";
  let xml = format!(
    r##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="B1"><f t="shared" ref="B1:B2" si="0">{long}</f><v>1</v></c></row><row r="2"><c r="B2"><f t="shared" si="0"/><v>2</v></c></row></sheetData></worksheet>"##
  );
  let mut sheet = sheet_of(xml.as_bytes(), &Styles::default());
  let mut budget = MAX_EXPANDED_FORMULA_BYTES;
  expand_shared_formulas(&mut sheet, &mut budget);
  assert_eq!(formula_at(&sheet, (0, 1)), Some(long.as_str()));
  assert_eq!(formula_at(&sheet, (1, 1)), None);
  assert_eq!(sheet.cells[&(1, 1)].shared_master, Some((0, 1)));
  assert_eq!(budget, MAX_EXPANDED_FORMULA_BYTES);
  assert!(
    sheet
      .warnings
      .iter()
      .any(|w| w.starts_with("1 cell(s) follow")),
    "{:?}",
    sheet.warnings
  );
}

const PLAIN_MAIN: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const MACRO_MAIN: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const TEMPLATE_MAIN: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml";
const MACRO_TEMPLATE_MAIN: &str = "application/vnd.ms-excel.template.macroEnabled.main+xml";

/// A file of one sheet with 42 in its first cell, its main part of the content type given.
fn file_of_kind(main_type: &str) -> Vec<u8> {
  let sheet = SheetSpec::visible(
    "Costs",
    r#"<sheetData><row r="1"><c r="A1"><v>42</v></c></row></sheetData>"#,
  );
  Parts::workbook(&[sheet], "").package(main_type)
}

/// Whether the file opened carries macros, and whether it is a template.
fn nature_of(main_type: &str) -> (bool, bool) {
  let workbook = Workbook::open(file_of_kind(main_type)).unwrap();
  assert_eq!(
    workbook.sheets[0].cells[&(0, 0)].value,
    CellValue::Number(42.0),
    "{main_type}"
  );
  (workbook.has_macros, workbook.is_template)
}

#[test]
fn a_plain_workbook_carries_no_macros_and_is_no_template() {
  assert_eq!(nature_of(PLAIN_MAIN), (false, false));
}

#[test]
fn a_macro_enabled_workbook_opens_and_says_it_can_carry_macros() {
  assert_eq!(nature_of(MACRO_MAIN), (true, false));
}

#[test]
fn a_template_opens_and_says_it_is_one() {
  assert_eq!(nature_of(TEMPLATE_MAIN), (false, true));
}

#[test]
fn a_macro_enabled_template_says_both() {
  assert_eq!(nature_of(MACRO_TEMPLATE_MAIN), (true, true));
}

#[test]
fn a_stored_value_is_typed_and_shown_through_the_format_its_style_selects() {
  let xf = |num_fmt_id: u32| Xf {
    num_fmt_id,
    font_id: 0,
    horizontal: Horizontal::General,
    vertical: Vertical::Bottom,
    wrap: false,
    indent: 0,
  };
  let styles = Styles {
    xfs: vec![xf(0), xf(14), xf(4)],
    ..Styles::default()
  };
  let mut warnings = Vec::new();
  let mut cell =
    |stored: Stored, style: usize| Cell::styled(stored, style, &styles, false, "A1", &mut warnings);

  let date = cell(Stored::number(46349.0), 1);
  assert_eq!(date.value, CellValue::Date("2026-11-23".to_string()));
  assert_eq!(
    (date.text.as_str(), date.raw.as_str()),
    ("11/23/2026", "46349")
  );
  assert_eq!(date.format.as_deref(), Some("m/d/yyyy"));

  let amount = cell(Stored::number(1234.5), 2);
  assert_eq!(amount.value, CellValue::Number(1234.5));
  assert_eq!(
    (amount.text.as_str(), amount.raw.as_str()),
    ("1,234.50", "1234.5")
  );

  let plain = cell(Stored::number(42.0), 0);
  assert_eq!((plain.text.as_str(), plain.format.as_deref()), ("42", None));

  // Only a number goes through a number format.
  let label = cell(Stored::text("Total".to_string()), 1);
  assert_eq!(label.value, CellValue::Text("Total".to_string()));
  assert_eq!(
    (label.text.as_str(), label.raw.as_str()),
    ("Total", "Total")
  );
  let on = cell(Stored::boolean(true), 2);
  assert_eq!(
    (on.value.clone(), on.text.as_str()),
    (CellValue::Boolean(true), "TRUE")
  );
  let error = cell(Stored::error("#DIV/0!".to_string()), 0);
  assert_eq!(error.value, CellValue::Error("#DIV/0!".to_string()));

  // What the caller knows and the format does not is left for the caller to set.
  assert_eq!(
    (date.formula.clone(), date.cached, date.merge),
    (None, false, None)
  );
  assert!(warnings.is_empty(), "{warnings:?}");
}
