//! The document's own text: its paragraphs, with how each run is set, and the rows of its
//! tables, with the edge each cell ends at.

use std::collections::HashMap;

use encoding_rs::Encoding;

use crate::token::{Token, Tokens};

// The model a document is read into is `contentdown`'s, shared with the other formats that
// record a flow of text. It is this crate's API under the names it always had.
pub use contentdown::document::{
  Align, Block, Body, DownFrom, Inline, Paper, Paragraph, Placing, Row, Run, Running,
};

/// Lengths in the file are in twentieths of a point.
fn points(twips: i32) -> f64 {
  f64::from(twips) / 20.0
}

/// A font of the file's table: its name, and the code page its character set is read in.
#[derive(Default)]
struct Font {
  name: String,
  page: Option<u16>,
}

/// The code page of a font's character set, as the format numbers them. None for the sets
/// that mean "the file's own".
fn page_of_charset(charset: i32) -> Option<u16> {
  Some(match charset {
    77 => 10000,
    128 => 932,
    129 => 949,
    134 => 936,
    136 => 950,
    161 => 1253,
    162 => 1254,
    163 => 1258,
    177 => 1255,
    178 => 1256,
    186 => 1257,
    204 => 1251,
    222 => 874,
    238 => 1250,
    _ => return None,
  })
}

/// The encoding of a code page. The Western page stands in for one that is not known.
fn encoding_of(page: u16) -> &'static Encoding {
  let label = match page {
    874 => "windows-874",
    932 => "shift_jis",
    936 => "gbk",
    949 => "euc-kr",
    950 => "big5",
    1250 => "windows-1250",
    1251 => "windows-1251",
    1253 => "windows-1253",
    1254 => "windows-1254",
    1255 => "windows-1255",
    1256 => "windows-1256",
    1257 => "windows-1257",
    1258 => "windows-1258",
    10000 => "macintosh",
    65001 => "utf-8",
    _ => "windows-1252",
  };
  Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252)
}

/// What the group being read holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Holds {
  /// The document's text.
  Text,
  /// The table of fonts.
  Fonts,
  /// Something that is not the document's text: a table of colors or of styles, a picture, a
  /// header, a field's instruction.
  Passed,
}

/// What a group can change, and gives back when it closes.
#[derive(Clone)]
struct State {
  holds: Holds,
  /// Nothing has come in the group yet, so a control word may still say what it holds.
  fresh: bool,
  font: i32,
  size: f64,
  bold: bool,
  italic: bool,
  hidden: bool,
  /// How many characters after a `\u` stand in for it.
  stand_ins: usize,
  para: Para,
}

/// What the file has said of the paragraph being read, which holds from one paragraph to the
/// next until the file starts over with `\pard`.
#[derive(Clone, Copy, Default, PartialEq)]
struct Para {
  align: Align,
  indent_left: f64,
  indent_right: f64,
  in_table: bool,
  /// Where the paragraph is put, as far as the file has said: across, down, how wide, and
  /// what each is measured from.
  placed: Placed,
  space_before: f64,
  space_after: f64,
}

/// The place a paragraph is given, a part at a time.
#[derive(Clone, Copy, Default, PartialEq)]
struct Placed {
  x: Option<f64>,
  y: Option<f64>,
  width: Option<f64>,
  /// How tall the frame is, as the file states it. Kept to tell one frame from another.
  tall: Option<i32>,
  from_page_side: bool,
  from_page_top: bool,
}

impl Placed {
  /// A place, once the file has said both how far across and how far down.
  fn placing(self) -> Option<Placing> {
    Some(Placing {
      x: self.x?,
      y: self.y?,
      width: self.width,
      from_page_side: self.from_page_side,
      down_from: match self.from_page_top {
        true => DownFrom::PageTop,
        false => DownFrom::TopMargin,
      },
    })
  }
}

/// Groups that open with one of these words do not hold the document's text.
const PASSED: [&str; 18] = [
  "colortbl",
  "stylesheet",
  "info",
  "pict",
  "object",
  "headerl",
  "footerl",
  "footnote",
  "fldinst",
  "nonshppict",
  "shprslt",
  "sp",
  "xe",
  "tc",
  "bkmkstart",
  "bkmkend",
  "datafield",
  "themedata",
];

const DEFAULT_SIZE: f64 = 12.0;

