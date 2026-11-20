use image::DynamicImage;

use crate::testing::{Value, tiff};
use crate::{Tiff, TiffError};

/// A page kept as it is, with no compression: its size, what its samples are, and its data in
/// strips of `rows` rows each.
fn kept(
  (width, height): (u32, u32),
  bits: Vec<u16>,
  photometric: u16,
  rows: u32,
  strips: Vec<Vec<u8>>,
) -> Vec<(u16, Value)> {
  let samples = u16::try_from(bits.len()).unwrap();
  vec![
    (256, Value::Long(vec![width])),
    (257, Value::Long(vec![height])),
    (258, Value::Short(bits)),
    (259, Value::Short(vec![1])),
    (262, Value::Short(vec![photometric])),
    (273, Value::Pieces(strips)),
    (277, Value::Short(vec![samples])),
    (278, Value::Long(vec![rows])),
  ]
}

/// The pixels of the first page of a file of one directory.
fn first_page(entries: Vec<(u16, Value)>) -> Result<DynamicImage, TiffError> {
  Tiff::open(tiff(true, &[entries])).unwrap().raster(1)
}

/// The gray of each pixel of a page, a row at a time.
fn gray(image: &DynamicImage) -> Vec<Vec<u8>> {
  let DynamicImage::ImageLuma8(pixels) = image else {
    panic!("not gray: {:?}", image.color());
  };
  pixels
    .rows()
    .map(|row| row.map(|pixel| pixel.0[0]).collect())
    .collect()
}

#[test]
fn a_page_of_gray_kept_as_it_is_is_the_bytes_it_keeps() {
  let page = kept(
    (4, 2),
    vec![8],
    1,
    2,
    vec![vec![0, 64, 128, 255, 10, 20, 30, 40]],
  );
  let image = first_page(page).unwrap();
  assert_eq!(gray(&image), [[0, 64, 128, 255], [10, 20, 30, 40]]);
}

#[test]
fn a_page_whose_zero_is_white_is_turned_over() {
  let page = kept((3, 1), vec![8], 0, 1, vec![vec![0, 100, 255]]);
  assert_eq!(gray(&first_page(page).unwrap()), [[255, 155, 0]]);
}

#[test]
fn strips_are_put_together_in_the_order_the_page_lists_them() {
  // Three rows in strips of two: the last strip holds the one row that is left.
  let strips = vec![vec![1, 2, 3, 4], vec![5, 6]];
  let image = first_page(kept((2, 3), vec![8], 1, 2, strips)).unwrap();
  assert_eq!(gray(&image), [[1, 2], [3, 4], [5, 6]]);
}

#[test]
fn a_page_in_color_keeps_its_three_samples() {
  let page = kept(
    (2, 1),
    vec![8, 8, 8],
    2,
    1,
    vec![vec![255, 0, 0, 0, 128, 255]],
  );
  let DynamicImage::ImageRgb8(pixels) = first_page(page).unwrap() else {
    panic!("not color");
  };
  assert_eq!(pixels.into_raw(), [255, 0, 0, 0, 128, 255]);
}

#[test]
fn a_fourth_sample_that_says_how_much_shows_through_is_kept_and_any_other_is_dropped() {
  let with = |extra: u16| {
    let mut page = kept((1, 1), vec![8, 8, 8, 8], 2, 1, vec![vec![10, 20, 30, 128]]);
    page.push((338, Value::Short(vec![extra])));
    first_page(page).unwrap()
  };
  // 2 is how much of the pixel shows; 0 is a sample of no stated meaning.
  let DynamicImage::ImageRgba8(pixels) = with(2) else {
    panic!("not color with alpha");
  };
  assert_eq!(pixels.into_raw(), [10, 20, 30, 128]);
  let DynamicImage::ImageRgb8(pixels) = with(0) else {
    panic!("not color");
  };
  assert_eq!(pixels.into_raw(), [10, 20, 30]);
}

#[test]
fn a_page_of_one_bit_a_pixel_is_black_and_white() {
  // Ten pixels a row: a row is two bytes, and the six bits left over in the second are not
  // pixels. Zero is white.
  let rows = vec![vec![0b1010_0000, 0b0100_0000, 0b0000_0000, 0b1100_0000]];
  let image = first_page(kept((10, 2), vec![1], 0, 2, rows.clone())).unwrap();
  assert_eq!(
    gray(&image),
    [
      [0, 255, 0, 255, 255, 255, 255, 255, 255, 0],
      [255, 255, 255, 255, 255, 255, 255, 255, 0, 0]
    ]
  );
  // The same bytes with zero for black.
  let image = first_page(kept((10, 2), vec![1], 1, 2, rows)).unwrap();
  assert_eq!(gray(&image)[0], [255, 0, 255, 0, 0, 0, 0, 0, 0, 255]);
}

