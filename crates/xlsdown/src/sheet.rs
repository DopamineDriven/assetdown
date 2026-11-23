//! One sheet's records: its extent, its rows and columns, its cells, and its merged ranges.

use std::collections::{BTreeMap, HashMap};

use xlsxdown::{
  Cell, CellRange, ColSpec, MAX_CELLS, MAX_FORMULA_CHARS, RowSpec, Sheet, SpreadsheetError, Stored,
  Styles, cell_ref,
};

use crate::formula::{self, Links};
use crate::globals::Bound;
use crate::record::{BOF, EOF, Record, Records};
use crate::text::Parts;

/// What a run of records is, as its opening record states: a sheet of cells, a sheet of the
/// macros of Excel 4 (which is one of cells too), or a chart.
const WORKSHEET: u16 = 0x0010;
const MACRO_SHEET: u16 = 0x0040;
const CHART: u16 = 0x0020;
/// The height of a row a sheet says nothing about, in points.
const DEFAULT_ROW_HEIGHT: f64 = 12.75;
const MAX_ROW_HEIGHT: f64 = 409.5;
/// The characters a column holds when a sheet says nothing, and the room a column has around
/// its characters, as a share of one.
const DEFAULT_COLUMN_CHARACTERS: u16 = 8;
const COLUMN_PADDING: f64 = 8.0 / 7.0;

/// What every sheet of a workbook shares: the string table, the styles, the date system,
/// and what a formula's references are resolved against.
#[derive(Clone, Copy)]
pub(crate) struct Context<'a> {
  pub strings: &'a [String],
  pub styles: &'a Styles,
  pub date1904: bool,
  pub links: &'a Links,
}

/// What a cell's formula is.
enum Formula {
  /// The cell holds a value and no formula.
  None,
  /// The cell has a formula of its own: its text, or nothing where it could not be
  /// written out.
  Own(Option<String>),
  /// The cell takes its formula from the one kept for the cell at this row and column,
  /// which may be the cell itself.
  Kept((u32, u32)),
}

/// A formula kept once for the cells that use it.
struct Kept {
  tokens: Vec<u8>,
  /// What follows the tokens in their record: the lists of constants the formula uses.
  extra: Vec<u8>,
  /// Whether the formula's references are steps from the cell that uses it. They are for a
  /// formula cells share; a formula that fills a range together is the same in each cell.
  steps: bool,
}

/// A formula that gave text, whose text is in a record still to come.
struct Awaited {
  row: u16,
  col: u16,
  xf: u16,
  formula: Formula,
}

/// A number kept the short way: a whole number or the top of a double, either one a hundred
/// times what the cell holds when the lowest bit is set.
fn short_number(kept: u32) -> f64 {
  let value = if kept & 0x02 != 0 {
    f64::from(i32::from_le_bytes(kept.to_le_bytes()) >> 2)
  } else {
    f64::from_bits(u64::from(kept & 0xFFFF_FFFC) << 32)
  };
  if kept & 0x01 != 0 {
    value / 100.0
  } else {
    value
  }
}

/// An error by the code a sheet shows for it.
fn error_text(code: u8) -> &'static str {
  match code {
    0x00 => "#NULL!",
    0x07 => "#DIV/0!",
    0x0F => "#VALUE!",
    0x17 => "#REF!",
    0x1D => "#NAME?",
    0x24 => "#NUM!",
    0x2A => "#N/A",
    _ => "#N/A",
  }
}

struct Reader<'a, 'b> {
  sheet: Sheet,
  context: Context<'a>,
  cells_left: &'b mut usize,
  awaited: Option<Awaited>,
  /// The formulas kept for more than one cell, by the cell each is kept after.
  kept: HashMap<(u32, u32), Kept>,
  /// The cells that take their formula from one of those, each with the cell it is kept for.
  taking: Vec<((u32, u32), (u32, u32))>,
  /// The cell of the formula read last, which a kept formula that follows is kept for.
  last_formula: Option<(u32, u32)>,
  /// How many formulas could not be written out.
  unwritten: usize,
  /// The characters a default column holds, and its width when the sheet states it outright.
  column_characters: Option<u16>,
  column_width: Option<u16>,
}

