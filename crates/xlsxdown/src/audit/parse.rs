//! What a formula does with the cells it reads, as far as following totals needs to know.
//!
//! A formula is read into the little of its grammar that matters here: references, numbers,
//! text, the signs between them, and calls of functions. From that, two questions are
//! answered. Does the formula's value come to a sum of the cells it reads, whatever it then
//! does to show the sum (round it, write it as text, join it to a word, or keep it blank
//! until there is something to show)? And is its value one cell times another, guarded or
//! rounded the same ways? Anything the reading does not know (a name, an error, a function
//! not listed) makes the answer to both no.

use crate::formula::Reference;

/// How deep brackets and calls may nest before a formula is given up on. Excel stops at 64.
const MAX_DEPTH: usize = 64;

/// What a formula comes to. Each reference is named by its place among the references of
/// the formula, in the order they stand in it.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Shape {
  /// The references the formula adds up, each added (1.0) or taken away (-1.0), when its
  /// value is their sum. What a question in the formula asks about is not among them.
  pub adds: Option<Vec<(usize, f64)>>,
  /// The two cells the formula multiplies, when its value is one cell times another.
  pub product: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
  Reference(usize),
  Number(f64),
  Text,
  /// The name of a function, with the bracket that opens its arguments still to come.
  Function(String),
  /// Something whose value is not followed: a name, an error, a reference this workbook
  /// cannot resolve, a list of constants.
  Opaque,
  /// A sign: `+ - * / ^ & % ( ) ,`, or `=` for any comparison.
  Sign(char),
}

/// The formula as tokens. None for a formula with something in it that is not closed.
fn tokens(formula: &str, found: &[Reference]) -> Option<Vec<Token>> {
  let bytes = formula.as_bytes();
  let mut out = Vec::new();
  let mut references = found.iter().enumerate().peekable();
  let mut at = 0;
  while at < bytes.len() {
    // References are in the order they stand, so the next one is the only one to look for.
    if let Some((index, reference)) = references.peek()
      && reference.start == at
    {
      out.push(Token::Reference(*index));
      at = reference.end.max(at + 1);
      references.next();
      continue;
    }
    let byte = bytes[at];
    match byte {
      b' ' | b'\t' | b'\r' | b'\n' => at += 1,
      b'"' => {
        // To the closing mark; a doubled mark inside is one mark of the text.
        let mut end = at + 1;
        loop {
          match bytes.get(end)? {
            b'"' if bytes.get(end + 1) == Some(&b'"') => end += 2,
            b'"' => break,
            _ => end += 1,
          }
        }
        out.push(Token::Text);
        at = end + 1;
      }
      b'0'..=b'9' | b'.' => {
        let mut end = at;
        while bytes
          .get(end)
          .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
        {
          end += 1;
        }
        // An exponent, with its sign.
        if matches!(bytes.get(end), Some(b'E' | b'e')) {
          let mut after = end + 1;
          if matches!(bytes.get(after), Some(b'+' | b'-')) {
            after += 1;
          }
          if bytes.get(after).is_some_and(u8::is_ascii_digit) {
            end = after;
            while bytes.get(end).is_some_and(u8::is_ascii_digit) {
              end += 1;
            }
          }
        }
        out.push(match formula[at..end].parse() {
          Ok(number) => Token::Number(number),
          Err(_) => Token::Opaque,
        });
        at = end;
      }
      b'+' | b'-' | b'*' | b'/' | b'^' | b'&' | b'%' | b'(' | b')' | b',' => {
        out.push(Token::Sign(char::from(byte)));
        at += 1;
      }
      b'=' | b'<' | b'>' => {
        out.push(Token::Sign('='));
        at += 1;
        if matches!(bytes.get(at), Some(b'=' | b'>')) {
          at += 1;
        }
      }
      // A sheet's name in marks, before a reference that was not taken for one of this
      // workbook's: another workbook's, or a run of sheets.
      b'\'' => {
        let mut end = at + 1;
        loop {
          match bytes.get(end)? {
            b'\'' if bytes.get(end + 1) == Some(&b'\'') => end += 2,
            b'\'' => break,
            _ => end += 1,
          }
        }
        out.push(Token::Opaque);
        at = end + 1;
      }
      // Another workbook's number, a table's column, or a list of constants.
      b'[' | b'{' => {
        let close = if byte == b'[' { b']' } else { b'}' };
        let mut depth = 0usize;
        let mut end = at;
        loop {
          let inner = *bytes.get(end)?;
          if inner == byte {
            depth += 1;
          } else if inner == close {
            depth -= 1;
            if depth == 0 {
              break;
            }
          }
          end += 1;
        }
        out.push(Token::Opaque);
        at = end + 1;
      }
      _ => {
        // A word: the name of a function when a bracket follows it, else a name, a truth
        // value, an error, or what is left of a reference that was not taken for one.
        let mut end = at;
        while bytes.get(end).is_some_and(|b| {
          !matches!(
            b,
            b' '
              | b'\t'
              | b'\r'
              | b'\n'
              | b'"'
              | b'+'
              | b'-'
              | b'*'
              | b'/'
              | b'^'
              | b'&'
              | b'%'
              | b'('
              | b')'
              | b','
              | b'='
              | b'<'
              | b'>'
              | b'\''
              | b'['
              | b'{'
          )
        }) {
          end += 1;
        }
        // A byte that starts no word and is no sign cannot be passed over one at a time.
        if end == at {
          return None;
        }
        // The end of a word is on a character's boundary: every byte it stops at is ASCII.
        let word = &formula[at..end];
        out.push(
          if bytes.get(end) == Some(&b'(') && !word.contains(['!', '#', ':']) {
            Token::Function(word.to_ascii_uppercase())
          } else {
            Token::Opaque
          },
        );
        at = end;
      }
    }
  }
  Some(out)
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
  Reference(usize),
  Number(f64),
  Text,
  /// An argument left out.
  Nothing,
  /// A value that is not followed.
  Opaque,
  /// Terms added, each with its sign.
  Added(Vec<(f64, Expr)>),
  Times(Box<Expr>, Box<Expr>),
  Joined(Vec<Expr>),
  Call(String, Vec<Expr>),
}

