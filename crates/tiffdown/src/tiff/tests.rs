use super::*;
use crate::testing::{Value, sized, tiff};

#[test]
fn bytes_that_are_not_a_tiff_are_refused() {
  let others: [&[u8]; 5] = [
    b"",
    b"II",
    b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n",
    // The two letters of a TIFF and another format's number after them.
    b"II\x2C\0\x08\0\0\0",
    b"MM\0\x2A",
  ];
  for other in others {
    let refused = Tiff::open(other.to_vec()).unwrap_err();
    assert_eq!(
      refused.to_string(),
      "unsupported format: the file does not open as a TIFF does",
      "{other:?}"
    );
  }
}

#[test]
fn a_bigtiff_is_named_and_refused() {
  let big = b"II\x2B\0\x08\0\0\0\x10\0\0\0\0\0\0\0".to_vec();
  assert_eq!(
    Tiff::open(big).unwrap_err().to_string(),
    "unsupported format: the file is a BigTIFF, which is not read yet"
  );
}

#[test]
fn a_file_of_one_page_says_its_size_in_either_byte_order() {
  for little in [true, false] {
    let file = tiff(little, &[sized(2560, 3296)]);
    let size = u64::try_from(file.len()).unwrap();
    let opened = Tiff::open(file).unwrap();
    assert_eq!(opened.byte_size(), size);
    assert_eq!(opened.warnings(), Vec::<String>::new());
    let [page] = opened.pages() else {
      panic!("{} pages", opened.pages().len());
    };
    assert_eq!((page.number, page.width, page.height), (1, 2560, 3296));
  }
}

#[test]
fn each_directory_of_the_chain_is_a_page_in_the_order_of_the_chain() {
  let file = tiff(
    true,
    &[sized(2560, 3296), sized(1700, 2200), sized(3296, 2560)],
  );
  let opened = Tiff::open(file).unwrap();
  let pages: Vec<(u32, u32, u32)> = opened
    .pages()
    .iter()
    .map(|page| (page.number, page.width, page.height))
    .collect();
  assert_eq!(pages, [(1, 2560, 3296), (2, 1700, 2200), (3, 3296, 2560)]);
}

/// The one page of a file whose only directory says `more` beside its size.
fn page_saying(more: Vec<(u16, Value)>) -> Page {
  let mut entries = sized(1700, 2200);
  entries.extend(more);
  let opened = Tiff::open(tiff(true, &[entries])).unwrap();
  let [page] = opened.pages() else {
    panic!("{} pages", opened.pages().len());
  };
  page.clone()
}

#[test]
fn a_page_says_how_its_samples_are_coded() {
  // A faxed page: one bit to a sample, coded as a fax machine codes it, with zero for white.
  let fax = page_saying(vec![
    (258, Value::Short(vec![1])),
    (259, Value::Short(vec![4])),
    (262, Value::Short(vec![0])),
    (277, Value::Short(vec![1])),
  ]);
  assert_eq!(
    (fax.bits_per_sample.as_slice(), fax.samples_per_pixel),
    (&[1][..], 1)
  );
  assert_eq!(fax.compression, Compression::Group4);
  assert_eq!(fax.photometric, Some(Photometric::WhiteIsZero));
  // A scan in color: three samples of eight bits.
  let scan = page_saying(vec![
    (258, Value::Short(vec![8, 8, 8])),
    (259, Value::Short(vec![5])),
    (262, Value::Short(vec![2])),
    (277, Value::Short(vec![3])),
  ]);
  assert_eq!(
    (scan.bits_per_sample.as_slice(), scan.samples_per_pixel),
    (&[8, 8, 8][..], 3)
  );
  assert_eq!(scan.compression, Compression::Lzw);
  assert_eq!(scan.photometric, Some(Photometric::Rgb));
}