/// Which header or footer a group is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Runs {
  Header,
  Footer,
  FirstHeader,
  FirstFooter,
}

impl Runs {
  /// The header or footer a group that opens with this word is. One for left pages only is
  /// not among them: a page here is not known to be a left one.
  fn of(name: &str) -> Option<Runs> {
    match name {
      "header" | "headerr" => Some(Runs::Header),
      "footer" | "footerr" => Some(Runs::Footer),
      "headerf" => Some(Runs::FirstHeader),
      "footerf" => Some(Runs::FirstFooter),
      _ => None,
    }
  }
}

/// The parts of a reader that hold one stretch of text: the document's own, or a header's.
#[derive(Default)]
struct Story {
  blocks: Vec<Block>,
  inlines: Vec<Inline>,
  cell: Vec<Paragraph>,
  cells: Vec<Vec<Paragraph>>,
  edges: Vec<f64>,
  row_left: f64,
  frame: Option<Placed>,
}

/// A header or footer being read: which, how deep in groups it opened, and the text that was
/// being read when it did, which is gone back to when it closes.
struct Aside {
  runs: Runs,
  depth: usize,
  held: Story,
}

struct Reader {
  fonts: HashMap<i32, Font>,
  /// The file's own code page.
  page: u16,
  default_font: i32,
  paper: Paper,
  blocks: Vec<Block>,
  /// The paragraph being read.
  inlines: Vec<Inline>,
  /// The cell being read, and the cells of the row before it.
  cell: Vec<Paragraph>,
  cells: Vec<Vec<Paragraph>>,
  edges: Vec<f64>,
  row_left: f64,
  /// The frame the last paragraph was placed in, where it was placed.
  frame: Option<Placed>,
  /// Bytes given in hex that have not been turned into text yet: two of them can make one
  /// character.
  bytes: Vec<u8>,
  bytes_page: u16,
  /// The first half of a character given as two numbers.
  high: Option<u16>,
  /// How many of the characters that follow stand in for one already read.
  to_skip: usize,
  /// The font of the table that is being read.
  listing: i32,
  /// The header or footer being read, when one is.
  aside: Option<Aside>,
  /// The headers and footers read, each the first the file states of its kind.
  header: Option<Vec<Block>>,
  footer: Option<Vec<Block>>,
  first_header: Option<Vec<Block>>,
  first_footer: Option<Vec<Block>>,
  /// The file says its first page has a header and a footer of its own.
  title_page: bool,
}

impl Reader {
  /// Changes the text being read for another, and gives what was being read.
  fn exchange(&mut self, mut story: Story) -> Story {
    std::mem::swap(&mut self.blocks, &mut story.blocks);
    std::mem::swap(&mut self.inlines, &mut story.inlines);
    std::mem::swap(&mut self.cell, &mut story.cell);
    std::mem::swap(&mut self.cells, &mut story.cells);
    std::mem::swap(&mut self.edges, &mut story.edges);
    std::mem::swap(&mut self.row_left, &mut story.row_left);
    std::mem::swap(&mut self.frame, &mut story.frame);
    story
  }

  /// Starts a header or footer in the group `depth` deep, setting the document's text aside.
  fn enter(&mut self, runs: Runs, depth: usize) {
    let held = self.exchange(Story::default());
    self.aside = Some(Aside { runs, depth, held });
  }

  /// Ends the header or footer being read when the group closing, `depth` deep, is the one
  /// that opened it, and goes back to the text set aside. Says whether it did.
  fn leave(&mut self, depth: usize, state: &State) -> bool {
    if self.aside.as_ref().is_none_or(|aside| aside.depth != depth) {
      return false;
    }
    let Some(aside) = self.aside.take() else {
      return false;
    };
    self.finish(state);
    let read = self.exchange(aside.held).blocks;
    // The first the file states of a kind is the document's: one a later section states for
    // itself is not followed.
    let kept = match aside.runs {
      Runs::Header => &mut self.header,
      Runs::Footer => &mut self.footer,
      Runs::FirstHeader => &mut self.first_header,
      Runs::FirstFooter => &mut self.first_footer,
    };
    kept.get_or_insert(read);
    true
  }