impl Reader<'_, '_> {
  fn place(
    &mut self,
    (row, col, xf): (u16, u16, u16),
    stored: Stored,
    formula: Formula,
    cached: bool,
  ) -> Result<(), SpreadsheetError> {
    let position = (u32::from(row), u32::from(col));
    let styled = Cell::styled(
      stored,
      usize::from(xf),
      self.context.styles,
      self.context.date1904,
      &cell_ref(position.0, position.1),
      &mut self.sheet.warnings,
    );
    let (text, shared_master) = match formula {
      Formula::None => (None, None),
      // A formula that could not be written out is there all the same, with its text empty.
      Formula::Own(text) => {
        self.unwritten += usize::from(text.is_none());
        (Some(text.unwrap_or_default()), None)
      }
      // The text is written once the whole sheet is read, when the formula kept for the
      // cell is known.
      Formula::Kept(holder) => (None, (holder != position).then_some(holder)),
    };
    let cell = Cell {
      formula: text,
      shared_master,
      cached,
      ..styled
    };
    if self.sheet.cells.insert(position, cell).is_none() {
      *self.cells_left = self.cells_left.checked_sub(1).ok_or_else(|| {
        SpreadsheetError::LimitExceeded(format!("the workbook has more than {MAX_CELLS} cells"))
      })?;
    }
    Ok(())
  }

  /// A formula that gave text and was not followed by it is a cell with no value.
  fn settle(&mut self) -> Result<(), SpreadsheetError> {
    match self.awaited.take() {
      Some(Awaited {
        row,
        col,
        xf,
        formula,
      }) => self.place((row, col, xf), Stored::blank(), formula, false),
      None => Ok(()),
    }
  }

  /// A cell's record: its row, its column, its format, and after them its value.
  fn cell(&mut self, record: Record<'_>) -> Result<(), SpreadsheetError> {
    let (Some(row), Some(col), Some(xf)) = (record.u16_at(0), record.u16_at(2), record.u16_at(4))
    else {
      return Ok(());
    };
    let at = (row, col, xf);
    let value = record.data.get(6..).unwrap_or(&[]);
    match record.id {
      0x0203 => {
        if let Some(Ok(bytes)) = value.get(..8).map(<[u8; 8]>::try_from) {
          self.place(
            at,
            Stored::number(f64::from_le_bytes(bytes)),
            Formula::None,
            true,
          )?;
        }
      }
      0x027E => {
        if let Some(kept) = record.u32_at(6) {
          self.place(at, Stored::number(short_number(kept)), Formula::None, true)?;
        }
      }
      0x00FD => {
        let Some(index) = record.u32_at(6) else {
          return Ok(());
        };
        let shared = usize::try_from(index)
          .ok()
          .and_then(|index| self.context.strings.get(index));
        match shared {
          Some(text) => self.place(at, Stored::text(text.clone()), Formula::None, true)?,
          None => self.sheet.warnings.push(format!(
            "{}: shared string index {index} is out of range; skipped",
            cell_ref(u32::from(row), u32::from(col))
          )),
        }
      }
      // Text kept in the cell itself, with or without formatting after it.
      0x0204 | 0x00D6 => {
        if let Some(text) = Parts::one(value).string() {
          self.place(at, Stored::text(text), Formula::None, true)?;
        }
      }
      0x0205 => {
        if let [kept, is_error, ..] = value {
          let stored = match is_error {
            0 => Stored::boolean(*kept != 0),
            _ => Stored::error(error_text(*kept).to_string()),
          };
          self.place(at, stored, Formula::None, true)?;
        }
      }
      0x0006 => self.formula(at, value)?,
      _ => {}
    }
    Ok(())
  }

