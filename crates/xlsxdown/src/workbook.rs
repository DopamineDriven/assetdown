//! The workbook model: parts to a sparse cell model with styles, shared formulas and caches.
//!
//! Parts are found through relationships, never by conventional name. Cells keep their stored
//! spelling (`raw`), their typed value, the formula stored on them or the shared master they
//! follow, and whether a result was cached. Numeric `text` is the format module's rendering of
//! the value under the cell's format code; a code this release does not render leaves `text` as
//! the stored spelling and marks the cell unsupported.

use std::collections::{BTreeMap, HashMap};

use ooxmldown::{DocProps, Package, PackageError, PackageKind, parse_xml};
use roxmltree::{Document, Node};

use crate::address::{CellRange, MAX_COLS, MAX_ROWS, cell_ref, parse_cell_ref};
use crate::error::SpreadsheetError;
use crate::format::{NumberFormat, SerialKind, iso_date, iso_time, render_general, serial_kind};
use crate::formula::shift_formula;
use crate::source::Source;

/// A workbook with more cells or merges than this is refused rather than modelled.
pub const MAX_CELLS: usize = 2_000_000;
pub const MAX_MERGES: usize = 200_000;
/// Excel's limit on the length of a formula. A shared formula longer than this is not written
/// out for the cells that follow it.
pub const MAX_FORMULA_CHARS: usize = 8_192;
/// Formula text written out for the followers of shared formulas, across the whole workbook.
/// Past it, followers name their master and carry no text of their own.
pub const MAX_EXPANDED_FORMULA_BYTES: usize = 64 << 20;
/// Excel's maxima for a column width (characters), a row height (points) and a font size (points).
const MAX_COL_WIDTH: f64 = 255.0;
const MAX_ROW_HEIGHT: f64 = 409.5;
const MAX_FONT_SIZE: f64 = 409.0;

