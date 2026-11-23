//! Word documents for the tests, built by hand: the text in pieces, and what describes it.

/// Where the text starts in the main stream, after the block that says where everything is.
const TEXT_AT: usize = 0x0800;
const PAGE: usize = 512;

/// A property with a one-byte value.
pub(crate) fn byte(sprm: u16, value: u8) -> Vec<u8> {
  [&sprm.to_le_bytes()[..], &[value]].concat()
}

/// A property with a two-byte value.
pub(crate) fn word(sprm: u16, value: i16) -> Vec<u8> {
  [sprm.to_le_bytes(), value.to_le_bytes()].concat()
}

/// A property with a four-byte value.
pub(crate) fn long(sprm: u16, value: u32) -> Vec<u8> {
  [&sprm.to_le_bytes()[..], &value.to_le_bytes()].concat()
}

/// A table row's edges, in twentieths of a point: its left side, and the right side of each
/// of its cells.
pub(crate) fn row_edges(edges: &[i16]) -> Vec<u8> {
  let mut value = vec![u8::try_from(edges.len() - 1).unwrap()];
  value.extend(edges.iter().flat_map(|edge| edge.to_le_bytes()));
  // A description of each cell follows the edges; twenty bytes to a cell.
  value.resize(value.len() + 20 * (edges.len() - 1), 0);
  let mut out = 0xD608u16.to_le_bytes().to_vec();
  out.extend(u16::try_from(value.len() + 1).unwrap().to_le_bytes());
  out.extend(value);
  out
}

/// A style: its number is its place in the sheet.
pub(crate) struct Style {
  pub name: &'static str,
  /// The style this one starts from, or 0x0FFF for none.
  pub based_on: u16,
  pub paragraph: Vec<u8>,
  pub characters: Vec<u8>,
}

/// A document under construction.
#[derive(Default)]
pub(crate) struct DocSpec {
  /// The text in pieces: each is kept one byte to a character, or two.
  pieces: Vec<(String, bool)>,
  /// The style and the properties of the paragraph at each place, counting from 0.
  paragraphs: Vec<(usize, u16, Vec<u8>)>,
  /// Properties of the characters from one position to another.
  runs: Vec<(usize, usize, Vec<u8>)>,
  section: Option<Vec<u8>>,
  styles: Vec<Style>,
  fonts: Vec<&'static str>,
  /// What is set in the flags of the opening block besides which table stream is used.
  flags: u16,
  version: Option<(u16, u16)>,
  /// The text of the headers and footers, with where each one starts.
  running: Option<(String, Vec<u32>)>,
  /// The text of the text boxes, with where each one starts.
  boxes: Option<(String, Vec<u32>)>,
  shapes: Vec<Shape>,
}

/// A shape anchored in the text.
pub(crate) struct Shape {
  /// The place of the mark it is anchored at.
  pub at: u32,
  pub id: u32,
  /// Its left, top, right, and bottom, in twentieths of a point.
  pub edges: (i32, i32, i32, i32),
  /// What its edges are measured from: two bits across, and two bits down.
  pub flags: u16,
  /// The text box that holds its text, counting from 1.
  pub text_box: Option<u16>,
}

/// One record of a drawing: its version and instance, its kind, and its data.
pub(crate) fn drawn(version: u16, kind: u16, data: &[u8]) -> Vec<u8> {
  let mut out = version.to_le_bytes().to_vec();
  out.extend(kind.to_le_bytes());
  out.extend(u32::try_from(data.len()).unwrap().to_le_bytes());
  out.extend(data);
  out
}

impl DocSpec {
  pub fn new() -> Self {
    DocSpec::default()
  }

  /// A piece of text kept one byte to a character.
  pub fn text(mut self, text: &str) -> Self {
    self.pieces.push((text.to_string(), false));
    self
  }

  /// A piece of text kept two bytes to a character.
  pub fn wide(mut self, text: &str) -> Self {
    self.pieces.push((text.to_string(), true));
    self
  }

  /// The properties of the paragraph numbered `nth` from 0, in the Normal style.
  pub fn paragraph(self, nth: usize, properties: &[Vec<u8>]) -> Self {
    self.styled(nth, 0, properties)
  }

  /// The same, in the style numbered `style`.
  pub fn styled(mut self, nth: usize, style: u16, properties: &[Vec<u8>]) -> Self {
    self.paragraphs.push((nth, style, properties.concat()));
    self
  }

  /// The properties of the characters from `start` up to `end`.
  pub fn run(mut self, start: usize, end: usize, properties: &[Vec<u8>]) -> Self {
    self.runs.push((start, end, properties.concat()));
    self
  }

  pub fn section(mut self, properties: &[Vec<u8>]) -> Self {
    self.section = Some(properties.concat());
    self
  }