  /// A formula: the result it was left with, some flags, and its tokens with their length.
  fn formula(&mut self, at: (u16, u16, u16), value: &[u8]) -> Result<(), SpreadsheetError> {
    let Some(Ok(result)) = value.get(..8).map(<[u8; 8]>::try_from) else {
      return Ok(());
    };
    let position = (u32::from(at.0), u32::from(at.1));
    // The tokens follow some flags and their own length, and what follows the tokens is
    // the lists of constants they use.
    let length = value.get(14..16).map_or(0, |length| {
      usize::from(u16::from_le_bytes([length[0], length[1]]))
    });
    let tokens = value.get(16..16 + length).unwrap_or(&[]);
    let extra = value.get(16 + length..).unwrap_or(&[]);
    let formula = match tokens {
      // A cell that takes its formula from one kept for several says only which.
      [0x01, r0, r1, c0, c1, ..] => {
        let holder = (
          u32::from(u16::from_le_bytes([*r0, *r1])),
          u32::from(u16::from_le_bytes([*c0, *c1])),
        );
        self.taking.push((position, holder));
        Formula::Kept(holder)
      }
      _ => Formula::Own(formula::text(tokens, extra, position, self.context.links)),
    };
    self.last_formula = Some(position);
    let stored = match result {
      // Text, which the next record holds.
      [0, _, _, _, _, _, 0xFF, 0xFF] => {
        self.awaited = Some(Awaited {
          row: at.0,
          col: at.1,
          xf: at.2,
          formula,
        });
        return Ok(());
      }
      [1, _, on, _, _, _, 0xFF, 0xFF] => Stored::boolean(on != 0),
      [2, _, code, _, _, _, 0xFF, 0xFF] => Stored::error(error_text(code).to_string()),
      [_, _, _, _, _, _, 0xFF, 0xFF] => Stored::text(String::new()),
      number => Stored::number(f64::from_le_bytes(number)),
    };
    self.place(at, stored, formula, true)
  }

  /// A row of numbers kept the short way: the row, the first column, a format and a number
  /// for each column, and the last column.
  fn short_numbers(&mut self, record: Record<'_>) -> Result<(), SpreadsheetError> {
    let (Some(row), Some(first)) = (record.u16_at(0), record.u16_at(2)) else {
      return Ok(());
    };
    let each = record.data.get(4..).unwrap_or(&[]).as_chunks::<6>().0;
    for (offset, kept) in each.iter().enumerate() {
      let Some(col) = u16::try_from(offset)
        .ok()
        .and_then(|offset| first.checked_add(offset))
      else {
        break;
      };
      let xf = u16::from_le_bytes([kept[0], kept[1]]);
      let number = short_number(u32::from_le_bytes([kept[2], kept[3], kept[4], kept[5]]));
      self.place((row, col, xf), Stored::number(number), Formula::None, true)?;
    }
    Ok(())
  }

