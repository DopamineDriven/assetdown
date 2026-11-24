//! A workbook's total, and the lines it adds up, found by following its formulas.
//!
//! A sheet's labels say what its maker called things; its formulas say what is added to
//! what. The total is the formula that no other formula reads and that reaches the most
//! cells. From it the formulas are followed down: one that only adds cells up is a subtotal,
//! and what it adds is looked at in turn; anything else (a number typed in, a quantity times
//! a price, a charge worked out from a subtotal) is a line. So the lines come to the total,
//! or the workbook says that they do not.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::formula::{Reference, references};
use crate::workbook::{Cell, CellValue, Sheet, Workbook};

/// A workbook whose formulas read more cells than this between them is not followed. No
/// real workbook comes near it; one made to exhaust a reader does.
const MAX_READS: usize = 4_000_000;
/// How many cells the following of a workbook's formulas may look at, once they are read.
/// Which formula reaches furthest is found by following each that nothing reads the whole
/// way down, which a file can make the square of its size. Real workbooks use a sliver of
/// this.
const MAX_STEPS: usize = 50_000_000;
/// How far up its column a line with no text in its row looks for a heading.
const HEADING_ROWS: u32 = 200;

/// A cell of a workbook: its sheet, by its place among the workbook's, its row, and its
/// column, each from nought.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellAt {
  pub sheet: usize,
  pub row: u32,
  pub col: u32,
}

/// One line a total adds up.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditLine {
  /// The cell that holds the line's amount.
  pub at: CellAt,
  /// What the line adds to the total: below nought for what the total takes away.
  pub amount: f64,
  /// The text of the line's row, or failing that the heading over its column.
  pub description: Option<String>,
  /// The cells that text was read from, in the order it reads.
  pub described_by: Vec<CellAt>,
  /// Where the amount is one cell times another and one of the two is shown as money: the
  /// cell that says how many, and the one that holds the price of each.
  pub quantity: Option<CellAt>,
  pub unit_price: Option<CellAt>,
}

/// A workbook's total, with the lines it adds up.
#[derive(Debug, Clone, PartialEq)]
pub struct Audit {
  pub total: CellAt,
  pub value: f64,
  /// In the order the total reads them. The total itself, when it is one cell times
  /// another. None when the total is worked out some other way than by adding cells up.
  pub lines: Vec<AuditLine>,
  /// Whether the lines come to the total, give or take half a cent to a line.
  pub adds_up: bool,
}

/// The looks a following has left. One is taken for each cell looked at.
struct Steps(usize);

/// The following of a workbook took more looks than it was allowed.
struct Spent;

impl Steps {
  fn take(&mut self, looks: usize) -> Result<(), Spent> {
    self.0 = self.0.checked_sub(looks).ok_or(Spent)?;
    Ok(())
  }
}

/// A formula, as the cells it reads.
struct Formula {
  /// The numbers it reads, in the order it reads them, each with whether it is added (1) or
  /// taken away (-1).
  reads: Vec<(CellAt, f64)>,
  /// The formula only adds cells up.
  sums: bool,
  /// The formula is one cell times another.
  product: Option<(CellAt, CellAt)>,
}

fn number(cell: &Cell) -> Option<f64> {
  match cell.value {
    CellValue::Number(value) => Some(value),
    _ => None,
  }
}

/// Whether a cell is shown as money: with a currency's sign, or with two places.
fn money(cell: &Cell) -> bool {
  cell.currency_symbol.is_some()
    || cell
      .format
      .as_deref()
      .is_some_and(|code| code.contains(".00") && !code.contains('%'))
}

/// Whether the reference that starts at `start` is taken away: a minus stands before it,
/// with at most spaces and opening brackets between.
fn taken_away(formula: &str, start: usize) -> bool {
  formula[..start]
    .chars()
    .rev()
    .find(|c| !c.is_whitespace() && *c != '(')
    == Some('-')
}

/// A formula with its references taken out, in upper case, and without its spaces: what it
/// does to the cells it reads.
fn workings(formula: &str, found: &[Reference]) -> String {
  let mut rest = String::new();
  let mut at = 0;
  for reference in found {
    rest.push_str(&formula[at..reference.start]);
    at = reference.end;
  }
  rest.push_str(&formula[at..]);
  rest
    .chars()
    .filter(|c| !c.is_whitespace())
    .flat_map(char::to_uppercase)
    .collect()
}