  /// Ends what is still open of the text being read: a paragraph with no end mark, and a row
  /// whose last cells were not closed.
  fn finish(&mut self, state: &State) {
    self.flush(state);
    if !self.inlines.is_empty() {
      self.end_paragraph(state);
    }
    if !self.cells.is_empty() || !self.cell.is_empty() {
      if !self.cell.is_empty() {
        self.cells.push(std::mem::take(&mut self.cell));
      }
      self.end_row();
    }
  }

  fn page_for(&self, state: &State) -> u16 {
    let of_font = self.fonts.get(&state.font).and_then(|font| font.page);
    of_font.unwrap_or(self.page)
  }

  /// Text set as the group now has it, joined to the run before when that is set the same.
  fn push(&mut self, text: &str, state: &State) {
    if text.is_empty() || state.hidden {
      return;
    }
    let font = self.fonts.get(&state.font).map(|font| font.name.clone());
    let font = font.filter(|name| !name.is_empty());
    if let Some(Inline::Text(run)) = self.inlines.last_mut()
      && run.size == state.size
      && run.bold == state.bold
      && run.italic == state.italic
      && run.font == font
    {
      run.text.push_str(text);
      return;
    }
    self.inlines.push(Inline::Text(Run {
      text: text.to_string(),
      size: state.size,
      bold: state.bold,
      italic: state.italic,
      font,
    }));
  }

  /// Turns the bytes waiting into text.
  fn flush(&mut self, state: &State) {
    if self.bytes.is_empty() {
      return;
    }
    let bytes = std::mem::take(&mut self.bytes);
    let (text, _) = encoding_of(self.bytes_page).decode_without_bom_handling(&bytes);
    self.push(&text, state);
  }

  fn paragraph(&mut self, state: &State) -> Paragraph {
    self.flush(state);
    let para = state.para;
    let placing = para.placed.placing();
    // Paragraphs one after another that are placed in the same words are one frame.
    let continues_frame = placing.is_some() && self.frame == Some(para.placed);
    self.frame = placing.map(|_| para.placed);
    Paragraph {
      inlines: std::mem::take(&mut self.inlines),
      align: para.align,
      indent_left: para.indent_left,
      indent_right: para.indent_right,
      placing,
      continues_frame,
      space_before: para.space_before,
      space_after: para.space_after,
    }
  }

  fn end_paragraph(&mut self, state: &State) {
    let paragraph = self.paragraph(state);
    match state.para.in_table {
      true => self.cell.push(paragraph),
      false => self.blocks.push(Block::Paragraph(paragraph)),
    }
  }

  fn end_cell(&mut self, state: &State) {
    let paragraph = self.paragraph(state);
    if !paragraph.inlines.is_empty() {
      self.cell.push(paragraph);
    }
    self.cells.push(std::mem::take(&mut self.cell));
  }

  fn end_row(&mut self) {
    self.blocks.push(Block::Row(Row {
      left: self.row_left,
      edges: self.edges.clone(),
      cells: std::mem::take(&mut self.cells),
    }));
  }

  /// A character given by its number, which may be half of one.
  fn unit(&mut self, unit: u16, state: &State) {
    self.flush(state);
    let text = match (self.high.take(), unit) {
      (Some(high), 0xDC00..=0xDFFF) => String::from_utf16_lossy(&[high, unit]),
      (_, 0xD800..=0xDBFF) => {
        self.high = Some(unit);
        return;
      }
      (_, unit) => String::from_utf16_lossy(&[unit]),
    };
    self.push(&text, state);
  }