/// What a cell holds, typed; dates carry both the serial and its ISO rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
  Text(String),
  Number(f64),
  Boolean(bool),
  Error(String),
  /// The ISO rendering of a serial under a date code; the serial itself stays in `raw`.
  Date(String),
  /// `HH:MM:SS` of a serial's fraction under a clock-only code; the serial stays in `raw`.
  Time(String),
  Blank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Horizontal {
  #[default]
  General,
  Left,
  Center,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vertical {
  Top,
  Center,
  #[default]
  Bottom,
}

/// One `cellXfs` entry: the number format, font and alignment a cell style selects.
#[derive(Debug, Clone, PartialEq)]
pub struct Xf {
  pub num_fmt_id: u32,
  pub font_id: usize,
  pub horizontal: Horizontal,
  pub vertical: Vertical,
  pub wrap: bool,
  pub indent: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontSpec {
  pub name: String,
  pub size: f64,
  pub bold: bool,
  pub italic: bool,
}

impl FontSpec {
  fn calibri_11() -> FontSpec {
    FontSpec {
      name: "Calibri".to_string(),
      size: 11.0,
      bold: false,
      italic: false,
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Styles {
  pub xfs: Vec<Xf>,
  pub fonts: Vec<FontSpec>,
  pub custom_formats: HashMap<u32, String>,
  /// The Normal style's font, which SpreadsheetML column widths are measured in.
  pub normal_font: FontSpec,
}

impl Default for Styles {
  fn default() -> Self {
    Styles {
      xfs: Vec::new(),
      fonts: vec![FontSpec::calibri_11()],
      custom_formats: HashMap::new(),
      normal_font: FontSpec::calibri_11(),
    }
  }
}

/// Built-in number formats by id (ECMA-376 §18.8.30), in their en-US spellings.
fn builtin_format(id: u32) -> Option<&'static str> {
  Some(match id {
    1 => "0",
    2 => "0.00",
    3 => "#,##0",
    4 => "#,##0.00",
    5 => "$#,##0_);($#,##0)",
    6 => "$#,##0_);[Red]($#,##0)",
    7 => "$#,##0.00_);($#,##0.00)",
    8 => "$#,##0.00_);[Red]($#,##0.00)",
    9 => "0%",
    10 => "0.00%",
    11 => "0.00E+00",
    12 => "# ?/?",
    13 => "# ??/??",
    14 => "m/d/yyyy",
    15 => "d-mmm-yy",
    16 => "d-mmm",
    17 => "mmm-yy",
    18 => "h:mm AM/PM",
    19 => "h:mm:ss AM/PM",
    20 => "h:mm",
    21 => "h:mm:ss",
    22 => "m/d/yyyy h:mm",
    37 => "#,##0 ;(#,##0)",
    38 => "#,##0 ;[Red](#,##0)",
    39 => "#,##0.00;(#,##0.00)",
    40 => "#,##0.00;[Red](#,##0.00)",
    41 => "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)",
    42 => "_($* #,##0_);_($* (#,##0);_($* \"-\"_);_(@_)",
    43 => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
    44 => "_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)",
    45 => "mm:ss",
    46 => "[h]:mm:ss",
    47 => "mmss.0",
    48 => "##0.0E+0",
    49 => "@",
    _ => return None,
  })
}

/// The format code a cell style selects: the file's own `<numFmt>` for the id when it has one,
/// else the built-in spelling; `None` for General, an unknown style, or a built-in id this
/// release has no spelling for (see `unknown_builtin_id`).
pub fn format_code(styles: &Styles, style: usize) -> Option<String> {
  let id = styles.xfs.get(style)?.num_fmt_id;
  if id == 0 {
    return None;
  }
  styles
    .custom_formats
    .get(&id)
    .cloned()
    .or_else(|| builtin_format(id).map(str::to_string))
}

/// A non-zero built-in id with neither a `<numFmt>` nor a spelling in the table: a format this
/// release cannot render and cannot even name.
fn unknown_builtin_id(styles: &Styles, style: usize) -> Option<u32> {
  let id = styles.xfs.get(style)?.num_fmt_id;
  (id != 0 && !styles.custom_formats.contains_key(&id) && builtin_format(id).is_none())
    .then_some(id)
}

/// A length from the file: a non-negative number capped at Excel's maximum for it (an overflowing
/// spelling such as `1e999` is the maximum); `NaN` and negatives are no length.
fn length_attr(value: Option<&str>, max: f64) -> Option<f64> {
  value
    .and_then(|v| v.parse::<f64>().ok())
    .filter(|v| !v.is_nan() && *v >= 0.0)
    .map(|v| v.min(max))
}

fn attr<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
  node.attribute(name)
}

fn is_on(value: Option<&str>) -> bool {
  !matches!(value, Some("0" | "false" | "off"))
}

/// `styles.xml`: fonts, custom number formats and every `cellXfs` entry, with or without children.
pub(crate) fn parse_styles(bytes: &[u8]) -> Result<Styles, SpreadsheetError> {
  let doc = parse_xml(bytes)?;
  let mut styles = Styles {
    fonts: Vec::new(),
    ..Styles::default()
  };
  let root = doc.root_element();
  let children = |name: &str| {
    root
      .children()
      .find(|n| n.is_element() && n.tag_name().name() == name)
  };
  if let Some(fonts) = children("fonts") {
    for font in fonts
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "font")
    {
      let mut spec = FontSpec::calibri_11();
      for prop in font.children().filter(|n| n.is_element()) {
        match prop.tag_name().name() {
          "name" => spec.name = attr(prop, "val").unwrap_or("Calibri").to_string(),
          "sz" => spec.size = length_attr(attr(prop, "val"), MAX_FONT_SIZE).unwrap_or(11.0),
          "b" => spec.bold = is_on(attr(prop, "val")),
          "i" => spec.italic = is_on(attr(prop, "val")),
          _ => {}
        }
      }
      styles.fonts.push(spec);
    }
  }
  if styles.fonts.is_empty() {
    styles.fonts.push(FontSpec::calibri_11());
  }
  styles.normal_font = styles.fonts[0].clone();
  if let Some(formats) = children("numFmts") {
    for f in formats
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "numFmt")
    {
      if let (Some(id), Some(code)) = (
        attr(f, "numFmtId").and_then(|v| v.parse().ok()),
        attr(f, "formatCode"),
      ) {
        styles.custom_formats.insert(id, code.to_string());
      }
    }
  }
  if let Some(xfs) = children("cellXfs") {
    for xf in xfs
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "xf")
    {
      let alignment = xf
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "alignment");
      let horizontal = match alignment.and_then(|a| attr(a, "horizontal")) {
        Some("left") => Horizontal::Left,
        Some("center" | "centerContinuous") => Horizontal::Center,
        Some("right") => Horizontal::Right,
        _ => Horizontal::General,
      };
      let vertical = match alignment.and_then(|a| attr(a, "vertical")) {
        Some("top") => Vertical::Top,
        Some("center") => Vertical::Center,
        _ => Vertical::Bottom,
      };
      styles.xfs.push(Xf {
        num_fmt_id: attr(xf, "numFmtId")
          .and_then(|v| v.parse().ok())
          .unwrap_or(0),
        font_id: attr(xf, "fontId").and_then(|v| v.parse().ok()).unwrap_or(0),
        horizontal,
        vertical,
        wrap: alignment
          .is_some_and(|a| is_on(attr(a, "wrapText")) && attr(a, "wrapText").is_some()),
        indent: alignment
          .and_then(|a| attr(a, "indent"))
          .and_then(|v| v.parse().ok())
          .unwrap_or(0),
      });
    }
  }
  Ok(styles)
}

/// `sharedStrings.xml`: every `<si>` keeps its slot; runs concatenate; phonetic runs are skipped.
pub(crate) fn parse_shared_strings(bytes: &[u8]) -> Result<Vec<String>, SpreadsheetError> {
  let doc = parse_xml(bytes)?;
  Ok(
    doc
      .root_element()
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "si")
      .map(|si| rich_text(si))
      .collect(),
  )
}