  /// A record that is not a cell's.
  fn other(&mut self, record: Record<'_>) {
    let sheet = &mut self.sheet;
    match record.id {
      // The first row and the one past the last, and the same for the columns.
      0x0200 => {
        if let (Some(r0), Some(r1), Some(c0), Some(c1)) = (
          record.u32_at(0),
          record.u32_at(4),
          record.u16_at(8),
          record.u16_at(10),
        ) && r1 > r0
          && c1 > c0
        {
          sheet.dimension = Some(CellRange {
            r0,
            c0: u32::from(c0),
            r1: r1 - 1,
            c1: u32::from(c1) - 1,
          });
        }
      }
      0x0225 => {
        if let Some(twips) = record.u16_at(2) {
          sheet.default_row_height = (f64::from(twips) / 20.0).min(MAX_ROW_HEIGHT);
        }
      }
      0x0055 => self.column_characters = record.u16_at(0),
      0x0099 => self.column_width = record.u16_at(0),
      0x007D => {
        if let (Some(first), Some(last), Some(width), Some(flags)) = (
          record.u16_at(0),
          record.u16_at(2),
          record.u16_at(4),
          record.u16_at(8),
        ) {
          sheet.cols.push(ColSpec {
            min: u32::from(first) + 1,
            max: u32::from(last) + 1,
            width: f64::from(width) / 256.0,
            hidden: flags & 0x0001 != 0,
          });
        }
      }
      0x0208 => {
        if let (Some(row), Some(height), Some(flags)) =
          (record.u16_at(0), record.u16_at(6), record.u16_at(12))
        {
          sheet.rows.insert(
            u32::from(row),
            RowSpec {
              // The top bit marks a row left at the sheet's default height.
              height: (height & 0x8000 == 0)
                .then(|| (f64::from(height) / 20.0).min(MAX_ROW_HEIGHT)),
              hidden: flags & 0x0020 != 0,
            },
          );
        }
      }
      // A count, and for each range its first and last row and its first and last column.
      0x00E5 => {
        let ranges = record.data.get(2..).unwrap_or(&[]).as_chunks::<8>().0;
        let count = usize::from(record.u16_at(0).unwrap_or(0));
        sheet.merges.extend(ranges.iter().take(count).map(|range| {
          let at = |at: usize| u32::from(u16::from_le_bytes([range[at], range[at + 1]]));
          CellRange {
            r0: at(0),
            r1: at(2),
            c0: at(4),
            c1: at(6),
          }
        }));
      }
      0x01B8 => sheet.links += 1,
      0x001C => sheet.comments += 1,
      0x00B0 => sheet.pivot_tables += 1,
      // A table, as Excel 2003 keeps one and as a later Excel keeps one in this format.
      0x0872 | 0x0878 => sheet.tables += 1,
      0x00EC => sheet.drawings = 1,
      // An object: what it is comes first, after the number and the length of that statement.
      0x005D if record.u16_at(0) == Some(0x0015) => match record.u16_at(4) {
        Some(0x0008) => sheet.pictures += 1,
        Some(0x0005) => sheet.charts += 1,
        _ => {}
      },
      _ => {}
    }
  }
}