#[test]
fn every_coding_the_format_numbers_has_a_name_and_any_other_keeps_its_number() {
  let codings = [
    (1, Compression::None),
    (2, Compression::ModifiedHuffman),
    (3, Compression::Group3),
    (4, Compression::Group4),
    (5, Compression::Lzw),
    (6, Compression::OldJpeg),
    (7, Compression::Jpeg),
    (8, Compression::Deflate),
    (32946, Compression::Deflate),
    (32773, Compression::PackBits),
    (34712, Compression::Other(34712)),
  ];
  for (number, named) in codings {
    let page = page_saying(vec![(259, Value::Short(vec![number]))]);
    assert_eq!(page.compression, named, "{number}");
  }
  let meanings = [
    (0, Photometric::WhiteIsZero),
    (1, Photometric::BlackIsZero),
    (2, Photometric::Rgb),
    (3, Photometric::Palette),
    (4, Photometric::Mask),
    (5, Photometric::Cmyk),
    (6, Photometric::YCbCr),
    (8, Photometric::CieLab),
    (32803, Photometric::Other(32803)),
  ];
  for (number, named) in meanings {
    let page = page_saying(vec![(262, Value::Short(vec![number]))]);
    assert_eq!(page.photometric, Some(named), "{number}");
  }
}

#[test]
fn what_a_directory_leaves_out_is_what_the_format_says_it_is() {
  let page = page_saying(Vec::new());
  assert_eq!(
    (page.bits_per_sample.as_slice(), page.samples_per_pixel),
    (&[1][..], 1)
  );
  assert_eq!(page.compression, Compression::None);
  assert_eq!(page.orientation, 1);
  // What the samples mean has no default, and neither has a resolution.
  assert_eq!(page.photometric, None);
  assert_eq!(page.resolution, None);
}

#[test]
fn resolution_is_pixels_to_the_inch_whatever_unit_the_file_states_it_in() {
  let stated = |across: (u32, u32), down: (u32, u32), unit: Option<u16>| {
    let mut more = vec![
      (282, Value::Fractions(vec![across])),
      (283, Value::Fractions(vec![down])),
    ];
    more.extend(unit.map(|unit| (296, Value::Short(vec![unit]))));
    page_saying(more).resolution
  };
  let square = |side: f64| Some(Resolution { x: side, y: side });
  assert_eq!(stated((300, 1), (300, 1), Some(2)), square(300.0));
  // The inch is the unit when none is named.
  assert_eq!(stated((300, 1), (300, 1), None), square(300.0));
  assert_eq!(stated((3000, 10), (3000, 10), Some(2)), square(300.0));
  // To the centimeter.
  assert_eq!(stated((100, 1), (100, 1), Some(3)), square(254.0));
  // A fax at its standard resolution: its pixels are not square.
  assert_eq!(
    stated((204, 1), (98, 1), Some(2)),
    Some(Resolution { x: 204.0, y: 98.0 })
  );
  // In no unit, a resolution says only the shape of a pixel; and nothing over nothing is no number.
  assert_eq!(stated((300, 1), (300, 1), Some(1)), None);
  assert_eq!(stated((300, 0), (300, 1), Some(2)), None);
  assert_eq!(stated((0, 1), (0, 1), Some(2)), None);
}

#[test]
fn how_the_page_is_to_be_turned_is_a_number_from_one_to_eight() {
  assert_eq!(
    page_saying(vec![(274, Value::Short(vec![6]))]).orientation,
    6
  );
  assert_eq!(
    page_saying(vec![(274, Value::Short(vec![8]))]).orientation,
    8
  );
  // A number the format does not have is read as the usual one.
  assert_eq!(
    page_saying(vec![(274, Value::Short(vec![0]))]).orientation,
    1
  );
  assert_eq!(
    page_saying(vec![(274, Value::Short(vec![9]))]).orientation,
    1
  );
}

#[test]
fn a_smaller_copy_of_a_page_and_a_mask_for_one_are_not_pages() {
  let marked = |width: u32, tag: u16, value: Value| {
    let mut entries = sized(width, 100);
    entries.push((tag, value));
    entries
  };
  let file = tiff(
    true,
    &[
      // One page of a document of several, which is a page.
      marked(1000, 254, Value::Long(vec![2])),
      // A reduced copy of it, in the new marking and in the old.
      marked(250, 254, Value::Long(vec![1])),
      marked(125, 255, Value::Short(vec![2])),
      marked(2000, 254, Value::Long(vec![2])),
      // A mask for a page.
      marked(2000, 254, Value::Long(vec![4])),
      sized(3000, 100),
    ],
  );
  let opened = Tiff::open(file).unwrap();
  let pages: Vec<(u32, u32)> = opened
    .pages()
    .iter()
    .map(|page| (page.number, page.width))
    .collect();
  assert_eq!(pages, [(1, 1000), (2, 2000), (3, 3000)]);
}