  pub fn style(mut self, style: Style) -> Self {
    self.styles.push(style);
    self
  }

  pub fn fonts(mut self, names: &[&'static str]) -> Self {
    self.fonts = names.to_vec();
    self
  }

  pub fn flags(mut self, flags: u16) -> Self {
    self.flags |= flags;
    self
  }

  /// The mark the file opens with and the version it states, where a test wants an older one.
  pub fn version(mut self, mark: u16, version: u16) -> Self {
    self.version = Some((mark, version));
    self
  }

  /// The text of the headers and footers, which follows the document's own, and the place
  /// each of them starts at within it.
  pub fn running(mut self, text: &str, starts: &[u32]) -> Self {
    self.running = Some((text.to_string(), starts.to_vec()));
    self
  }

  /// The text of the text boxes, which follows every other run of text, and the place each of
  /// them starts at within it.
  pub fn boxes(mut self, text: &str, starts: &[u32]) -> Self {
    self.boxes = Some((text.to_string(), starts.to_vec()));
    self
  }

  pub fn shape(mut self, shape: Shape) -> Self {
    self.shapes.push(shape);
    self
  }

  /// The drawing data: what the drawings share, and then the main text's drawing, with a
  /// group of shapes in it. Each shape states its number, and its text box where it has one.
  fn drawings(&self) -> Vec<u8> {
    let shapes: Vec<u8> = self
      .shapes
      .iter()
      .flat_map(|shape| {
        let mut stated = shape.id.to_le_bytes().to_vec();
        stated.extend(0u32.to_le_bytes());
        let mut held = drawn(0x0002, 0xF00A, &stated);
        if let Some(text_box) = shape.text_box {
          let mut property = 0x0080u16.to_le_bytes().to_vec();
          property.extend((u32::from(text_box) << 16).to_le_bytes());
          held.extend(drawn(0x0013, 0xF00B, &property));
        }
        drawn(0x000F, 0xF004, &held)
      })
      .collect();
    let group = drawn(0x000F, 0xF003, &shapes);
    let mut out = drawn(0x000F, 0xF000, &[]);
    out.push(0);
    out.extend(drawn(0x000F, 0xF002, &group));
    out
  }

  /// The sheet of styles: a header, and each style with its length before it.
  fn style_sheet(&self) -> Vec<u8> {
    let mut sheet = 18u16.to_le_bytes().to_vec();
    sheet.extend(u16::try_from(self.styles.len()).unwrap().to_le_bytes());
    sheet.extend(10u16.to_le_bytes());
    sheet.extend([0u8; 14]);
    for (number, style) in self.styles.iter().enumerate() {
      let even = |bytes: &mut Vec<u8>| bytes.resize(bytes.len().next_multiple_of(2), 0);
      // A paragraph style, with two sets of properties after its name.
      let mut body = 0u16.to_le_bytes().to_vec();
      body.extend((1u16 | style.based_on << 4).to_le_bytes());
      body.extend(2u16.to_le_bytes());
      body.extend([0u8; 4]);
      let name: Vec<u16> = style.name.encode_utf16().collect();
      body.extend(u16::try_from(name.len()).unwrap().to_le_bytes());
      body.extend(name.iter().flat_map(|unit| unit.to_le_bytes()));
      body.extend([0u8; 2]);
      even(&mut body);
      body.extend(
        u16::try_from(style.paragraph.len() + 2)
          .unwrap()
          .to_le_bytes(),
      );
      body.extend(u16::try_from(number).unwrap().to_le_bytes());
      body.extend(&style.paragraph);
      even(&mut body);
      body.extend(u16::try_from(style.characters.len()).unwrap().to_le_bytes());
      body.extend(&style.characters);
      even(&mut body);
      sheet.extend(u16::try_from(body.len()).unwrap().to_le_bytes());
      sheet.extend(body);
    }
    sheet
  }

  /// The table of fonts: a count, and each font with its length before it and its name at
  /// the fortieth byte.
  fn font_table(&self) -> Vec<u8> {
    let mut table = u16::try_from(self.fonts.len())
      .unwrap()
      .to_le_bytes()
      .to_vec();
    table.extend([0u8; 2]);
    for name in &self.fonts {
      let mut font = vec![0u8; 39];
      font.extend(
        name
          .encode_utf16()
          .chain([0])
          .flat_map(|unit| unit.to_le_bytes()),
      );
      table.push(u8::try_from(font.len()).unwrap());
      table.extend(font);
    }
    table
  }

  /// The two streams: the main one, and the table stream.
  pub fn streams(&self) -> (Vec<u8>, Vec<u8>) {
    let mut main = vec![0u8; TEXT_AT];
    // Where each character is kept, and where the text ends.
    let mut places: Vec<usize> = Vec::new();
    let mut pieces: Vec<(usize, usize, bool)> = Vec::new();
    let mut all = String::new();
    let after = [&self.running, &self.boxes];
    let after = after
      .into_iter()
      .flatten()
      .map(|(text, _)| (text.clone(), false));
    for (text, wide) in self.pieces.iter().cloned().chain(after) {
      let at = main.len();
      let count = text.chars().count();
      pieces.push((places.len(), at, wide));
      if wide {
        assert!(text.chars().all(|c| c.len_utf16() == 1));
        main.extend(text.encode_utf16().flat_map(|unit| unit.to_le_bytes()));
        places.extend((0..count).map(|nth| at + nth * 2));
      } else {
        let (bytes, _, lossy) = encoding_rs::WINDOWS_1252.encode(&text);
        assert!(!lossy && bytes.len() == count);
        main.extend(bytes.iter());
        places.extend((0..count).map(|nth| at + nth));
      }
      all.push_str(&text);
    }
    let total = places.len();
    places.push(main.len());
    let own: usize = self
      .pieces
      .iter()
      .map(|(text, _)| text.chars().count())
      .sum();
    main.resize(main.len().next_multiple_of(PAGE), 0);

    // A page of paragraph properties: where each paragraph starts and the last one ends, for
    // each a place on the page, and at those places the properties, from the end backward.
    let ends: Vec<usize> = all
      .chars()
      .enumerate()
      .filter(|(_, c)| matches!(c, '\r' | '\u{7}'))
      .map(|(at, _)| at + 1)
      .collect();
    let mut page = vec![0u8; PAGE];
    let mut free = PAGE - 1;
    let mut starts = vec![places[0]];
    let mut offsets = Vec::new();
    for (nth, end) in ends.iter().enumerate() {
      starts.push(places[*end]);
      let stated = self.paragraphs.iter().find(|(at, _, _)| *at == nth);
      let (style, properties) = stated.map_or((0, &[][..]), |(_, style, p)| (*style, &p[..]));
      let mut data = style.to_le_bytes().to_vec();
      data.extend(properties);
      let kept = if data.len() % 2 == 1 {
        [&[u8::try_from(data.len().div_ceil(2)).unwrap()][..], &data].concat()
      } else {
        [&[0, u8::try_from(data.len() / 2).unwrap()][..], &data].concat()
      };
      free = (free - kept.len()) & !1;
      page[free..free + kept.len()].copy_from_slice(&kept);
      offsets.push(u8::try_from(free / 2).unwrap());
    }
    let mut at = 0;
    for start in &starts {
      page[at..at + 4].copy_from_slice(&u32::try_from(*start).unwrap().to_le_bytes());
      at += 4;
    }
    for offset in &offsets {
      page[at] = *offset;
      at += 13;
    }
    assert!(
      at <= free,
      "the test document has too many paragraphs for one page"
    );
    page[PAGE - 1] = u8::try_from(ends.len()).unwrap();
    let paragraphs_at = main.len() / PAGE;
    main.extend(page);

    // A page of character properties, in the same way: runs with nothing stated between them.
    let mut runs = self.runs.clone();
    runs.sort_by_key(|(start, _, _)| *start);
    let mut page = vec![0u8; PAGE];
    let mut free = PAGE - 1;
    let mut starts = vec![places[0]];
    let mut offsets = Vec::new();
    let mut reached = 0;
    for (start, end, properties) in &runs {
      if *start > reached {
        starts.push(places[*start]);
        offsets.push(0);
      }
      let kept = [&[u8::try_from(properties.len()).unwrap()][..], properties].concat();
      free = (free - kept.len()) & !1;
      page[free..free + kept.len()].copy_from_slice(&kept);
      starts.push(places[*end]);
      offsets.push(u8::try_from(free / 2).unwrap());
      reached = *end;
    }
    if reached < total {
      starts.push(places[total]);
      offsets.push(0);
    }
    let mut at = 0;
    for start in &starts {
      page[at..at + 4].copy_from_slice(&u32::try_from(*start).unwrap().to_le_bytes());
      at += 4;
    }
    page[at..at + offsets.len()].copy_from_slice(&offsets);
    assert!(at + offsets.len() <= free);
    page[PAGE - 1] = u8::try_from(offsets.len()).unwrap();
    let characters_at = main.len() / PAGE;
    main.extend(page);

    let section_at = main.len();
    if let Some(section) = &self.section {
      main.extend(u16::try_from(section.len()).unwrap().to_le_bytes());
      main.extend(section);
    }

    // The table stream, and where each of its parts is.
    let mut table = Vec::new();
    let mut parts: Vec<(usize, usize, usize)> = Vec::new();
    let mut part = |index: usize, bytes: Vec<u8>, table: &mut Vec<u8>| {
      parts.push((index, table.len(), bytes.len()));
      table.extend(bytes);
    };
    let u32s = |values: &[usize]| -> Vec<u8> {
      values
        .iter()
        .flat_map(|value| u32::try_from(*value).unwrap().to_le_bytes())
        .collect()
    };
    if !self.styles.is_empty() {
      part(1, self.style_sheet(), &mut table);
    }
    // The pieces: where each starts among the characters and the text ends, and for each
    // where it is kept and how.
    let mut starts: Vec<usize> = pieces.iter().map(|(start, _, _)| *start).collect();
    starts.push(total);
    let mut plc = u32s(&starts);
    for (_, at, wide) in &pieces {
      let kept = if *wide { *at } else { (*at * 2) | 0x4000_0000 };
      plc.extend([0u8; 2]);
      plc.extend(u32::try_from(kept).unwrap().to_le_bytes());
      plc.extend([0u8; 2]);
    }
    let mut clx = vec![0x02];
    clx.extend(u32::try_from(plc.len()).unwrap().to_le_bytes());
    clx.extend(plc);
    part(33, clx, &mut table);
    part(
      13,
      u32s(&[places[0], places[total], paragraphs_at]),
      &mut table,
    );
    part(
      12,
      u32s(&[places[0], places[total], characters_at]),
      &mut table,
    );
    if self.section.is_some() {
      let mut sections = u32s(&[0, own]);
      sections.extend([0u8; 2]);
      sections.extend(u32::try_from(section_at).unwrap().to_le_bytes());
      sections.extend([0u8; 2]);
      sections.extend(0xFFFF_FFFFu32.to_le_bytes());
      part(6, sections, &mut table);
    }
    if !self.fonts.is_empty() {
      part(15, self.font_table(), &mut table);
    }
    if let Some((_, starts)) = &self.running {
      let starts: Vec<u8> = starts
        .iter()
        .flat_map(|start| start.to_le_bytes())
        .collect();
      part(11, starts, &mut table);
    }
    if let Some((_, starts)) = &self.boxes {
      // Where each box starts and the last ends, and 22 bytes about each box.
      let mut listed: Vec<u8> = starts
        .iter()
        .flat_map(|start| start.to_le_bytes())
        .collect();
      listed.resize(listed.len() + 22 * (starts.len() - 1), 0);
      part(56, listed, &mut table);
    }
    if !self.shapes.is_empty() {
      // Where each shape is anchored and the text ends, and 26 bytes about each shape.
      let mut anchors: Vec<u8> = self
        .shapes
        .iter()
        .flat_map(|shape| shape.at.to_le_bytes())
        .collect();
      anchors.extend(u32::try_from(own).unwrap().to_le_bytes());
      for shape in &self.shapes {
        let (left, top, right, bottom) = shape.edges;
        anchors.extend(shape.id.to_le_bytes());
        for edge in [left, top, right, bottom] {
          anchors.extend(edge.to_le_bytes());
        }
        anchors.extend(shape.flags.to_le_bytes());
        anchors.extend([0u8; 4]);
      }
      part(40, anchors, &mut table);
      part(50, self.drawings(), &mut table);
    }

    let (mark, version) = self.version.unwrap_or((0xA5EC, 0x00C1));
    main[0..2].copy_from_slice(&mark.to_le_bytes());
    main[2..4].copy_from_slice(&version.to_le_bytes());
    main[0x0A..0x0C].copy_from_slice(&(0x0200 | self.flags).to_le_bytes());
    main[0x20..0x22].copy_from_slice(&0x000Eu16.to_le_bytes());
    main[0x3E..0x40].copy_from_slice(&0x0016u16.to_le_bytes());
    main[0x4C..0x50].copy_from_slice(&u32::try_from(own).unwrap().to_le_bytes());
    let count = |story: &Option<(String, Vec<u32>)>| {
      let count = story.as_ref().map_or(0, |(text, _)| text.chars().count());
      u32::try_from(count).unwrap().to_le_bytes()
    };
    main[0x54..0x58].copy_from_slice(&count(&self.running));
    main[0x64..0x68].copy_from_slice(&count(&self.boxes));
    main[0x98..0x9A].copy_from_slice(&0x005Du16.to_le_bytes());
    for (index, at, length) in parts {
      let slot = 0x9A + index * 8;
      main[slot..slot + 4].copy_from_slice(&u32::try_from(at).unwrap().to_le_bytes());
      main[slot + 4..slot + 8].copy_from_slice(&u32::try_from(length).unwrap().to_le_bytes());
    }
    (main, table)
  }

  /// The file: a compound file of the two streams.
  pub fn file(&self) -> Vec<u8> {
    let (main, table) = self.streams();
    cfbdown::compound(&[("WordDocument", &main), ("1Table", &table)]).unwrap()
  }
}