/// The text of an `<si>` or `<is>`: every `<t>` in order, except those inside `<rPh>`.
fn rich_text(node: Node<'_, '_>) -> String {
  let mut out = String::new();
  for t in node
    .descendants()
    .filter(|n| n.is_element() && n.tag_name().name() == "t")
  {
    let phonetic = t
      .ancestors()
      .any(|a| a.is_element() && a.tag_name().name() == "rPh");
    if !phonetic {
      out.push_str(t.text().unwrap_or(""));
    }
  }
  out
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColSpec {
  pub min: u32,
  pub max: u32,
  pub width: f64,
  pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RowSpec {
  pub height: Option<f64>,
  pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
  pub value: CellValue,
  pub raw: String,
  pub text: String,
  pub format: Option<String>,
  pub format_supported: bool,
  /// The currency a Number cell's format names, when it names one.
  pub currency_symbol: Option<String>,
  /// The cell's formula, without the leading `=`: the text stored on the cell, or for a cell
  /// that follows a shared formula the master's text with its relative references moved here.
  pub formula: Option<String>,
  /// The master cell of the shared formula this cell follows; `None` on the master itself and
  /// on a cell with a formula of its own.
  pub shared_master: Option<(u32, u32)>,
  /// Whether a result was stored beside the formula or value.
  pub cached: bool,
  pub style: usize,
  pub merge: Option<CellRange>,
  /// The style's built-in number format id when this release has no spelling for it.
  pub builtin_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
  pub name: String,
  pub index: u32,
  pub hidden: bool,
  pub dimension: Option<CellRange>,
  pub print_area: Option<CellRange>,
  pub cols: Vec<ColSpec>,
  pub rows: BTreeMap<u32, RowSpec>,
  pub default_row_height: f64,
  pub default_col_width: Option<f64>,
  pub cells: BTreeMap<(u32, u32), Cell>,
  pub merges: Vec<CellRange>,
  pub drawings: usize,
  /// Links the sheet lists. One link may cover a range of cells.
  pub links: usize,
  /// Tables the sheet names.
  pub tables: usize,
  /// Cells with a note or a comment thread.
  pub comments: usize,
  /// Pictures and charts on the sheet's drawing, those inside groups included.
  pub pictures: usize,
  pub charts: usize,
  pub pivot_tables: usize,
  pub warnings: Vec<String>,
}

impl Sheet {
  pub fn col_hidden(&self, col: u32) -> bool {
    self
      .cols
      .iter()
      .any(|c| c.hidden && col + 1 >= c.min && col < c.max)
  }

  pub fn row_hidden(&self, row: u32) -> bool {
    self.rows.get(&row).is_some_and(|r| r.hidden)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Workbook {
  pub sheets: Vec<Sheet>,
  pub date1904: bool,
  pub styles: Styles,
  /// What the workbook says about itself: its title, author and dates, and the application that
  /// saved it. Every field is absent when the file does not state it.
  pub props: DocProps,
  /// Names the workbook defines, the built-in ones (`_xlnm.Print_Area` and the like) left out.
  pub defined_names: usize,
  /// The file is macro-enabled. Macros are never read or run.
  pub has_macros: bool,
  /// The file is a template.
  pub is_template: bool,
}

impl Workbook {
  /// Read a workbook from the bytes of an `.xlsx` file, or of a macro-enabled workbook or a
  /// template (`.xlsm`, `.xltx`, `.xltm`), which hold the same parts. A package that can be read
  /// but is none of these is an unsupported format, named for what it is instead.
  pub fn open(bytes: Vec<u8>) -> Result<Workbook, SpreadsheetError> {
    let mut package = Package::open(bytes)?;
    let kind = match package.kind() {
      Ok(kind) => kind,
      Err(limit @ PackageError::LimitExceeded(_)) => return Err(limit.into()),
      // A zip without a readable package structure is not an Office document at all.
      Err(PackageError::Malformed(_)) => PackageKind::Other,
    };
    let (has_macros, is_template) = match kind {
      PackageKind::Workbook => (false, false),
      PackageKind::MacroWorkbook => (true, false),
      // The package layer has one kind for both templates; the main part's type tells them apart.
      PackageKind::TemplateWorkbook => (macro_enabled(&mut package)?, true),
      other => {
        return Err(SpreadsheetError::UnsupportedFormat(unsupported_detail(
          other,
        )));
      }
    };
    Ok(Workbook {
      has_macros,
      is_template,
      ..parse_workbook(&mut package)?
    })
  }

  /// The sheets a caller asked for, in workbook order: those named in `names` whether hidden or
  /// not, or with no names every visible sheet, plus the hidden ones when `hidden_sheets` is set.
  pub fn selected_sheets(&self, names: Option<&[String]>, hidden_sheets: bool) -> Vec<&Sheet> {
    self
      .sheets
      .iter()
      .filter(|sheet| match names {
        Some(names) => names.contains(&sheet.name),
        None => hidden_sheets || !sheet.hidden,
      })
      .collect()
  }
}

/// Whether the main part is of a macro-enabled content type.
fn macro_enabled(package: &mut Package) -> Result<bool, SpreadsheetError> {
  let main = package.officedocument()?;
  Ok(
    package
      .content_type(&main)?
      .is_some_and(|content_type| content_type.contains("macroEnabled")),
  )
}

/// What a package this crate does not open is, in the words a caller reads.
fn unsupported_detail(kind: PackageKind) -> String {
  match kind {
    PackageKind::StrictWorkbook => {
      format!("{} (not supported in this release)", kind.description())
    }
    PackageKind::Workbook
    | PackageKind::MacroWorkbook
    | PackageKind::TemplateWorkbook
    | PackageKind::WordDocument
    | PackageKind::MacroDocument
    | PackageKind::TemplateDocument
    | PackageKind::Presentation
    | PackageKind::MacroPresentation
    | PackageKind::TemplatePresentation
    | PackageKind::SlideShow
    | PackageKind::Other => kind.description().to_string(),
  }
}

/// One `<sheet>` of `workbook.xml`, before its part is read.
struct SheetEntry {
  name: String,
  relationship_id: String,
  hidden: bool,
}

/// `Form!$A$1:$F$12` (the sheet part possibly quoted) to a range.
/// `Sheet!$A$1:$F$12` to a range; `None` for a union of ranges (a top-level comma), which is no
/// print area for the page.
fn print_area_range(text: &str) -> Option<CellRange> {
  let mut quoted = false;
  for c in text.chars() {
    match c {
      '\'' => quoted = !quoted,
      ',' if !quoted => return None,
      _ => {}
    }
  }
  let range = text.rsplit_once('!').map(|(_, r)| r).unwrap_or(text);
  CellRange::parse(range.trim())
}

/// Read a whole workbook from its package.
pub fn parse_workbook(package: &mut Package) -> Result<Workbook, SpreadsheetError> {
  let main = package.officedocument()?;
  let props = package.doc_props()?;
  read_workbook(package, &main, props)
}

/// Read the workbook whose main part is `main` from wherever its parts are kept.
pub(crate) fn read_workbook(
  source: &mut dyn Source,
  main: &str,
  props: DocProps,
) -> Result<Workbook, SpreadsheetError> {
  let rels = source.relationships(main)?;
  let by_id: HashMap<&str, &str> = rels
    .iter()
    .filter(|r| !r.external)
    .map(|r| (r.id.as_str(), r.target.as_str()))
    .collect();
  let target_of_kind = |suffix: &str| {
    rels
      .iter()
      .find(|r| !r.external && r.kind.ends_with(suffix))
      .map(|r| r.target.clone())
  };

  let workbook_xml = source.part(main)?;
  let doc = parse_xml(&workbook_xml)?;
  let root = doc.root_element();
  let mut date1904 = false;
  let mut defined_names = 0usize;
  let mut entries = Vec::new();
  let mut print_areas: HashMap<usize, CellRange> = HashMap::new();
  let mut print_area_warnings: HashMap<usize, String> = HashMap::new();
  for n in root.descendants().filter(|n| n.is_element()) {
    match n.tag_name().name() {
      "workbookPr" => date1904 = matches!(attr(n, "date1904"), Some("1" | "true")),
      "sheet" => entries.push(SheetEntry {
        name: attr(n, "name").unwrap_or("").to_string(),
        relationship_id: attr(n, "id").unwrap_or("").to_string(),
        hidden: matches!(attr(n, "state"), Some("hidden" | "veryHidden")),
      }),
      "definedName" if !attr(n, "name").is_some_and(|name| name.starts_with("_xlnm.")) => {
        defined_names += 1;
      }
      "definedName" if attr(n, "name") == Some("_xlnm.Print_Area") => {
        if let (Some(sheet), Some(text)) = (
          attr(n, "localSheetId").and_then(|v| v.parse::<usize>().ok()),
          n.text(),
        ) {
          match print_area_range(text) {
            Some(range) => {
              print_areas.insert(sheet, range);
            }
            None => {
              print_area_warnings.insert(
                sheet,
                "the print area is not a single rectangle; the page is the content extent"
                  .to_string(),
              );
            }
          }
        }
      }
      _ => {}
    }
  }
  if entries.is_empty() {
    return Err(SpreadsheetError::Malformed(
      "the workbook lists no sheets".to_string(),
    ));
  }

  let shared = match target_of_kind("/sharedStrings") {
    Some(path) => parse_shared_strings(&source.part(&path)?)?,
    None => Vec::new(),
  };
  let styles = match target_of_kind("/styles") {
    Some(path) => parse_styles(&source.part(&path)?)?,
    None => Styles::default(),
  };

  let mut sheets = Vec::new();
  let mut cells_total = 0usize;
  let mut merges_total = 0usize;
  let mut expansion_budget = MAX_EXPANDED_FORMULA_BYTES;
  for (position, entry) in entries.iter().enumerate() {
    let Some(path) = by_id.get(entry.relationship_id.as_str()) else {
      return Err(SpreadsheetError::Malformed(format!(
        "sheet \"{}\" has no relationship for {}",
        entry.name, entry.relationship_id
      )));
    };
    let index = u32::try_from(position)
      .map_err(|_| SpreadsheetError::Malformed("the workbook lists too many sheets".to_string()))?;
    let bytes = source.part(path)?;
    let mut sheet = parse_sheet(
      &bytes,
      &entry.name,
      index,
      entry.hidden,
      WorkbookContext {
        shared: &shared,
        styles: &styles,
        date1904,
      },
      print_areas.get(&position).copied(),
    )?;
    if let Some(warning) = print_area_warnings.remove(&position) {
      sheet.warnings.push(warning);
    }
    read_related(source, path, &mut sheet)?;
    expand_shared_formulas(&mut sheet, &mut expansion_budget);
    cells_total += sheet.cells.len();
    merges_total += sheet.merges.len();
    if cells_total > MAX_CELLS {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the workbook has more than {MAX_CELLS} cells"
      )));
    }
    if merges_total > MAX_MERGES {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the workbook has more than {MAX_MERGES} merged ranges"
      )));
    }
    sheets.push(sheet);
  }
  Ok(Workbook {
    sheets,
    date1904,
    styles,
    props,
    defined_names,
    has_macros: false,
    is_template: false,
  })
}