#[test]
fn bits_written_low_end_first_are_read_low_end_first() {
  // The fill order says which end of a byte holds the first pixel: 2 is the low end.
  let mut page = kept((8, 1), vec![1], 0, 1, vec![vec![0b0000_0101]]);
  page.push((266, Value::Short(vec![2])));
  assert_eq!(
    gray(&first_page(page).unwrap()),
    [[0, 255, 0, 255, 255, 255, 255, 255]]
  );
}

#[test]
fn a_page_that_picks_its_colors_from_a_table_is_given_in_color() {
  // Two bits would do for four colors; this page spends eight. The table is every red, then
  // every green, then every blue, each a number of sixteen bits.
  let mut table = vec![0u16; 3 * 256];
  (table[1], table[256 + 1], table[512 + 1]) = (0xFFFF, 0x8080, 0);
  (table[2], table[256 + 2], table[512 + 2]) = (0, 0, 0xFFFF);
  let mut page = kept((3, 1), vec![8], 3, 1, vec![vec![1, 2, 0]]);
  page.push((320, Value::Short(table)));
  let DynamicImage::ImageRgb8(pixels) = first_page(page).unwrap() else {
    panic!("not color");
  };
  assert_eq!(pixels.into_raw(), [255, 128, 0, 0, 0, 255, 0, 0, 0]);
}

#[test]
fn a_page_the_file_does_not_have_cannot_be_asked_for() {
  let opened = Tiff::open(tiff(true, &[kept((1, 1), vec![8], 1, 1, vec![vec![7]])])).unwrap();
  for number in [0, 2, 99] {
    assert_eq!(
      opened.raster(number).unwrap_err().to_string(),
      format!("limit exceeded: page {number} was asked for, and the file has 1 page")
    );
  }
}

#[test]
fn a_page_whose_data_is_not_all_there_is_malformed() {
  // A strip of fewer bytes than its rows need.
  let short = kept((4, 2), vec![8], 1, 2, vec![vec![1, 2, 3, 4, 5]]);
  assert_eq!(
    first_page(short).unwrap_err().to_string(),
    "malformed file: strip 1 of page 1 holds fewer bytes than its rows need"
  );
  // A strip the file ends before.
  let mut file = tiff(true, &[kept((4, 2), vec![8], 1, 2, vec![vec![9; 8]])]);
  file.truncate(file.len() - 3);
  assert_eq!(
    Tiff::open(file).unwrap().raster(1).unwrap_err().to_string(),
    "malformed file: strip 1 of page 1 does not lie within the file"
  );
  // Strips that stop a row short of the page.
  let fewer = kept((2, 3), vec![8], 1, 2, vec![vec![1, 2, 3, 4]]);
  assert_eq!(
    first_page(fewer).unwrap_err().to_string(),
    "malformed file: the strips of page 1 hold 2 of its 3 rows"
  );
  // A page that lists no data at all.
  let mut none = kept((4, 2), vec![8], 1, 2, vec![vec![9; 8]]);
  none.retain(|(tag, _)| *tag != 273);
  assert_eq!(
    first_page(none).unwrap_err().to_string(),
    "malformed file: page 1 does not say where its pixels are"
  );
}

#[test]
fn a_page_kept_in_a_way_that_is_not_read_says_which() {
  let coded = |coding: u16| {
    let mut page = kept((4, 2), vec![8], 1, 2, vec![vec![9; 8]]);
    page.retain(|(tag, _)| *tag != 259);
    page.push((259, Value::Short(vec![coding])));
    first_page(page).unwrap_err().to_string()
  };
  assert_eq!(
    coded(6),
    "unsupported format: page 1 is kept as old-style JPEG, which is not read"
  );
  assert_eq!(
    coded(34712),
    "unsupported format: page 1 is kept in coding 34712, which is not read"
  );
  // A page kept in tiles lists where its tiles are and has no strips.
  let mut tiled = kept((4, 2), vec![8], 1, 2, vec![vec![9; 8]]);
  tiled.retain(|(tag, _)| *tag != 273 && *tag != 278);
  tiled.extend([
    (322, Value::Long(vec![16])),
    (323, Value::Long(vec![16])),
    (324, Value::Pieces(vec![vec![9; 256]])),
  ]);
  assert_eq!(
    first_page(tiled).unwrap_err().to_string(),
    "unsupported format: page 1 is kept in tiles, which are not read yet"
  );
  let deep = kept((1, 1), vec![16], 1, 1, vec![vec![0, 0]]);
  assert_eq!(
    first_page(deep).unwrap_err().to_string(),
    "unsupported format: page 1 has samples of 16 bits, which are not read yet"
  );
}

