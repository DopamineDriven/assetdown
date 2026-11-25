//! A workbook's total, and the lines it adds up, found by following its formulas.
//!
//! A sheet's labels say what its maker called things; its formulas say what is added to
//! what. The total is a formula that no other formula reads: one shown as money before one
//! that is not, and of those the one that reaches the most cells. From it the formulas are
//! followed down: one that comes to a sum of cells is a subtotal, and what it adds is looked
//! at in turn; anything else (a number typed in, a quantity times a price, a charge worked
//! out from a subtotal) is a line. So the lines come to the total, or the workbook says that
//! they do not.
//!
//! What a formula comes to is read from its text by `parse`, which sees a sum through what
//! is done to show it: a rounding, a writing of it as text, a joining of it to a word, and a
//! question that keeps it blank until there is something to show. A total written out as
//! text is one of the formulas, worth the sum of the cells it adds up.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::formula::{Reference, references};
use crate::workbook::{Cell, CellValue, Sheet, Workbook};

mod parse;

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
  /// What the total comes to: the number its cell holds, or, for a cell that holds its sum
  /// written out as text, the sum of the cells it adds up.
  pub value: f64,
  /// Whether the total's cell is shown as money: with a currency's sign, or with two places.
  pub shown_as_money: bool,
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
  /// Every number it reads, whatever it does with it, in the order it reads them.
  reads: Vec<CellAt>,
  /// The cells it adds up, each added (1) or taken away (-1), when what it comes to is their
  /// sum. A cell a question in the formula asks about is read, and is not among these.
  adds: Option<Vec<(CellAt, f64)>>,
  /// The two cells it multiplies, when what it comes to is one cell times another.
  product: Option<(CellAt, CellAt)>,
  /// What it comes to: the number its cell holds, or, for a cell that holds its sum written
  /// out as text, the sum of the cells it adds up.
  value: f64,
}

fn number(cell: &Cell) -> Option<f64> {
  match cell.value {
    CellValue::Number(value) => Some(value),
    _ => None,
  }
}

/// Whether text reads as an amount of money: it ends in two places of decimals, or it
/// carries a currency's sign beside its digits.
pub(crate) fn reads_as_money(text: &str) -> bool {
  let text = text.trim().trim_end_matches(')');
  let digits = |part: &str| part.len() == 2 && part.chars().all(|c| c.is_ascii_digit());
  let two_places = text
    .rsplit_once(['.', ','])
    .is_some_and(|(whole, places)| digits(places) && whole.ends_with(|c: char| c.is_ascii_digit()));
  let signed = text.chars().any(|c| c.is_ascii_digit())
    && text.chars().any(|c| "$\u{20ac}\u{a3}\u{a5}".contains(c));
  two_places || signed
}