/// How many element children of `node` are named `name`.
fn named_children(node: Node<'_, '_>, name: &str) -> usize {
  node
    .children()
    .filter(|child| child.is_element() && child.tag_name().name() == name)
    .count()
}

/// Cells with a note or a comment thread. A thread leaves a `comment` here as well, as the
/// placeholder Excel writes for readers that predate threads.
fn count_comments(doc: &Document<'_>) -> usize {
  doc
    .descendants()
    .filter(|n| n.is_element() && n.tag_name().name() == "comment")
    .count()
}

/// The pictures and the charts on a drawing, in that order, those inside groups included.
/// Content a drawing offers a second time, as the fallback for a reader without some feature,
/// is not counted twice.
fn count_drawing(doc: &Document<'_>) -> (usize, usize) {
  let (mut pictures, mut charts) = (0, 0);
  let mut pending = vec![doc.root_element()];
  while let Some(node) = pending.pop() {
    for child in node.children().filter(|child| child.is_element()) {
      match child.tag_name().name() {
        "Fallback" => {}
        "pic" => pictures += 1,
        "chart" => charts += 1,
        _ => pending.push(child),
      }
    }
  }
  (pictures, charts)
}

/// A part a sheet can be read without, handed to `read` as XML. `None`, with a warning on the
/// sheet, when the part is missing or is not XML. A limit error is still an error.
fn related_part<T>(
  source: &mut dyn Source,
  path: &str,
  warnings: &mut Vec<String>,
  read: impl FnOnce(&Document<'_>) -> T,
) -> Result<Option<T>, SpreadsheetError> {
  let parsed = source
    .part(path)
    .and_then(|bytes| parse_xml(&bytes).map(|doc| read(&doc)));
  match parsed {
    Ok(value) => Ok(Some(value)),
    Err(PackageError::Malformed(detail)) => {
      warnings.push(format!("{path} was left out: {detail}"));
      Ok(None)
    }
    Err(limit) => Err(limit.into()),
  }
}

/// Count what a sheet holds outside its own part: the cells with comments, the pictures and
/// charts on its drawing, and its pivot tables. All of it is reached through the sheet's
/// relationships, so a sheet without any has none of these.
fn read_related(
  source: &mut dyn Source,
  path: &str,
  sheet: &mut Sheet,
) -> Result<(), SpreadsheetError> {
  let relationships = match source.relationships(path) {
    Ok(relationships) => relationships,
    Err(PackageError::Malformed(_)) => return Ok(()),
    Err(limit) => return Err(limit.into()),
  };
  for relationship in relationships.iter().filter(|r| !r.external) {
    // The last segment of the type names the kind, in the transitional and the strict spelling.
    match relationship.kind.rsplit('/').next() {
      Some("pivotTable") => sheet.pivot_tables += 1,
      Some("comments") => {
        let counted = related_part(
          source,
          &relationship.target,
          &mut sheet.warnings,
          count_comments,
        )?;
        sheet.comments += counted.unwrap_or(0);
      }
      Some("drawing") => {
        let counted = related_part(
          source,
          &relationship.target,
          &mut sheet.warnings,
          count_drawing,
        )?;
        let (pictures, charts) = counted.unwrap_or((0, 0));
        sheet.pictures += pictures;
        sheet.charts += charts;
      }
      _ => {}
    }
  }
  Ok(())
}

/// One `<c>` as read, before its value is resolved.
#[derive(Default)]
struct RawCell {
  kind: Option<String>,
  style: usize,
  value: Option<String>,
  inline: Option<String>,
  formula: Option<String>,
  shared_index: Option<u32>,
  shared_ref: Option<String>,
}

/// What every sheet of a workbook shares: the string table, the styles and the date system.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WorkbookContext<'a> {
  pub shared: &'a [String],
  pub styles: &'a Styles,
  pub date1904: bool,
}