/// Where the directory at `at` says the next one is, in a file whose numbers run low byte first.
fn next_pointer(file: &[u8], at: usize) -> usize {
  let entries = usize::from(u16::from_le_bytes([file[at], file[at + 1]]));
  at + 2 + 12 * entries
}

#[test]
fn a_chain_that_leads_back_on_itself_is_followed_once() {
  let back = ["its directories lead back to one already read, where the list of its pages stops"];
  // The only directory names itself as the next.
  let mut file = tiff(true, &[sized(640, 480)]);
  let pointer = next_pointer(&file, 8);
  file[pointer..pointer + 4].copy_from_slice(&8u32.to_le_bytes());
  let opened = Tiff::open(file).unwrap();
  assert_eq!(opened.pages().len(), 1);
  assert_eq!(opened.warnings(), back);
  // The second of two names the first.
  let mut file = tiff(true, &[sized(640, 480), sized(320, 240)]);
  let second = usize::try_from(u32::from_le_bytes(
    *file[next_pointer(&file, 8)..].first_chunk().unwrap(),
  ))
  .unwrap();
  let pointer = next_pointer(&file, second);
  file[pointer..pointer + 4].copy_from_slice(&8u32.to_le_bytes());
  let opened = Tiff::open(file).unwrap();
  assert_eq!(opened.pages().len(), 2);
  assert_eq!(opened.warnings(), back);
}

#[test]
fn a_file_cut_off_inside_a_later_directory_keeps_the_pages_before_it_and_says_so() {
  let file = tiff(true, &[sized(640, 480), sized(320, 240), sized(160, 120)]);
  let cut = file.len() - 10;
  let opened = Tiff::open(file[..cut].to_vec()).unwrap();
  let widths: Vec<u32> = opened.pages().iter().map(|page| page.width).collect();
  assert_eq!(widths, [640, 320]);
  assert_eq!(
    opened.warnings(),
    ["a directory lies past the end of the file, so the pages listed may not be all of them"]
  );
}

#[test]
fn a_file_whose_first_directory_is_not_there_is_malformed() {
  let whole = tiff(true, &[sized(640, 480)]);
  let mut far = whole.clone();
  far[4..8].copy_from_slice(&1_000_000u32.to_le_bytes());
  let mut none = whole.clone();
  none[4..8].copy_from_slice(&0u32.to_le_bytes());
  for file in [far, none, whole[..20].to_vec(), whole[..8].to_vec()] {
    assert_eq!(
      Tiff::open(file).unwrap_err().to_string(),
      "malformed file: the first directory does not lie within the file"
    );
  }
}

#[test]
fn a_directory_that_states_no_size_is_not_a_page() {
  let no_size = vec![(259, Value::Short(vec![4]))];
  let no_height = vec![(256, Value::Long(vec![640]))];
  let opened = Tiff::open(tiff(
    true,
    &[sized(640, 480), no_size, no_height, sized(320, 240)],
  ))
  .unwrap();
  let pages: Vec<(u32, u32)> = opened
    .pages()
    .iter()
    .map(|page| (page.number, page.width))
    .collect();
  assert_eq!(pages, [(1, 640), (2, 320)]);
  // Said once, however many directories it is true of.
  assert_eq!(
    opened.warnings(),
    ["a directory states no size and is not listed as a page"]
  );
}

#[test]
fn no_more_directories_are_followed_than_a_file_could_sensibly_have() {
  // Seventy thousand directories with nothing in them, each six bytes: none is a page.
  let mut file = b"II\x2A\0\x08\0\0\0".to_vec();
  for index in 0..70_000u32 {
    file.extend([0, 0]);
    file.extend((8 + 6 * (index + 1)).to_le_bytes());
  }
  file.extend([0; 6]);
  let opened = Tiff::open(file).unwrap();
  assert_eq!(opened.pages().len(), 0);
  assert_eq!(
    opened.warnings(),
    [
      "a directory states no size and is not listed as a page",
      "the file has more directories than are followed, so the pages listed may not be all of them"
    ]
  );
}