/// Whether a cell is shown as money: with a currency's sign, or with two places. A cell
/// that holds text is, when the text reads as an amount.
fn money(cell: &Cell) -> bool {
  match &cell.value {
    CellValue::Text(text) => reads_as_money(text),
    _ => {
      cell.currency_symbol.is_some()
        || cell
          .format
          .as_deref()
          .is_some_and(|code| code.contains(".00") && !code.contains('%'))
    }
  }
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

  /// What a cell comes to: what its formula comes to, or the number it holds.
  fn value(&self, at: CellAt) -> Option<f64> {
    match self.formulas.get(&at) {
      Some(formula) => Some(formula.value),
      None => self.cell(at).and_then(number),
    }
  }

  /// Whether a cell's formula comes to a sum of cells.
  fn sums(&self, at: CellAt) -> bool {
    self
      .formulas
      .get(&at)
      .is_some_and(|formula| formula.adds.is_some())
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
          Some(text) if !text.is_empty() => text,
          _ => continue,
        };
        // A formula left with a number, or with text that holds one: a sum written out.
        let written_out = match &cell.value {
          CellValue::Number(_) => false,
          CellValue::Text(shown) if shown.chars().any(|c| c.is_ascii_digit()) => true,
          _ => continue,
        };
        let at = CellAt {
          sheet,
          row: *row,
          col: *col,
        };
        let found = references(text);
        // The numbers each reference holds.
        let held: Vec<Vec<CellAt>> = found
          .iter()
          .map(|reference| {
            let on = match &reference.sheet {
              None => Some(sheet),
              Some(name) => named.get(&name.to_lowercase()).copied(),
            };
            on.map_or_else(Vec::new, |on| numbers_in(on, reference))
          })
          .collect();
        let shape = parse::shape(text, &found);
        let adds = shape.adds.and_then(|added| {
          let mut seen = HashSet::new();
          let cells: Vec<(CellAt, f64)> = added
            .into_iter()
            .flat_map(|(index, sign)| held[index].iter().map(move |cell| (*cell, sign)))
            .filter(|(cell, _)| *cell != at && seen.insert(*cell))
            .collect();
          (!cells.is_empty()).then_some(cells)
        });
        let value = match (number(cell), &adds) {
          (Some(value), _) => value,
          (None, Some(added)) if written_out => added
            .iter()
            .filter_map(|(cell, sign)| {
              Some(
                sign
                  * number(
                    workbook.sheets[cell.sheet]
                      .cells
                      .get(&(cell.row, cell.col))?,
                  )?,
              )
            })
            .sum(),
          // Text that is worked out some other way than by adding cells up is not followed.
          _ => continue,
        };
        // The cell a reference is when it is one cell with a number in it.
        let single = |index: usize| match held[index][..] {
          [one] => Some(one),
          _ => None,
        };
        let product = shape
          .product
          .and_then(|(first, second)| Some((single(first)?, single(second)?)));
        let mut seen = HashSet::new();
        let reads: Vec<CellAt> = held
          .into_iter()
          .flatten()
          .filter(|cell| *cell != at && seen.insert(*cell))
          .collect();
        reads_in_all += reads.len();
        if reads_in_all > MAX_READS {
          return None;
        }
        read.extend(reads.iter().copied());
        formulas.insert(
          at,
          Formula {
            reads,
            adds,
            product,
            value,
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
      for read in &formula.reads {
        if reached.insert(*read) {
          open.push(*read);
        }
      }
    }
    Ok(reached.len())
  }

  /// The total: of the formulas nothing reads, on the sheets that are shown, one shown as
  /// money before one that is not; of those, the one that reaches the most cells; of those,
  /// the lowest on its sheet.
  ///
  /// What the labels on the page gave as the total is not asked. It was tried: a formula
  /// that came to the labels' amount was taken before any other, and measured on real
  /// workbooks, that moved totals from right to wrong far more often than the other way. A
  /// label that reads as a total is often on a part of the bill.
  fn total(&self, steps: &mut Steps) -> Result<Option<CellAt>, Spent> {
    let mut best = None;
    for (at, formula) in &self.formulas {
      if formula.reads.is_empty() || self.read.contains(at) || self.workbook.sheets[at.sheet].hidden
      {
        continue;
      }
      let shown_as_money = self.cell(*at).is_some_and(money);
      let rank = (
        shown_as_money,
        self.reach(*at, steps)?,
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
    let amount = sign * self.value(at)?;
    if amount == 0.0 {
      return None;
    }
    // Of the two cells a line multiplies, the price is the one shown as money. Where the
    // formats do not say (neither is shown so, or both are), the one that is a sum of cells
    // is how many: hours added up, at a rate. Failing both, which is which is not known.
    let (quantity, unit_price) = self
      .formulas
      .get(&at)
      .and_then(|formula| formula.product)
      .and_then(|(first, second)| {
        let shown = |cell: CellAt| self.cell(cell).is_some_and(money);
        match (shown(first), shown(second)) {
          (false, true) => Some((first, second)),
          (true, false) => Some((second, first)),
          _ => match (self.sums(first), self.sums(second)) {
            (true, false) => Some((first, second)),
            (false, true) => Some((second, first)),
            _ => None,
          },
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
        Some(Formula {
          adds: Some(added), ..
        }) => open.extend(
          added
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
    if !self.sums(first) && !self.sums(second) {
      return Ok(None);
    }
    // Every formula is looked at for one that comes to the same.
    steps.take(self.formulas.len())?;
    let mut others: Vec<CellAt> = self
      .formulas
      .iter()
      .filter(|(at, formula)| {
        formula.adds.is_some() && !seen.contains(at) && comes_to(formula.value, line.amount, 1)
      })
      .map(|(at, _)| *at)
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
  let Some(value) = graph.value(total) else {
    return Ok(None);
  };
  let mut seen = HashSet::new();
  // A total that adds cells up is followed down to its lines, and one that is one cell times
  // another (so many, at so much) is the one line itself. Any other is not followed: it is
  // all there is.
  let found = match graph.formulas.get(&total) {
    Some(formula) if formula.adds.is_some() || formula.product.is_some() => {
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
    shown_as_money: graph.cell(total).is_some_and(money),
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
    self.audited(MAX_STEPS)
  }

  /// The audit, allowed `steps` looks at cells once the formulas are read.
  fn audited(&self, steps: usize) -> Option<Audit> {
    let graph = Graph::of(self)?;
    followed(&graph, &mut Steps(steps)).ok().flatten()
  }
}

#[cfg(test)]
mod tests;