/// Whether a formula's workings only add cells up: sums, pluses, and minuses, with or
/// without a rounding of the whole, or the kind of subtotal that is a sum.
fn only_adds(workings: &str) -> bool {
  let rounded = workings.strip_prefix("ROUND(").and_then(|inner| {
    let (inner, places) = inner.strip_suffix(')')?.rsplit_once(',')?;
    places.chars().all(|c| c.is_ascii_digit()).then_some(inner)
  });
  rounded
    .unwrap_or(workings)
    .replace("SUBTOTAL(109,", "(")
    .replace("SUBTOTAL(9,", "(")
    .replace("SUM", "")
    .chars()
    .all(|c| "+-(),".contains(c))
}

/// The formulas of a workbook, by the cells they read.
struct Graph<'a> {
  workbook: &'a Workbook,
  formulas: HashMap<CellAt, Formula>,
  /// Every cell some formula reads.
  read: HashSet<CellAt>,
}

impl<'a> Graph<'a> {
  fn cell(&self, at: CellAt) -> Option<&'a Cell> {
    self
      .workbook
      .sheets
      .get(at.sheet)?
      .cells
      .get(&(at.row, at.col))
  }

  /// None when the formulas read more cells than any real workbook's do.
  fn of(workbook: &'a Workbook) -> Option<Graph<'a>> {
    // A sheet's name is the same whatever its case.
    let named: HashMap<String, usize> = workbook
      .sheets
      .iter()
      .enumerate()
      .map(|(index, sheet)| (sheet.name.to_lowercase(), index))
      .collect();
    // The cells that hold numbers, a column at a time: a formula mostly reads down one.
    let by_column: Vec<BTreeSet<(u32, u32)>> = workbook
      .sheets
      .iter()
      .map(|sheet| {
        sheet
          .cells
          .iter()
          .filter(|(_, cell)| number(cell).is_some())
          .map(|((row, col), _)| (*col, *row))
          .collect()
      })
      .collect();
    let numbers_in = |sheet: usize, reference: &Reference| -> Vec<CellAt> {
      let range = reference.range;
      let cells: &Sheet = &workbook.sheets[sheet];
      let mut found: Vec<(u32, u32)> = if range.r1 - range.r0 <= range.c1 - range.c0 {
        // Wider than it is tall: a row at a time.
        cells
          .cells
          .range((range.r0, range.c0)..=(range.r1, range.c1))
          .filter(|((_, col), cell)| (range.c0..=range.c1).contains(col) && number(cell).is_some())
          .map(|(at, _)| *at)
          .collect()
      } else {
        let columns = &by_column[sheet];
        let last = columns.last().map_or(0, |(col, _)| *col);
        (range.c0..=range.c1.min(last))
          .flat_map(|col| columns.range((col, range.r0)..=(col, range.r1)))
          .map(|(col, row)| (*row, *col))
          .collect()
      };
      found.sort_unstable();
      found
        .into_iter()
        .map(|(row, col)| CellAt { sheet, row, col })
        .collect()
    };

    let mut formulas = HashMap::new();
    let mut read = HashSet::new();
    let mut reads_in_all = 0usize;
    for (sheet, cells) in workbook.sheets.iter().enumerate() {
      for ((row, col), cell) in &cells.cells {
        let text = match cell.formula.as_deref() {
          Some(text) if !text.is_empty() && number(cell).is_some() => text,
          _ => continue,
        };
        let at = CellAt {
          sheet,
          row: *row,
          col: *col,
        };
        let found = references(text);
        let mut reads: Vec<(CellAt, f64)> = Vec::new();
        let mut seen = HashSet::new();
        // The cell each reference is when it is one cell with a number in it.
        let mut single: Vec<Option<CellAt>> = Vec::with_capacity(found.len());
        for reference in &found {
          let on = match &reference.sheet {
            None => Some(sheet),
            Some(name) => named.get(&name.to_lowercase()).copied(),
          };
          let numbers = on.map_or_else(Vec::new, |on| numbers_in(on, reference));
          let range = reference.range;
          single.push(match numbers[..] {
            [one] if (range.r0, range.c0) == (range.r1, range.c1) => Some(one),
            _ => None,
          });
          let sign = if taken_away(text, reference.start) {
            -1.0
          } else {
            1.0
          };
          for number in numbers {
            if number != at && seen.insert(number) {
              reads.push((number, sign));
            }
          }
        }
        reads_in_all += reads.len();
        if reads_in_all > MAX_READS {
          return None;
        }
        let workings = workings(text, &found);
        let product = match single[..] {
          [Some(first), Some(second)] if workings.replace(['(', ')'], "") == "*" => {
            Some((first, second))
          }
          _ => None,
        };
        read.extend(reads.iter().map(|(cell, _)| *cell));
        formulas.insert(
          at,
          Formula {
            sums: !reads.is_empty() && only_adds(&workings),
            reads,
            product,
          },
        );
      }
    }
    Some(Graph {
      workbook,
      formulas,
      read,
    })
  }

  /// How many cells a formula reaches: those it reads, those they read, and so on.
  fn reach(&self, top: CellAt, steps: &mut Steps) -> Result<usize, Spent> {
    let mut reached = HashSet::new();
    let mut open = vec![top];
    while let Some(cell) = open.pop() {
      let Some(formula) = self.formulas.get(&cell) else {
        continue;
      };
      steps.take(formula.reads.len())?;
      for (read, _) in &formula.reads {
        if reached.insert(*read) {
          open.push(*read);
        }
      }
    }
    Ok(reached.len())
  }

  /// The total: of the formulas nothing reads, on the sheets that are shown, the one that
  /// reaches the most cells. Of those that reach as many, one shown as money; of those, the
  /// lowest on its sheet.
  fn total(&self, steps: &mut Steps) -> Result<Option<CellAt>, Spent> {
    let mut best = None;
    for (at, formula) in &self.formulas {
      if formula.reads.is_empty() || self.read.contains(at) || self.workbook.sheets[at.sheet].hidden
      {
        continue;
      }
      let shown_as_money = self.cell(*at).is_some_and(money);
      let rank = (
        self.reach(*at, steps)?,
        shown_as_money,
        at.row,
        at.col,
        std::cmp::Reverse(at.sheet),
      );
      if best.is_none_or(|(held, _)| rank > held) {
        best = Some((rank, *at));
      }
    }
    Ok(best.map(|(_, at)| at))
  }

  /// The text that says what a line is, with the cells it was read from: the text of its
  /// row to the left of it, or failing that to the right, or failing that the heading over
  /// its column.
  fn description(&self, at: CellAt) -> Option<(String, Vec<CellAt>)> {
    let sheet = self.workbook.sheets.get(at.sheet)?;
    let text = |cell: &'a Cell| match &cell.value {
      CellValue::Text(text) => Some(text.trim()).filter(|text| !text.is_empty()),
      _ => None,
    };
    let in_row = |columns: std::ops::RangeInclusive<u32>| {
      let (found, cells): (Vec<&str>, Vec<CellAt>) = sheet
        .cells
        .range((at.row, *columns.start())..=(at.row, *columns.end()))
        .filter_map(|((row, col), cell)| {
          let from = CellAt {
            sheet: at.sheet,
            row: *row,
            col: *col,
          };
          Some((text(cell)?, from))
        })
        .unzip();
      (!found.is_empty()).then(|| (found.join(" "), cells))
    };
    let left = at.col.checked_sub(1).and_then(|last| in_row(0..=last));
    let right = || in_row(at.col + 1..=u32::MAX);
    let heading = || {
      (at.row.saturating_sub(HEADING_ROWS)..at.row)
        .rev()
        .find_map(|row| {
          let over = sheet.cells.get(&(row, at.col)).and_then(text)?;
          Some((over.to_string(), vec![CellAt { row, ..at }]))
        })
    };
    left.or_else(right).or_else(heading)
  }

  /// A cell as a line, when it holds a number that is not nought.
  fn line(&self, at: CellAt, sign: f64) -> Option<AuditLine> {
    let amount = sign * self.cell(at).and_then(number)?;
    if amount == 0.0 {
      return None;
    }
    // Of the two cells a line multiplies, the price is the one shown as money. With
    // neither shown so, or both, which is which is not known.
    let (quantity, unit_price) = self
      .formulas
      .get(&at)
      .and_then(|formula| formula.product)
      .and_then(|(first, second)| {
        let shown = |cell: CellAt| self.cell(cell).is_some_and(money);
        match (shown(first), shown(second)) {
          (false, true) => Some((first, second)),
          (true, false) => Some((second, first)),
          _ => None,
        }
      })
      .unzip();
    let (description, described_by) = self.description(at).unzip();
    Some(AuditLine {
      at,
      amount,
      description,
      described_by: described_by.unwrap_or_default(),
      quantity,
      unit_price,
    })
  }

  /// The lines under a cell: itself when it is a line, and when it only adds cells up, the
  /// lines under each of them. A cell is looked at once, so that formulas that read each
  /// other in a ring are gone round once.
  fn lines(
    &self,
    top: CellAt,
    sign: f64,
    seen: &mut HashSet<CellAt>,
    steps: &mut Steps,
  ) -> Result<Vec<AuditLine>, Spent> {
    let mut found = Vec::new();
    let mut open = vec![(top, sign)];
    while let Some((cell, sign)) = open.pop() {
      steps.take(1)?;
      if !seen.insert(cell) {
        continue;
      }
      match self.formulas.get(&cell) {
        // The last of what it reads goes in first, so that the first comes out first.
        Some(formula) if formula.sums => open.extend(
          formula
            .reads
            .iter()
            .rev()
            .map(|(read, its_sign)| (*read, sign * its_sign)),
        ),
        _ => found.extend(self.line(cell, sign)),
      }
    }
    Ok(found)
  }

  /// A line that multiplies a total (hours in all, times a rate) gives way to the rows that
  /// come to the same amount, when the sheet has them under a sum of their own: they say
  /// what the amount is made of.
  fn in_its_place(
    &self,
    line: &AuditLine,
    seen: &HashSet<CellAt>,
    steps: &mut Steps,
  ) -> Result<Option<Vec<AuditLine>>, Spent> {
    let Some((first, second)) = self
      .formulas
      .get(&line.at)
      .and_then(|formula| formula.product)
    else {
      return Ok(None);
    };
    let sums = |cell: CellAt| self.formulas.get(&cell).is_some_and(|formula| formula.sums);
    if !sums(first) && !sums(second) {
      return Ok(None);
    }
    // Every formula is looked at for one that comes to the same.
    steps.take(self.formulas.len())?;
    let mut others: Vec<CellAt> = self
      .formulas
      .iter()
      .filter(|(at, formula)| formula.sums && !seen.contains(at))
      .map(|(at, _)| *at)
      .filter(|at| {
        self
          .cell(*at)
          .and_then(number)
          .is_some_and(|value| comes_to(value, line.amount, 1))
      })
      .collect();
    others.sort_unstable();
    for other in others {
      // Each is followed from what has been seen so far, which is copied for it.
      steps.take(seen.len())?;
      let lines = self.lines(other, 1.0, &mut seen.clone(), steps)?;
      let sum: f64 = lines.iter().map(|line| line.amount).sum();
      if !lines.is_empty() && comes_to(sum, line.amount, lines.len()) {
        return Ok(Some(lines));
      }
    }
    Ok(None)
  }
}