/// One worksheet part to a `Sheet`. Malformed references and ranges become warnings, never panics.
pub(crate) fn parse_sheet(
  bytes: &[u8],
  name: &str,
  index: u32,
  hidden: bool,
  context: WorkbookContext<'_>,
  print_area: Option<CellRange>,
) -> Result<Sheet, SpreadsheetError> {
  let doc = parse_xml(bytes)?;
  let root = doc.root_element();
  let mut sheet = Sheet {
    name: name.to_string(),
    index,
    hidden,
    dimension: None,
    print_area,
    cols: Vec::new(),
    rows: BTreeMap::new(),
    default_row_height: 15.0,
    default_col_width: None,
    cells: BTreeMap::new(),
    merges: Vec::new(),
    drawings: 0,
    links: 0,
    tables: 0,
    comments: 0,
    pictures: 0,
    charts: 0,
    pivot_tables: 0,
    warnings: Vec::new(),
  };
  let mut shared_masters: HashMap<u32, (u32, u32)> = HashMap::new();

  for n in root.children().filter(|n| n.is_element()) {
    match n.tag_name().name() {
      "dimension" => sheet.dimension = attr(n, "ref").and_then(CellRange::parse),
      "sheetFormatPr" => {
        sheet.default_row_height =
          length_attr(attr(n, "defaultRowHeight"), MAX_ROW_HEIGHT).unwrap_or(15.0);
        sheet.default_col_width = length_attr(attr(n, "defaultColWidth"), MAX_COL_WIDTH);
      }
      "cols" => {
        for c in n
          .children()
          .filter(|c| c.is_element() && c.tag_name().name() == "col")
        {
          if let (Some(min), Some(max)) = (
            attr(c, "min").and_then(|v| v.parse().ok()),
            attr(c, "max").and_then(|v| v.parse().ok()),
          ) {
            sheet.cols.push(ColSpec {
              min,
              max,
              width: length_attr(attr(c, "width"), MAX_COL_WIDTH).unwrap_or(0.0),
              hidden: attr(c, "hidden").is_some_and(|v| is_on(Some(v))),
            });
          }
        }
      }
      "sheetData" => parse_rows(n, &mut sheet, context, &mut shared_masters),
      "mergeCells" => {
        for m in n
          .children()
          .filter(|m| m.is_element() && m.tag_name().name() == "mergeCell")
        {
          match attr(m, "ref").and_then(CellRange::parse) {
            Some(range) => sheet.merges.push(range),
            None => sheet.warnings.push(format!(
              "mergeCell ref {:?} is not a range; skipped",
              attr(m, "ref").unwrap_or("")
            )),
          }
        }
      }
      "drawing" => sheet.drawings += 1,
      "hyperlinks" => sheet.links += named_children(n, "hyperlink"),
      "tableParts" => sheet.tables += named_children(n, "tablePart"),
      _ => {}
    }
  }
  for range in &sheet.merges {
    if let Some(cell) = sheet.cells.get_mut(&(range.r0, range.c0)) {
      cell.merge = Some(*range);
    }
  }
  let unsupported = sheet.cells.values().filter(|c| !c.format_supported).count();
  if unsupported > 0 {
    sheet.warnings.push(format!(
      "{unsupported} cell(s) use number formats this release does not render"
    ));
  }
  let unknown_ids: std::collections::BTreeSet<u32> =
    sheet.cells.values().filter_map(|c| c.builtin_id).collect();
  if !unknown_ids.is_empty() {
    let ids: Vec<String> = unknown_ids.iter().map(u32::to_string).collect();
    sheet.warnings.push(format!(
      "built-in number format id(s) {} have no spelling this release knows; those cells keep their stored text",
      ids.join(", ")
    ));
  }
  Ok(sheet)
}