/// A reader of tokens into an expression, one level of precedence to a method.
struct Reader<'a> {
  tokens: &'a [Token],
  at: usize,
}

impl Reader<'_> {
  fn peek(&self) -> Option<&Token> {
    self.tokens.get(self.at)
  }

  fn take(&mut self, sign: char) -> bool {
    let found = self.peek() == Some(&Token::Sign(sign));
    if found {
      self.at += 1;
    }
    found
  }

  /// A comparison, or what stands on its own. A comparison's value is not followed.
  fn compared(&mut self, depth: usize) -> Option<Expr> {
    let first = self.joined(depth)?;
    if !self.take('=') {
      return Some(first);
    }
    self.joined(depth)?;
    while self.take('=') {
      self.joined(depth)?;
    }
    Some(Expr::Opaque)
  }

  fn joined(&mut self, depth: usize) -> Option<Expr> {
    let first = self.added(depth)?;
    if self.peek() != Some(&Token::Sign('&')) {
      return Some(first);
    }
    let mut parts = vec![first];
    while self.take('&') {
      parts.push(self.added(depth)?);
    }
    Some(Expr::Joined(parts))
  }

  fn added(&mut self, depth: usize) -> Option<Expr> {
    let first = self.multiplied(depth)?;
    if !matches!(self.peek(), Some(Token::Sign('+' | '-'))) {
      return Some(first);
    }
    let mut terms = vec![(1.0, first)];
    loop {
      let sign = if self.take('+') {
        1.0
      } else if self.take('-') {
        -1.0
      } else {
        break;
      };
      terms.push((sign, self.multiplied(depth)?));
    }
    Some(Expr::Added(terms))
  }

  fn multiplied(&mut self, depth: usize) -> Option<Expr> {
    let mut left = self.raised(depth)?;
    loop {
      if self.take('*') {
        let right = self.raised(depth)?;
        left = Expr::Times(Box::new(left), Box::new(right));
      } else if self.take('/') {
        self.raised(depth)?;
        left = Expr::Opaque;
      } else {
        return Some(left);
      }
    }
  }

  fn raised(&mut self, depth: usize) -> Option<Expr> {
    let base = self.signed(depth)?;
    if !self.take('^') {
      return Some(base);
    }
    self.signed(depth)?;
    while self.take('^') {
      self.signed(depth)?;
    }
    Some(Expr::Opaque)
  }

  fn signed(&mut self, depth: usize) -> Option<Expr> {
    let mut sign = 1.0;
    let mut any = false;
    loop {
      if self.take('-') {
        sign = -sign;
      } else if !self.take('+') {
        break;
      }
      any = true;
    }
    let value = self.percent(depth)?;
    Some(if any {
      Expr::Added(vec![(sign, value)])
    } else {
      value
    })
  }

  fn percent(&mut self, depth: usize) -> Option<Expr> {
    let value = self.single(depth)?;
    if !self.take('%') {
      return Some(value);
    }
    while self.take('%') {}
    Some(Expr::Opaque)
  }

  fn single(&mut self, depth: usize) -> Option<Expr> {
    let depth = depth.checked_sub(1)?;
    let token = self.peek()?.clone();
    self.at += 1;
    match token {
      Token::Reference(index) => Some(Expr::Reference(index)),
      Token::Number(number) => Some(Expr::Number(number)),
      Token::Text => Some(Expr::Text),
      Token::Opaque => Some(Expr::Opaque),
      Token::Sign('(') => {
        let inner = self.compared(depth)?;
        self.take(')').then_some(inner)
      }
      Token::Function(name) => {
        if !self.take('(') {
          return None;
        }
        let mut arguments = Vec::new();
        if self.take(')') {
          return Some(Expr::Call(name, arguments));
        }
        loop {
          // An argument left out stands between two commas, or before the closing bracket.
          if matches!(self.peek(), Some(Token::Sign(',' | ')'))) {
            arguments.push(Expr::Nothing);
          } else {
            arguments.push(self.compared(depth)?);
          }
          if self.take(')') {
            return Some(Expr::Call(name, arguments));
          }
          if !self.take(',') {
            return None;
          }
        }
      }
      Token::Sign(_) => None,
    }
  }
}