#[test]
fn a_page_says_what_wrote_it_and_when() {
  let page = page_saying(vec![
    (305, Value::Text("A Scanner's Program 4.2")),
    (306, Value::Text("2015:03:04 10:22:33")),
  ]);
  assert_eq!(page.software.as_deref(), Some("A Scanner's Program 4.2"));
  // As the file writes it, in the usual shape for a date. The file names no time zone, and
  // none is made up for it.
  assert_eq!(page.date_time.as_deref(), Some("2015-03-04T10:22:33"));
  // A page that says neither.
  let silent = page_saying(Vec::new());
  assert_eq!((silent.software, silent.date_time), (None, None));
  // A machine that did not know the time, and one that wrote nothing in the space for it.
  assert_eq!(
    page_saying(vec![(306, Value::Text("0000:00:00 00:00:00"))]).date_time,
    None
  );
  assert_eq!(page_saying(vec![(305, Value::Text("   "))]).software, None);
  // A date in a shape of the writer's own is kept as it stands.
  assert_eq!(
    page_saying(vec![(306, Value::Text("03/04/2015 10:22"))])
      .date_time
      .as_deref(),
    Some("03/04/2015 10:22")
  );
}

/// A page of this many pixels at this resolution.
fn scanned(width: u32, height: u32, across: u32, down: u32) -> Page {
  let mut entries = sized(width, height);
  entries.push((282, Value::Fractions(vec![(across, 1)])));
  entries.push((283, Value::Fractions(vec![(down, 1)])));
  let opened = Tiff::open(tiff(true, &[entries])).unwrap();
  opened.pages()[0].clone()
}

#[test]
fn a_page_s_size_on_paper_comes_from_its_pixels_and_its_resolution() {
  use contentdown::PaperSize;
  let letter = scanned(2550, 3300, 300, 300);
  assert_eq!(letter.size_in_points(), Some((612.0, 792.0)));
  assert_eq!(letter.paper_size(), Some(PaperSize::Letter));
  // What a fax machine makes of a letter page is a little off the exact size, and is the same paper.
  assert_eq!(
    scanned(2560, 3296, 300, 300).paper_size(),
    Some(PaperSize::Letter)
  );
  assert_eq!(
    scanned(2480, 3508, 300, 300).paper_size(),
    Some(PaperSize::A4)
  );
  // Pixels that are not square: each side is measured by its own resolution.
  let fax = scanned(1728, 1078, 204, 98);
  let (width, height) = fax.size_in_points().unwrap();
  assert_eq!((width.round(), height.round()), (610.0, 792.0));
  assert_eq!(fax.paper_size(), Some(PaperSize::Letter));
  // A size no paper has is said to be none of them, and a page with no resolution has no
  // size on paper at all.
  assert_eq!(
    scanned(1000, 1000, 300, 300).paper_size(),
    Some(PaperSize::Unknown)
  );
  let unmeasured = page_saying(Vec::new());
  assert_eq!(
    (unmeasured.size_in_points(), unmeasured.paper_size()),
    (None, None)
  );
}

#[test]
fn a_file_made_to_be_slow_to_read_is_not_read_to_its_end() {
  // A megabyte made to cost a minute: a thousand directories, each claiming 65,535 entries,
  // laid over one another four bytes apart. Every count reads as all ones, and where each
  // directory would end stands the place of the next.
  const DIRECTORIES: u32 = 1_000;
  const CLAIMED: u32 = 65_535;
  let ends = 8 + 2 + 12 * CLAIMED;
  let mut file = vec![0xFF; usize::try_from(ends + 4 * DIRECTORIES + 4).unwrap()];
  file[..8].copy_from_slice(b"II\x2A\0\x08\0\0\0");
  for index in 0..DIRECTORIES {
    let next = if index + 1 == DIRECTORIES {
      0
    } else {
      8 + 4 * (index + 1)
    };
    let pointer = usize::try_from(ends + 4 * index).unwrap();
    file[pointer..pointer + 4].copy_from_slice(&next.to_le_bytes());
  }
  let opened = Tiff::open(file).unwrap();
  assert_eq!(opened.pages().len(), 0);
  // No real file has a millionth of these entries to a page. The reading stops, and says so.
  assert_eq!(
    opened.warnings(),
    [
      "a directory states no size and is not listed as a page",
      "the file has more directories than are followed, so the pages listed may not be all of them"
    ]
  );
}