fn parse_rows(
  sheet_data: Node<'_, '_>,
  sheet: &mut Sheet,
  context: WorkbookContext<'_>,
  shared_masters: &mut HashMap<u32, (u32, u32)>,
) {
  // The 0-based row a `<row>` without a number falls on: the one after the row before it.
  let mut following: u32 = 0;
  for row in sheet_data
    .children()
    .filter(|r| r.is_element() && r.tag_name().name() == "row")
  {
    let row_index = match attr(row, "r").and_then(|v| v.parse::<u32>().ok()) {
      Some(number) => number.max(1) - 1,
      None => following,
    };
    following = row_index.saturating_add(1);
    if row_index >= MAX_ROWS {
      sheet
        .warnings
        .push(format!("row {following} is beyond the grid; skipped"));
      continue;
    }
    let spec = RowSpec {
      height: length_attr(attr(row, "ht"), MAX_ROW_HEIGHT),
      hidden: attr(row, "hidden").is_some_and(|v| is_on(Some(v))),
    };
    if spec != RowSpec::default() {
      sheet.rows.insert(row_index, spec);
    }
    let mut next_col: u32 = 0;
    for c in row
      .children()
      .filter(|c| c.is_element() && c.tag_name().name() == "c")
    {
      let position = match attr(c, "r") {
        Some(text) => match parse_cell_ref(text) {
          Some(pos) => pos,
          None => {
            sheet
              .warnings
              .push(format!("cell reference {text:?} is malformed; skipped"));
            continue;
          }
        },
        None => (row_index, next_col),
      };
      if position.1 >= MAX_COLS {
        sheet
          .warnings
          .push("a cell without a reference falls beyond the grid; skipped".to_string());
        continue;
      }
      next_col = position.1.saturating_add(1);
      let mut raw = RawCell {
        kind: attr(c, "t").map(str::to_string),
        style: attr(c, "s").and_then(|v| v.parse().ok()).unwrap_or(0),
        ..RawCell::default()
      };
      for child in c.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
          "v" => raw.value = Some(child.text().unwrap_or("").to_string()),
          "is" => raw.inline = Some(rich_text(child)),
          "f" => {
            let text = child
              .text()
              .map(str::to_string)
              .filter(|t| !t.trim().is_empty());
            raw.formula = text;
            if attr(child, "t") == Some("shared") {
              raw.shared_index = attr(child, "si").and_then(|v| v.parse().ok());
              raw.shared_ref = attr(child, "ref").map(str::to_string);
            }
          }
          _ => {}
        }
      }
      if let Some(cell) = resolve_cell(raw, position, context, shared_masters, &mut sheet.warnings)
      {
        sheet.cells.insert(position, cell);
      }
    }
  }
}

