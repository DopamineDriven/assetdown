//! What describes the text: the properties stated for any place in it, the styles, the
//! fonts, and the first section.
//!
//! Properties are kept as a run of them, each a number and a value. Those of a paragraph and
//! those of its characters are on pages of 512 bytes in the main stream, each page listing
//! the stretches of the stream it covers; a table in the table stream says which pages.

/// One property: its number, and its value.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Property<'a> {
  pub code: u16,
  pub value: &'a [u8],
}

impl Property<'_> {
  pub fn u8(&self) -> Option<u8> {
    self.value.first().copied()
  }

  pub fn u16(&self) -> Option<u16> {
    Some(u16::from_le_bytes(self.value.get(..2)?.try_into().ok()?))
  }

  pub fn i16(&self) -> Option<i16> {
    Some(i16::from_le_bytes(self.value.get(..2)?.try_into().ok()?))
  }
}

/// The properties of a run of them, in order. It ends at one whose value runs past the end.
pub(crate) fn properties(mut run: &[u8]) -> impl Iterator<Item = Property<'_>> {
  std::iter::from_fn(move || {
    let code = u16::from_le_bytes(run.get(..2)?.try_into().ok()?);
    let rest = &run[2..];
    // Three bits of the number say how long the value is.
    let (skip, length) = match code >> 13 {
      0 | 1 => (0, 1),
      2 | 4 | 5 => (0, 2),
      3 => (0, 4),
      7 => (0, 3),
      // A value with its length before it: in two bytes, counting one more than there is, for
      // the edges of a table's row; in one byte for the rest.
      _ if matches!(code, 0xD608 | 0xD609) => {
        let stated = usize::from(u16::from_le_bytes(rest.get(..2)?.try_into().ok()?));
        (2, stated.checked_sub(1)?)
      }
      // A change of tab stops with this length is counted another way, and is not read.
      _ if code == 0xC615 && rest.first() == Some(&255) => return None,
      _ => (1, usize::from(*rest.first()?)),
    };
    let value = rest.get(skip..skip + length)?;
    run = &rest[skip + length..];
    Some(Property { code, value })
  })
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

const PAGE: usize = 512;

/// Which properties a page holds: a paragraph's, with its style before them, or characters'.
#[derive(Clone, Copy)]
pub(crate) enum Of {
  Paragraphs,
  Characters,
}

/// The properties stated for stretches of the main stream.
pub(crate) struct Stated<'a> {
  /// Where each stretch starts and ends, and what is stated for it, in the order they lie in.
  stretches: Vec<(u32, u32, &'a [u8])>,
}

impl<'a> Stated<'a> {
  /// Read from the pages `listing` names: the stretches it covers, and then the number of
  /// the page for each.
  pub fn read(main: &'a [u8], listing: Option<&[u8]>, of: Of) -> Self {
    let mut stretches = Vec::new();
    let listing = listing.unwrap_or(&[]);
    let pages = listing.len().saturating_sub(4) / 8;
    for nth in 0..pages {
      let page = u32_at(listing, (pages + 1) * 4 + nth * 4)
        .and_then(|number| usize::try_from(number & 0x003F_FFFF).ok())
        .and_then(|number| main.get(number * PAGE..(number + 1) * PAGE));
      let Some(page) = page else {
        continue;
      };
      // The last byte counts the stretches. Their starts come first, with the end of the
      // last, and then for each where on the page its properties are, counted in pairs of
      // bytes, with nothing for a stretch that states none.
      let count = usize::from(page[PAGE - 1]);
      for stretch in 0..count {
        let (Some(start), Some(end)) = (u32_at(page, stretch * 4), u32_at(page, stretch * 4 + 4))
        else {
          break;
        };
        let slot = (count + 1) * 4
          + match of {
            Of::Paragraphs => stretch * 13,
            Of::Characters => stretch,
          };
        let data = page
          .get(slot)
          .map(|offset| usize::from(*offset) * 2)
          .filter(|at| *at != 0)
          .and_then(|at| match (of, *page.get(at)?) {
            // A paragraph's are counted in pairs of bytes, less one, or with a nought first
            // in pairs exactly.
            (Of::Paragraphs, 0) => {
              let pairs = usize::from(*page.get(at + 1)?);
              page.get(at + 2..at + 2 + pairs * 2)
            }
            (Of::Paragraphs, pairs) => page.get(at + 1..at + usize::from(pairs) * 2),
            (Of::Characters, length) => page.get(at + 1..at + 1 + usize::from(length)),
          });
        stretches.push((start, end, data.unwrap_or(&[])));
      }
    }
    stretches.sort_by_key(|(start, _, _)| *start);
    Stated { stretches }
  }

  /// What is stated for the character kept at `at`. Nothing where no stretch covers it.
  pub fn at(&self, at: u32) -> &'a [u8] {
    let after = self.stretches.partition_point(|(start, _, _)| *start <= at);
    match after.checked_sub(1).map(|index| self.stretches[index]) {
      Some((_, end, data)) if at < end => data,
      _ => &[],
    }
  }
}