/// The sheet the workbook lists as `bound`, read from where its records start. `cells_left`
/// is how many cells the workbook may still hold, and `text_left` how many bytes of formula
/// text it may still write out for the cells that share a formula; both are counted down.
pub(crate) fn read(
  stream: &[u8],
  bound: &Bound,
  index: u32,
  context: Context<'_>,
  print_area: Option<CellRange>,
  cells_left: &mut usize,
  text_left: &mut usize,
) -> Result<Sheet, SpreadsheetError> {
  let mut reader = Reader {
    sheet: Sheet {
      name: bound.name.clone(),
      index,
      hidden: bound.hidden,
      dimension: None,
      print_area,
      cols: Vec::new(),
      rows: BTreeMap::new(),
      default_row_height: DEFAULT_ROW_HEIGHT,
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
    },
    context,
    cells_left,
    awaited: None,
    kept: HashMap::new(),
    taking: Vec::new(),
    last_formula: None,
    unwritten: 0,
    column_characters: None,
    column_width: None,
  };
  let mut records = Records::from(stream, bound.at);
  let kind = records
    .next()
    .filter(|record| record.id == BOF)
    .and_then(|record| record.u16_at(2));
  match kind {
    Some(WORKSHEET | MACRO_SHEET) => {}
    // A sheet that is only a chart has no cells: the numbers in it are the chart's.
    Some(CHART) => {
      reader.sheet.charts = 1;
      return Ok(reader.sheet);
    }
    Some(_) => return Ok(reader.sheet),
    None => {
      reader
        .sheet
        .warnings
        .push("the sheet's records are not where the workbook says they are".to_string());
      return Ok(reader.sheet);
    }
  }
  // A chart on the sheet is a run of records inside the sheet's, with its own opening and
  // end. Its records are the chart's, the numbers it plots among them.
  let mut inside = 0usize;
  let mut ended = false;
  while let Some(record) = records.next() {
    match record.id {
      BOF => inside += 1,
      EOF if inside == 0 => {
        ended = true;
        break;
      }
      EOF => inside -= 1,
      _ if inside > 0 => {}
      // The text a formula gave. What defines a shared formula may stand before it.
      0x0207 => {
        if let Some(Awaited {
          row,
          col,
          xf,
          formula,
        }) = reader.awaited.take()
        {
          let text = Parts::of(record.data, records.continued()).string();
          match text {
            Some(text) => reader.place((row, col, xf), Stored::text(text), formula, true)?,
            None => reader.place((row, col, xf), Stored::blank(), formula, false)?,
          }
        }
      }
      // A formula kept for several cells, after the first of them: one they share, with
      // its references as steps from each, or one that fills a range together. Both say
      // which rows and columns they cover, and then give their tokens with their length.
      0x04BC | 0x0221 => {
        let at = if record.id == 0x04BC { 8 } else { 12 };
        let length = usize::from(record.u16_at(at).unwrap_or(0));
        let tokens = record.data.get(at + 2..at + 2 + length);
        if let (Some(holder), Some(tokens)) = (reader.last_formula, tokens) {
          reader.kept.insert(
            holder,
            Kept {
              tokens: tokens.to_vec(),
              extra: record.data[at + 2 + length..].to_vec(),
              steps: record.id == 0x04BC,
            },
          );
        }
      }
      // A table of what-ifs, which is not written out.
      0x0236 => {}
      0x0203 | 0x027E | 0x00FD | 0x0204 | 0x00D6 | 0x0205 | 0x0006 => {
        reader.settle()?;
        reader.cell(record)?;
      }
      0x00BD => {
        reader.settle()?;
        reader.short_numbers(record)?;
      }
      _ => reader.other(record),
    }
  }
  reader.settle()?;
  let Reader {
    mut sheet,
    column_characters,
    column_width,
    kept,
    taking,
    mut unwritten,
    ..
  } = reader;
  if !ended {
    sheet
      .warnings
      .push("the sheet's records stop before its end".to_string());
  }
  // Each cell that takes its formula from one kept for several has it written for itself.
  // The cell the formula is kept for always has its text. The cells that follow it have
  // theirs while the workbook's budget for such text lasts, and name that cell either way.
  let mut left_out = 0usize;
  for (position, holder) in taking {
    let written = kept.get(&holder).and_then(|kept| {
      let base = if kept.steps { position } else { holder };
      formula::text(&kept.tokens, &kept.extra, base, context.links)
    });
    let Some(cell) = sheet.cells.get_mut(&position) else {
      continue;
    };
    match written {
      _ if cell.shared_master.is_none() => {
        unwritten += usize::from(written.is_none());
        cell.formula = Some(written.unwrap_or_default());
      }
      Some(text) if text.len() <= MAX_FORMULA_CHARS && text.len() <= *text_left => {
        *text_left -= text.len();
        cell.formula = Some(text);
      }
      _ => left_out += 1,
    }
  }
  if unwritten > 0 {
    sheet.warnings.push(format!(
      "{unwritten} formula(s) hold what this release does not write out; their text is empty"
    ));
  }
  if left_out > 0 {
    sheet.warnings.push(format!(
      "{left_out} cell(s) follow a shared formula that was not written out for them; they name their master only"
    ));
  }
  // A width stated outright is in 256ths of a character. A count of characters has the room
  // around them added, and is cut to a 256th, as a workbook of 2007 states the same width.
  sheet.default_col_width = column_width
    .map(|width| f64::from(width) / 256.0)
    .or_else(|| {
      column_characters
        .filter(|characters| *characters != DEFAULT_COLUMN_CHARACTERS)
        .map(|characters| ((f64::from(characters) + COLUMN_PADDING) * 256.0).floor() / 256.0)
    });
  for range in &sheet.merges {
    if let Some(cell) = sheet.cells.get_mut(&(range.r0, range.c0)) {
      cell.merge = Some(*range);
    }
  }
  let unsupported = sheet
    .cells
    .values()
    .filter(|cell| !cell.format_supported)
    .count();
  if unsupported > 0 {
    sheet.warnings.push(format!(
      "{unsupported} cell(s) use number formats this release does not render"
    ));
  }
  Ok(sheet)
}