#[test]
fn a_page_too_large_to_hold_is_refused_before_anything_is_made_for_it() {
  let vast = kept((60_000, 60_000), vec![8], 1, 60_000, vec![vec![0; 16]]);
  assert_eq!(
    first_page(vast).unwrap_err().to_string(),
    "limit exceeded: page 1 is 60000 by 60000 pixels, more than are read"
  );
}

/// `page` kept in the coding of this number in place of none.
fn coded_as(coding: u16, mut page: Vec<(u16, Value)>) -> Vec<(u16, Value)> {
  page.retain(|(tag, _)| *tag != 259);
  page.push((259, Value::Short(vec![coding])));
  page
}

/// Bytes packed as the format packs them with LZW.
fn lzw(bytes: &[u8]) -> Vec<u8> {
  weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
    .encode(bytes)
    .unwrap()
}

#[test]
fn a_page_packed_with_lzw_comes_out_as_it_went_in() {
  let rows: Vec<u8> = (0..=255).cycle().take(64 * 5).collect();
  // Each strip is packed on its own: two strips of two rows and one of the row left.
  let strips = vec![lzw(&rows[..128]), lzw(&rows[128..256]), lzw(&rows[256..])];
  let image = first_page(coded_as(5, kept((64, 5), vec![8], 1, 2, strips))).unwrap();
  assert_eq!(gray(&image).concat(), rows);
}

#[test]
fn samples_kept_as_differences_from_their_neighbors_are_summed_back() {
  // Three pixels of color. Each sample after the first pixel is kept as what it adds to the
  // same sample of the pixel before it, and adding wraps round past 255.
  let differences = [10, 20, 30, 2, 2, 3, 3, 254, 253];
  let mut page = coded_as(
    5,
    kept((3, 1), vec![8, 8, 8], 2, 1, vec![lzw(&differences)]),
  );
  page.push((317, Value::Short(vec![2])));
  let DynamicImage::ImageRgb8(pixels) = first_page(page).unwrap() else {
    panic!("not color");
  };
  assert_eq!(pixels.into_raw(), [10, 20, 30, 12, 22, 33, 15, 20, 30]);
}

#[test]
fn packed_data_that_does_not_come_out_whole_is_malformed() {
  // Half of what was packed, and bytes that were never packed at all.
  let whole = lzw(&[7; 4000]);
  for data in [whole[..whole.len() / 2].to_vec(), vec![0xFF; 40]] {
    let page = coded_as(5, kept((40, 100), vec![8], 1, 100, vec![data]));
    assert_eq!(
      first_page(page).unwrap_err().to_string(),
      "malformed file: strip 1 of page 1 does not unpack to the rows it should hold"
    );
  }
}

/// A faxed page: one bit a pixel, coded as a fax machine's Group 4 codes it.
fn faxed(size: (u32, u32), photometric: u16, rows: u32, strips: Vec<Vec<u8>>) -> Vec<(u16, Value)> {
  coded_as(4, kept(size, vec![1], photometric, rows, strips))
}

/// Two rows of eight pixels, each four white and then four black, as Group 4 codes them. The
/// first row is coded on its own: `001`, then a run of four white (`1011`) and one of four
/// black (`011`). The second is the same as the row above it, which is one bit for each of
/// its two changes of color: `11`. Twelve bits, and four to fill the last byte.
const HALF_AND_HALF: [u8; 2] = [0b0011_0110, 0b1111_0000];

#[test]
fn a_faxed_page_is_decoded_from_its_runs_of_white_and_black() {
  let image = first_page(faxed((8, 2), 0, 2, vec![HALF_AND_HALF.to_vec()])).unwrap();
  let row = [255, 255, 255, 255, 0, 0, 0, 0];
  assert_eq!(gray(&image), [row, row]);
  // A row that is white throughout is the same as the white row imagined above the first:
  // one bit a row.
  let blank = first_page(faxed((8, 2), 0, 2, vec![vec![0b1100_0000]])).unwrap();
  assert_eq!(gray(&blank), [[255; 8], [255; 8]]);
}

#[test]
fn a_faxed_page_whose_zero_is_black_is_the_other_way_about() {
  let image = first_page(faxed((8, 2), 1, 2, vec![HALF_AND_HALF.to_vec()])).unwrap();
  assert_eq!(gray(&image)[0], [0, 0, 0, 0, 255, 255, 255, 255]);
}

#[test]
fn each_strip_of_a_faxed_page_starts_from_a_white_row_of_its_own() {
  // The same two rows as one strip of two and as two strips of one. A strip's first row is
  // coded against white, not against the row that ended the strip before it.
  let first_row_alone = vec![0b0011_0110, 0b1100_0000];
  let strips = vec![first_row_alone.clone(), first_row_alone];
  let image = first_page(faxed((8, 2), 0, 1, strips)).unwrap();
  let row = [255, 255, 255, 255, 0, 0, 0, 0];
  assert_eq!(gray(&image), [row, row]);
}