/// Whether an answer of a formula shows nothing: text, a nought, or an argument left out.
fn shows_nothing(expr: &Expr) -> bool {
  matches!(expr, Expr::Text | Expr::Nothing) || *expr == Expr::Number(0.0)
}

/// The one of two answers that is not nothing, when the other is.
fn the_answer_shown<'a>(then: &'a Expr, otherwise: &'a Expr) -> Option<&'a Expr> {
  match (shows_nothing(then), shows_nothing(otherwise)) {
    (false, true) => Some(then),
    (true, false) => Some(otherwise),
    _ => None,
  }
}

/// What is under the wrappers that leave a value what it is: a rounding to a fixed number
/// of places, a writing of it as text or a reading of it back, and a question whose other
/// answer shows nothing.
fn unwrapped(expr: &Expr) -> &Expr {
  let Expr::Call(name, arguments) = expr else {
    return expr;
  };
  let inner = match (name.as_str(), arguments.as_slice()) {
    ("ROUND", [inner, Expr::Number(_)]) | ("TEXT", [inner, _]) | ("VALUE" | "N", [inner]) => {
      Some(inner)
    }
    ("IF", [_, then]) => Some(then),
    ("IF", [_, then, otherwise]) => the_answer_shown(then, otherwise),
    ("IFERROR", [inner, fallback]) if shows_nothing(fallback) => Some(inner),
    _ => None,
  };
  inner.map_or(expr, unwrapped)
}