/// Built-in format ids whose codes are dates and times in the East Asian locales (ECMA-376
/// §18.8.30). This release has no spelling for them, but a number under one is still a date.
fn is_east_asian_date_id(id: u32) -> bool {
  matches!(id, 27..=36 | 50..=58)
}

/// A serial under a clock-only code. The code hides the day, but a serial that holds one names
/// a moment rather than a time of day, and the value keeps it.
fn clock_value(serial: f64, date1904: bool) -> CellValue {
  if serial >= 1.0
    && let Some(iso) = iso_date(serial, date1904, true)
  {
    CellValue::Date(iso)
  } else {
    CellValue::Time(iso_time(serial))
  }
}

/// Give each cell that follows a shared formula its own formula text: the master's, with its
/// relative references moved by the cell's offset from the master. A follower stays without
/// text when its master holds none, when the master is longer than a formula may be, or once
/// `budget` bytes of text have been written out across the workbook.
fn expand_shared_formulas(sheet: &mut Sheet, budget: &mut usize) {
  let mut followers: BTreeMap<(u32, u32), Vec<(u32, u32)>> = BTreeMap::new();
  for (&position, cell) in &sheet.cells {
    if let (Some(master), None) = (cell.shared_master, &cell.formula) {
      followers.entry(master).or_default().push(position);
    }
  }
  let mut left_out = 0usize;
  for (master, positions) in followers {
    let text = sheet
      .cells
      .get(&master)
      .and_then(|cell| cell.formula.clone())
      .filter(|text| text.chars().count() <= MAX_FORMULA_CHARS);
    let Some(text) = text else {
      left_out += positions.len();
      continue;
    };
    for position in positions {
      let moved = shift_formula(
        &text,
        i64::from(position.0) - i64::from(master.0),
        i64::from(position.1) - i64::from(master.1),
      );
      match (
        budget.checked_sub(moved.len()),
        sheet.cells.get_mut(&position),
      ) {
        (Some(remaining), Some(cell)) => {
          *budget = remaining;
          cell.formula = Some(moved);
        }
        _ => left_out += 1,
      }
    }
  }
  if left_out > 0 {
    sheet.warnings.push(format!(
      "{left_out} cell(s) follow a shared formula that was not written out for them; they name their master only"
    ));
  }
}

fn resolve_cell(
  raw: RawCell,
  position: (u32, u32),
  context: WorkbookContext<'_>,
  shared_masters: &mut HashMap<u32, (u32, u32)>,
  warnings: &mut Vec<String>,
) -> Option<Cell> {
  let WorkbookContext {
    shared,
    styles,
    date1904,
  } = context;
  let reference = cell_ref(position.0, position.1);
  let formula = raw.formula.clone();
  let mut shared_master = None;
  if let Some(si) = raw.shared_index {
    if raw.shared_ref.is_some() && formula.is_some() {
      shared_masters.insert(si, position);
    } else if let Some(master) = shared_masters.get(&si) {
      // A follower usually carries no text; one that does keeps it as its own formula.
      shared_master = Some(*master);
    } else {
      warnings.push(format!(
        "{reference}: shared formula {si} has no master; skipped"
      ));
    }
  }
  let cached = raw.value.is_some();
  let has_formula = formula.is_some() || shared_master.is_some();
  let stored = raw.value.clone().unwrap_or_default();
  let (value, text) = match raw.kind.as_deref() {
    Some("s") => {
      let Some(text) = stored
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|i| shared.get(i))
      else {
        warnings.push(format!(
          "{reference}: shared string index {stored:?} is out of range; skipped"
        ));
        return None;
      };
      (CellValue::Text(text.clone()), text.clone())
    }
    Some("str") => (CellValue::Text(stored.clone()), stored.clone()),
    Some("inlineStr") => {
      let text = raw.inline.clone().unwrap_or_default();
      (CellValue::Text(text.clone()), text)
    }
    Some("b") => {
      let on = matches!(stored.trim(), "1" | "true" | "TRUE");
      (
        CellValue::Boolean(on),
        if on { "TRUE" } else { "FALSE" }.to_string(),
      )
    }
    Some("e") => (CellValue::Error(stored.clone()), stored.clone()),
    _ => match raw.value.as_deref().map(str::trim) {
      Some(number) if !number.is_empty() => match number.parse::<f64>() {
        Ok(n) if n.is_finite() => (CellValue::Number(n), number.to_string()),
        _ => (CellValue::Text(number.to_string()), number.to_string()),
      },
      Some(_) => (CellValue::Text(String::new()), String::new()),
      None if has_formula => (CellValue::Blank, String::new()),
      None => return None,
    },
  };
  let raw_text = match &value {
    CellValue::Text(_) | CellValue::Error(_) | CellValue::Boolean(_) => text.clone(),
    _ => stored,
  };
  let stored = Stored {
    value,
    raw: raw_text,
    text,
  };
  Some(Cell {
    formula,
    shared_master,
    cached,
    ..Cell::styled(stored, raw.style, styles, date1904, &reference, warnings)
  })
}