#[test]
fn a_fax_written_low_end_first_is_turned_before_it_is_decoded() {
  let turned: Vec<u8> = HALF_AND_HALF
    .iter()
    .map(|byte| byte.reverse_bits())
    .collect();
  let mut page = faxed((8, 2), 0, 2, vec![turned]);
  page.push((266, Value::Short(vec![2])));
  assert_eq!(
    gray(&first_page(page).unwrap())[1],
    [255, 255, 255, 255, 0, 0, 0, 0]
  );
}

#[test]
fn a_fax_that_stops_before_its_last_row_is_malformed() {
  // One row of the two the page says the strip holds.
  let page = faxed(
    (8, 2),
    0,
    2,
    vec![vec![0b0011_0110, 0b1100_0000][..1].to_vec()],
  );
  assert_eq!(
    first_page(page).unwrap_err().to_string(),
    "malformed file: strip 1 of page 1 does not unpack to the rows it should hold"
  );
}

#[test]
fn strips_whose_lengths_an_old_writer_left_out_run_to_the_next_strip_or_the_end() {
  let mut file = tiff(
    true,
    &[kept((2, 2), vec![8], 1, 1, vec![vec![1, 2], vec![3, 4]])],
  );
  // The entry that gives the lengths is made into one that means nothing.
  let entries = usize::from(u16::from_le_bytes([file[8], file[9]]));
  let lengths = (0..entries)
    .map(|index| 10 + 12 * index)
    .find(|at| u16::from_le_bytes([file[*at], file[*at + 1]]) == 279)
    .unwrap();
  file[lengths..lengths + 2].copy_from_slice(&65_000u16.to_le_bytes());
  let image = Tiff::open(file).unwrap().raster(1).unwrap();
  assert_eq!(gray(&image), [[1, 2], [3, 4]]);
}

#[test]
fn a_page_is_given_turned_the_way_it_is_to_be_looked_at() {
  let turned = |orientation: u16| {
    let mut page = kept((2, 1), vec![8], 1, 1, vec![vec![10, 20]]);
    page.push((274, Value::Short(vec![orientation])));
    gray(&first_page(page).unwrap())
  };
  assert_eq!(turned(1), [[10, 20]]);
  // Mirrored, and upside down: for one row, each is the row backward.
  assert_eq!(turned(2), [[20, 10]]);
  assert_eq!(turned(3), [[20, 10]]);
  // On its side: the row becomes a column, read down or read up.
  assert_eq!(turned(6), [vec![10], vec![20]]);
  assert_eq!(turned(8), [vec![20], vec![10]]);
}

#[test]
fn a_fax_whose_pixels_are_not_square_is_stretched_until_they_are() {
  let at = |across: u32, down: u32| {
    let mut page = kept(
      (4, 2),
      vec![8],
      1,
      2,
      vec![vec![0, 0, 0, 0, 255, 255, 255, 255]],
    );
    page.push((282, Value::Fractions(vec![(across, 1)])));
    page.push((283, Value::Fractions(vec![(down, 1)])));
    gray(&first_page(page).unwrap())
  };
  // A fax at its standard resolution has half as many rows to the inch as pixels across:
  // each row is given twice, as the machine that prints it does.
  assert_eq!(at(204, 98), [[0; 4], [0; 4], [255; 4], [255; 4]]);
  // Fewer across than down, which is rarer: each pixel is given twice in its row.
  assert_eq!(at(100, 200), [[0; 8], [255; 8]]);
  // Square, or as near as a machine's rounding leaves it, is left as it is.
  assert_eq!(at(300, 300).len(), 2);
  assert_eq!(at(204, 196).len(), 2);
}

#[test]
fn a_page_is_written_out_as_a_png_of_the_same_pixels() {
  let opened = Tiff::open(tiff(
    true,
    &[
      faxed((8, 2), 0, 2, vec![HALF_AND_HALF.to_vec()]),
      kept(
        (2, 1),
        vec![8, 8, 8],
        2,
        1,
        vec![vec![255, 0, 0, 0, 128, 255]],
      ),
    ],
  ))
  .unwrap();
  for number in [1, 2] {
    let png = opened.png(number).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "page {number}");
    assert_eq!(
      image::load_from_memory(&png).unwrap(),
      opened.raster(number).unwrap(),
      "page {number}"
    );
  }
  // A page that cannot be read cannot be written out, for the same reason.
  assert_eq!(
    opened.png(3).unwrap_err().to_string(),
    "limit exceeded: page 3 was asked for, and the file has 2 pages"
  );
}