  fn word(&mut self, name: &str, parameter: Option<i32>, state: &mut State) {
    let number = parameter.unwrap_or(0);
    let on = parameter != Some(0);
    let character = match name {
      "emdash" => Some("—"),
      "endash" => Some("–"),
      "lquote" => Some("‘"),
      "rquote" => Some("’"),
      "ldblquote" => Some("“"),
      "rdblquote" => Some("”"),
      "bullet" => Some("•"),
      "emspace" | "enspace" | "qmspace" => Some(" "),
      _ => None,
    };
    if let Some(character) = character {
      self.flush(state);
      self.push(character, state);
      self.to_skip = 0;
      return;
    }
    if name == "u" {
      // Written as a number of sixteen bits with a sign.
      self.unit(number as u16, state);
      self.to_skip = state.stand_ins;
      return;
    }
    self.to_skip = 0;
    match name {
      "tab" => {
        self.flush(state);
        if !state.hidden {
          self.inlines.push(Inline::Tab);
        }
      }
      "line" => {
        self.flush(state);
        if !state.hidden {
          self.inlines.push(Inline::LineBreak);
        }
      }
      "par" | "sect" => self.end_paragraph(state),
      "page" => {
        self.flush(state);
        if !self.inlines.is_empty() {
          self.end_paragraph(state);
        }
        self.blocks.push(Block::PageBreak);
        self.frame = None;
      }
      "cell" | "nestcell" => self.end_cell(state),
      "row" | "nestrow" => self.end_row(),
      "trowd" => {
        self.edges.clear();
        self.row_left = 0.0;
      }
      "cellx" => self.edges.push(points(number)),
      "trleft" => self.row_left = points(number),
      "titlepg" => self.title_page = true,
      "intbl" => state.para.in_table = true,
      "pard" => {
        state.para = Para::default();
      }
      "posx" => state.para.placed.x = Some(points(number)),
      "posy" => state.para.placed.y = Some(points(number)),
      "absw" => state.para.placed.width = Some(points(number)).filter(|width| *width > 0.0),
      "phpg" => state.para.placed.from_page_side = true,
      "phmrg" | "phcol" => state.para.placed.from_page_side = false,
      "pvpg" => state.para.placed.from_page_top = true,
      "pvmrg" => state.para.placed.from_page_top = false,
      "absh" => state.para.placed.tall = Some(number),
      "sb" => state.para.space_before = points(number),
      "sa" => state.para.space_after = points(number),
      "plain" => {
        self.flush(state);
        state.font = self.default_font;
        state.size = DEFAULT_SIZE;
        state.bold = false;
        state.italic = false;
        state.hidden = false;
      }
      "ql" => state.para.align = Align::Left,
      "qc" => state.para.align = Align::Center,
      "qr" => state.para.align = Align::Right,
      "qj" => state.para.align = Align::Justified,
      "li" => state.para.indent_left = points(number),
      "ri" => state.para.indent_right = points(number),
      "b" => {
        self.flush(state);
        state.bold = on;
      }
      "i" => {
        self.flush(state);
        state.italic = on;
      }
      "v" => {
        self.flush(state);
        state.hidden = on;
      }
      "fs" => {
        self.flush(state);
        state.size = (f64::from(number) / 2.0).max(1.0);
      }
      "f" => {
        self.flush(state);
        state.font = number;
      }
      "uc" => state.stand_ins = usize::try_from(number).unwrap_or(0),
      "ansicpg" => self.page = u16::try_from(number).unwrap_or(1252),
      "deff" => {
        self.default_font = number;
        state.font = number;
      }
      "paperw" => self.paper.width = points(number),
      "paperh" => self.paper.height = points(number),
      "margl" => self.paper.left = points(number),
      "margr" => self.paper.right = points(number),
      "margt" => self.paper.top = points(number),
      "margb" => self.paper.bottom = points(number),
      _ => {}
    }
  }

  /// One token of the table of fonts.
  fn font(&mut self, token: Token<'_>) {
    match token {
      Token::Word {
        name: "f",
        parameter,
      } => self.listing = parameter.unwrap_or(0),
      Token::Word {
        name: "fcharset",
        parameter,
      } => {
        let page = page_of_charset(parameter.unwrap_or(0));
        self.fonts.entry(self.listing).or_default().page = page;
      }
      Token::Text(text) => {
        // A name ends with a semicolon.
        let name: String = text
          .iter()
          .take_while(|byte| **byte != b';')
          .map(|byte| char::from(*byte))
          .collect();
        self
          .fonts
          .entry(self.listing)
          .or_default()
          .name
          .push_str(&name);
      }
      _ => {}
    }
  }
}