/// Whether a sum of `count` amounts is `whole`, give or take half a cent to an amount.
fn comes_to(sum: f64, whole: f64, count: usize) -> bool {
  let count = u32::try_from(count.max(1)).unwrap_or(u32::MAX);
  (sum - whole).abs() <= 0.005 * f64::from(count) + 1e-9
}

/// The total of a workbook's formulas and its lines; `Ok(None)` when no formula is a total.
fn followed(graph: &Graph<'_>, steps: &mut Steps) -> Result<Option<Audit>, Spent> {
  let Some(total) = graph.total(steps)? else {
    return Ok(None);
  };
  let Some(value) = graph.cell(total).and_then(number) else {
    return Ok(None);
  };
  let mut seen = HashSet::new();
  // A total that adds cells up is followed down to its lines, and one that is one cell times
  // another (so many, at so much) is the one line itself. Any other is not followed: it is
  // all there is.
  let found = match graph.formulas.get(&total) {
    Some(formula) if formula.sums || formula.product.is_some() => {
      graph.lines(total, 1.0, &mut seen, steps)?
    }
    _ => Vec::new(),
  };
  let mut lines = Vec::with_capacity(found.len());
  for line in found {
    match graph.in_its_place(&line, &seen, steps)? {
      Some(made_of) => {
        seen.extend(made_of.iter().map(|line| line.at));
        lines.extend(made_of);
      }
      None => lines.push(line),
    }
  }
  let sum: f64 = lines.iter().map(|line| line.amount).sum();
  Ok(Some(Audit {
    total,
    value,
    adds_up: !lines.is_empty() && comes_to(sum, value, lines.len()),
    lines,
  }))
}

impl Workbook {
  /// The workbook's total and the lines it adds up, found by following its formulas. None
  /// for a workbook with no formula that reads a number, and for one made to be slow to
  /// follow: one whose formulas read more cells than any real workbook's do, or whose
  /// following takes more looks than any real workbook's does.
  pub fn audit(&self) -> Option<Audit> {
    self.audit_within(MAX_STEPS)
  }

  /// [`Workbook::audit`], allowed `steps` looks at cells once the formulas are read.
  fn audit_within(&self, steps: usize) -> Option<Audit> {
    let graph = Graph::of(self)?;
    followed(&graph, &mut Steps(steps)).ok().flatten()
  }
}

#[cfg(test)]
mod tests;