/// One style: the style it starts from, and what it states of a paragraph and of its text.
struct Style<'a> {
  based_on: Option<usize>,
  paragraph: &'a [u8],
  characters: &'a [u8],
}

/// The sheet of styles.
pub(crate) struct Styles<'a> {
  styles: Vec<Option<Style<'a>>>,
  /// The font of text that names none, by its place in the table of fonts.
  pub default_font: usize,
}

impl<'a> Styles<'a> {
  /// The sheet: a header with its length before it, and then each style with its length
  /// before it, a length of nought for a number that has no style.
  pub fn read(sheet: Option<&'a [u8]>) -> Self {
    let mut found = Styles {
      styles: Vec::new(),
      default_font: 0,
    };
    let Some(sheet) = sheet else {
      return found;
    };
    let header = usize::from(u16_at(sheet, 0).unwrap_or(0));
    let (Some(count), Some(fixed)) = (u16_at(sheet, 2), u16_at(sheet, 4)) else {
      return found;
    };
    found.default_font = (header >= 14)
      .then(|| u16_at(sheet, 2 + 12))
      .flatten()
      .map_or(0, usize::from);
    let mut at = 2 + header;
    for _ in 0..count {
      let Some(length) = u16_at(sheet, at).map(usize::from) else {
        break;
      };
      let style = sheet
        .get(at + 2..at + 2 + length)
        .filter(|style| !style.is_empty())
        .and_then(|style| Styles::style(style, usize::from(fixed)));
      found.styles.push(style);
      at += 2 + length;
    }
    found
  }

  /// One style: a part of fixed length that says what kind it is and what it starts from, its
  /// name, and then its sets of properties, each with its length before it and each starting
  /// on an even byte.
  fn style(style: &'a [u8], fixed: usize) -> Option<Style<'a>> {
    let kind = u16_at(style, 2)?;
    let based_on = Some(usize::from(kind >> 4)).filter(|based_on| *based_on != 0x0FFF);
    let name = usize::from(u16_at(style, fixed)?);
    let even = |at: usize| at.next_multiple_of(2);
    let mut at = even(fixed + 2 + name * 2 + 2);
    let mut set = || {
      let length = usize::from(u16_at(style, at)?);
      let data = style.get(at + 2..at + 2 + length)?;
      at = even(at + 2 + length);
      Some(data)
    };
    let (paragraph, characters) = match kind & 0x000F {
      // A paragraph's style: the paragraph's properties after the style's own number, and
      // then its text's.
      1 => {
        let paragraph = set().and_then(|data| data.get(2..)).unwrap_or(&[]);
        (paragraph, set().unwrap_or(&[]))
      }
      // A style of characters alone.
      2 => (&[][..], set().unwrap_or(&[])),
      _ => (&[][..], &[][..]),
    };
    Some(Style {
      based_on,
      paragraph,
      characters,
    })
  }

  /// What the style numbered `style` states, with what each style it starts from states
  /// before it: of a paragraph, and of its text.
  pub fn chain(&self, style: usize) -> Vec<(&'a [u8], &'a [u8])> {
    let mut chain = Vec::new();
    let mut next = Some(style);
    // A style that starts from itself, or from one that starts from it, is followed once.
    while let Some(style) = next.and_then(|number| self.styles.get(number)?.as_ref()) {
      if chain.len() > self.styles.len() {
        break;
      }
      chain.push((style.paragraph, style.characters));
      next = style.based_on;
    }
    chain.reverse();
    chain
  }
}

/// The names of the fonts, in the order the file numbers them: a count, and then each font
/// with its length before it and its name at the fortieth byte.
pub(crate) fn fonts(table: Option<&[u8]>) -> Vec<String> {
  let Some(table) = table else {
    return Vec::new();
  };
  let count = usize::from(u16_at(table, 0).unwrap_or(0));
  let mut names = Vec::new();
  let mut at = 4;
  for _ in 0..count {
    let Some(font) = table
      .get(at)
      .and_then(|length| table.get(at + 1..at + 1 + usize::from(*length)))
    else {
      break;
    };
    let units: Vec<u16> = font
      .get(39..)
      .unwrap_or(&[])
      .as_chunks::<2>()
      .0
      .iter()
      .map(|pair| u16::from_le_bytes(*pair))
      .take_while(|unit| *unit != 0)
      .collect();
    names.push(String::from_utf16_lossy(&units));
    at += 1 + font.len();
  }
  names
}

/// The properties of the first section: the table of sections says where each one's are kept
/// in the main stream, with their length before them.
pub(crate) fn first_section<'a>(main: &'a [u8], sections: Option<&[u8]>) -> &'a [u8] {
  let found = (|| {
    let sections = sections?;
    let count = sections.len().checked_sub(4)? / 16;
    let at = usize::try_from(u32_at(sections, (count + 1) * 4 + 2)?).ok()?;
    let length = usize::from(u16_at(main, at)?);
    main.get(at + 2..at + 2 + length)
  })();
  found.unwrap_or(&[])
}