/// The content of the document in `bytes`, which open as RTF does.
pub(crate) fn read(bytes: &[u8]) -> Body {
  let mut reader = Reader {
    fonts: HashMap::new(),
    page: 1252,
    default_font: 0,
    // Letter, with the margins a file that states none is given.
    paper: Paper {
      width: 612.0,
      height: 792.0,
      left: 90.0,
      right: 90.0,
      top: 72.0,
      bottom: 72.0,
    },
    blocks: Vec::new(),
    inlines: Vec::new(),
    cell: Vec::new(),
    cells: Vec::new(),
    edges: Vec::new(),
    row_left: 0.0,
    frame: None,
    bytes: Vec::new(),
    bytes_page: 1252,
    high: None,
    to_skip: 0,
    listing: 0,
    aside: None,
    header: None,
    footer: None,
    first_header: None,
    first_footer: None,
    title_page: false,
  };
  let mut state = State {
    holds: Holds::Text,
    fresh: false,
    font: 0,
    size: DEFAULT_SIZE,
    bold: false,
    italic: false,
    hidden: false,
    stand_ins: 1,
    para: Para::default(),
  };
  let mut stack: Vec<State> = Vec::new();
  for token in Tokens::over(bytes) {
    match token {
      Token::Open => {
        reader.flush(&state);
        stack.push(state.clone());
        state.fresh = true;
        continue;
      }
      Token::Close => {
        reader.flush(&state);
        let aside = reader.leave(stack.len(), &state);
        // What a group changed of the paragraph stays: a paragraph is ended outside the
        // group its text is in as often as inside it. A header's paragraphs are its own.
        if let Some(before) = stack.pop() {
          let kept = state.para;
          let was_text = state.holds == Holds::Text;
          state = before;
          if was_text && state.holds == Holds::Text && !aside {
            state.para = kept;
          }
        }
        continue;
      }
      _ => {}
    }
    // The first thing in a group may say that the group is not the document's text.
    if state.fresh {
      match token {
        // A group marked to be passed over by a reader that does not know it.
        Token::Symbol(b'*') => {
          state.holds = Holds::Passed;
          continue;
        }
        Token::Word {
          name: "fonttbl", ..
        } => state.holds = Holds::Fonts,
        Token::Word { name, .. } if PASSED.contains(&name) => state.holds = Holds::Passed,
        Token::Word { name, .. } if state.holds == Holds::Text => {
          if let Some(runs) = Runs::of(name) {
            match reader.aside {
              // A header inside a header is nothing a file has.
              Some(_) => state.holds = Holds::Passed,
              None => {
                reader.enter(runs, stack.len());
                state.para = Para::default();
              }
            }
            state.fresh = false;
            continue;
          }
        }
        _ => {}
      }
      state.fresh = false;
    }
    match state.holds {
      Holds::Passed => {}
      Holds::Fonts => reader.font(token),
      Holds::Text => match token {
        Token::Word { name, parameter } => reader.word(name, parameter, &mut state),
        Token::Hex(byte) => {
          if reader.to_skip > 0 {
            reader.to_skip -= 1;
            continue;
          }
          let page = reader.page_for(&state);
          if page != reader.bytes_page {
            reader.flush(&state);
            reader.bytes_page = page;
          }
          reader.bytes.push(byte);
        }
        Token::Text(text) => {
          let skipped = reader.to_skip.min(text.len());
          reader.to_skip -= skipped;
          let text = &text[skipped..];
          if text.is_empty() {
            continue;
          }
          reader.flush(&state);
          let page = reader.page_for(&state);
          let (text, _) = encoding_of(page).decode_without_bom_handling(text);
          reader.push(&text, &state);
        }
        Token::Symbol(symbol) => {
          reader.flush(&state);
          reader.to_skip = 0;
          match symbol {
            b'~' => reader.push("\u{a0}", &state),
            b'_' => reader.push("-", &state),
            b'{' | b'}' | b'\\' => reader.push(&char::from(symbol).to_string(), &state),
            // A place where a word may be broken, and anything else: nothing to show.
            _ => {}
          }
        }
        Token::Binary(_) | Token::Open | Token::Close => {}
      },
    }
  }
  // A file that ends inside a header still has the text that came before it.
  if let Some(aside) = reader.aside.take() {
    reader.finish(&state);
    reader.exchange(aside.held);
  }
  reader.finish(&state);
  let title_page = reader.title_page;
  Body {
    blocks: reader.blocks,
    paper: reader.paper,
    running: Running {
      header: reader.header.unwrap_or_default(),
      footer: reader.footer.unwrap_or_default(),
      first_header: title_page.then(|| reader.first_header.unwrap_or_default()),
      first_footer: title_page.then(|| reader.first_footer.unwrap_or_default()),
    },
  }
}

#[cfg(test)]
mod tests;