/// A value as a file stores it, before the number format of the cell's style is applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
  /// Text, a number, a boolean, an error, or blank.
  pub value: CellValue,
  /// The value as the file spells it.
  pub raw: String,
  /// What the cell shows when no number format changes it.
  pub text: String,
}

impl Stored {
  /// A number, spelled the shortest way that reads back as the same number.
  pub fn number(n: f64) -> Stored {
    Stored {
      value: CellValue::Number(n),
      raw: n.to_string(),
      text: n.to_string(),
    }
  }

  pub fn text(text: String) -> Stored {
    Stored {
      value: CellValue::Text(text.clone()),
      raw: text.clone(),
      text,
    }
  }

  pub fn boolean(on: bool) -> Stored {
    let text = if on { "TRUE" } else { "FALSE" }.to_string();
    Stored {
      value: CellValue::Boolean(on),
      raw: text.clone(),
      text,
    }
  }

  /// An error, by the code a sheet shows for it: `#DIV/0!`.
  pub fn error(code: String) -> Stored {
    Stored {
      value: CellValue::Error(code.clone()),
      raw: code.clone(),
      text: code,
    }
  }

  /// No value: what a formula that has not been calculated leaves.
  pub fn blank() -> Stored {
    Stored {
      value: CellValue::Blank,
      raw: String::new(),
      text: String::new(),
    }
  }
}

impl Cell {
  /// The cell a stored value makes under the style numbered `style`. A number is typed (a
  /// date, a time, or a number) and shown through the number format the style selects; any
  /// other value is kept as it is. `reference` names the cell in a warning. The cell is given
  /// without a formula or a merged range, which are the caller's to set.
  pub fn styled(
    stored: Stored,
    style: usize,
    styles: &Styles,
    date1904: bool,
    reference: &str,
    warnings: &mut Vec<String>,
  ) -> Cell {
    let Stored {
      value,
      raw: raw_text,
      text,
    } = stored;
    // Cell type comes before number format: only a number is rendered through its code.
    let format = format_code(styles, style);
    let builtin_id = if format.is_none() {
      unknown_builtin_id(styles, style)
    } else {
      None
    };
    let mut format_supported = true;
    let mut currency_symbol = None;
    // What the value is and what the cell shows are settled separately. The kind of the format
    // code types the value, whether or not this release can render the code; a code it cannot
    // render leaves `text` as the stored spelling and the cell marked unsupported.
    let (value, text) = match (value, &format) {
      (CellValue::Number(n), Some(code)) => {
        let parsed = NumberFormat::parse(code);
        let kind = parsed
          .as_ref()
          .map_or_else(|| serial_kind(code), NumberFormat::serial_kind);
        let rendered = parsed.as_ref().and_then(|p| p.render(n, date1904));
        format_supported = parsed.is_some();
        match kind {
          SerialKind::Number | SerialKind::Duration => {
            currency_symbol = parsed.as_ref().and_then(NumberFormat::currency_symbol);
            (CellValue::Number(n), rendered.unwrap_or(text))
          }
          SerialKind::Clock => (clock_value(n, date1904), rendered.unwrap_or(text)),
          SerialKind::Date { with_time } => match iso_date(n, date1904, with_time) {
            // A code that renders must render this serial too.
            Some(iso) if rendered.is_some() || parsed.is_none() => {
              (CellValue::Date(iso), rendered.unwrap_or(text))
            }
            _ => {
              warnings.push(if n == 60.0 && !date1904 {
                format!(
                  "{reference}: serial 60 is the 1900 leap day Excel never had; kept as a number"
                )
              } else {
                format!(
                  "{reference}: serial {raw_text} is not a date under {code:?}; kept as a number"
                )
              });
              (CellValue::Number(n), text)
            }
          },
        }
      }
      (CellValue::Number(n), None) => match builtin_id {
        Some(id) => {
          format_supported = false;
          if !is_east_asian_date_id(id) {
            (CellValue::Number(n), text)
          } else if n < 1.0 {
            (CellValue::Time(iso_time(n)), text)
          } else {
            // Which of these ids shows a clock depends on the locale, so the serial decides.
            match iso_date(n, date1904, n.fract() != 0.0) {
              Some(iso) => (CellValue::Date(iso), text),
              None => {
                warnings.push(format!(
                  "{reference}: serial {raw_text} is not a date under built-in format {id}; kept as a number"
                ));
                (CellValue::Number(n), text)
              }
            }
          }
        }
        None => (CellValue::Number(n), render_general(n)),
      },
      (value, _) => (value, text),
    };
    Cell {
      format_supported,
      currency_symbol,
      value,
      raw: raw_text,
      text,
      format,
      formula: None,
      shared_master: None,
      cached: false,
      style,
      merge: None,
      builtin_id,
    }
  }
}

#[cfg(test)]
mod tests;