/// The references an expression adds up, when its value is their sum.
fn adds(expr: &Expr, found: &[Reference]) -> Option<Vec<(usize, f64)>> {
  let each = |parts: &[Expr]| -> Option<Vec<(usize, f64)>> {
    let mut all = Vec::new();
    for part in parts {
      all.extend(adds(part, found)?);
    }
    (!all.is_empty()).then_some(all)
  };
  match unwrapped(expr) {
    Expr::Reference(index) => Some(vec![(*index, 1.0)]),
    Expr::Added(terms) => {
      let mut all = Vec::new();
      for (sign, term) in terms {
        all.extend(
          adds(term, found)?
            .into_iter()
            .map(|(index, its)| (index, sign * its)),
        );
      }
      Some(all)
    }
    Expr::Call(name, arguments) => match (name.as_str(), arguments.as_slice()) {
      ("SUM", arguments) => each(arguments),
      // The two kinds of subtotal that are sums: of every cell, and of those not hidden.
      ("SUBTOTAL", [Expr::Number(kind), rest @ ..]) if *kind == 9.0 || *kind == 109.0 => each(rest),
      // A question with two answers that are the same sum, written two ways.
      ("IF", [_, then, otherwise]) => {
        let (first, second) = (adds(then, found)?, adds(otherwise, found)?);
        same_sum(&first, &second, found).then_some(first)
      }
      _ => None,
    },
    // One part of what is joined is a sum; the rest are words, or cells that hold them.
    Expr::Joined(parts) => {
      let mut sums = parts
        .iter()
        .filter(|part| !matches!(part, Expr::Text | Expr::Reference(_)));
      let only = sums.next()?;
      if sums.next().is_some() {
        return None;
      }
      adds(only, found)
    }
    _ => None,
  }
}

/// Whether two lists of references add up the same cells with the same signs.
fn same_sum(first: &[(usize, f64)], second: &[(usize, f64)], found: &[Reference]) -> bool {
  let named = |list: &[(usize, f64)]| {
    let mut all: Vec<_> = list
      .iter()
      .filter_map(|(index, sign)| {
        let reference = found.get(*index)?;
        let range = reference.range;
        Some((
          reference.sheet.as_deref().map(str::to_lowercase),
          (range.r0, range.c0, range.r1, range.c1),
          *sign < 0.0,
        ))
      })
      .collect();
    all.sort();
    all
  };
  first.len() == second.len() && named(first) == named(second)
}

/// The two cells an expression multiplies, when its value is one cell times another.
fn product(expr: &Expr, found: &[Reference]) -> Option<(usize, usize)> {
  let Expr::Times(first, second) = unwrapped(expr) else {
    return None;
  };
  let (Expr::Reference(first), Expr::Reference(second)) = (first.as_ref(), second.as_ref()) else {
    return None;
  };
  let one_cell = |index: &usize| {
    found.get(*index).is_some_and(|reference| {
      reference.range.r0 == reference.range.r1 && reference.range.c0 == reference.range.c1
    })
  };
  (one_cell(first) && one_cell(second)).then_some((*first, *second))
}

/// What a formula comes to. `found` is `references(formula)`.
pub(super) fn shape(formula: &str, found: &[Reference]) -> Shape {
  let Some(tokens) = tokens(formula, found) else {
    return Shape::default();
  };
  let mut reader = Reader {
    tokens: &tokens,
    at: 0,
  };
  let Some(expr) = reader.compared(MAX_DEPTH) else {
    return Shape::default();
  };
  // Tokens left over are a formula this reading did not follow to its end.
  if reader.at != tokens.len() {
    return Shape::default();
  }
  Shape {
    adds: adds(&expr, found),
    product: product(&expr, found),
  }
}

#[cfg(test)]
mod tests;
