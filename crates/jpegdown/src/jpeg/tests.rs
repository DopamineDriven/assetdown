use contentdown::PaperSize;
use image::{DynamicImage, GrayImage, RgbImage};

use super::*;
use crate::testing::{Exif, encoded, exif, file, frame, jfif, with_segment};

fn gray(width: u32, height: u32) -> DynamicImage {
  DynamicImage::ImageLuma8(GrayImage::from_pixel(width, height, image::Luma([255])))
}

/// A file that states this much and holds no picture data: enough for what is read of a page
/// without decoding it.
fn stating(dpi: Option<u16>, width: u16, height: u16) -> Jpeg {
  let mut segments = Vec::new();
  if let Some(dpi) = dpi {
    segments.push(jfif(1, dpi, dpi));
  }
  segments.push(frame(0xC0, 8, width, height, 1));
  Jpeg::open(file(&segments)).unwrap()
}

#[test]
fn a_gray_scan_states_its_size_its_resolution_and_how_it_is_kept() {
  let jpeg = Jpeg::open(encoded(&gray(34, 22), Some(200))).unwrap();
  let page = jpeg.page();
  assert_eq!((page.width, page.height), (34, 22));
  assert_eq!(page.resolution, Some(Resolution { x: 200.0, y: 200.0 }));
  assert_eq!((page.bits_per_sample, page.components), (8, 1));
  assert_eq!((page.color, page.coding), (Color::Gray, Coding::Baseline));
  assert_eq!(page.orientation, 1);
}

#[test]
fn a_picture_in_color_says_so() {
  let color = DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, image::Rgb([200, 30, 30])));
  let jpeg = Jpeg::open(encoded(&color, None)).unwrap();
  assert_eq!(
    (jpeg.page().components, jpeg.page().color),
    (3, Color::YCbCr)
  );
  assert_eq!(jpeg.page().resolution, None);
}

#[test]
fn a_scan_that_states_a_resolution_is_the_size_that_makes_it() {
  let page = stating(Some(200), 1700, 2200).page().clone();
  assert_eq!(page.size_in_points(), Some((612.0, 792.0)));
  assert_eq!(page.paper_size(), Some(PaperSize::Letter));
  assert_eq!(
    page.size_as_read(),
    PageSize {
      width: 612.0,
      height: 792.0,
      assumed: None
    }
  );
}

#[test]
fn a_resolution_that_would_make_a_page_no_sheet_is_not_believed() {
  // A photograph from a telephone: 72 to the inch is a number its software always writes, and
  // would make the sheet three and a half feet wide. Its shape is nearest to a letter sheet.
  let page = stating(Some(72), 3024, 4032).page().clone();
  assert_eq!(page.size_in_points(), Some((3024.0, 4032.0)));
  assert_eq!(
    page.size_as_read(),
    PageSize {
      width: 612.0,
      height: 792.0,
      assumed: Some(PaperSize::Letter)
    }
  );
}

#[test]
fn a_usual_default_is_believed_only_when_it_makes_a_standard_sheet() {
  // 96 to the inch on a letter page scanned at 150: thirteen inches by seventeen is a size
  // that could be, and is no paper.
  let scanned = stating(Some(96), 1275, 1650).page().size_as_read();
  assert_eq!((scanned.width, scanned.height), (612.0, 792.0));
  assert_eq!(scanned.assumed, Some(PaperSize::Letter));
  // 72 to the inch on a picture that is a letter page at 72 is a measurement.
  let small = stating(Some(72), 612, 792).page().size_as_read();
  assert_eq!(
    (small.width, small.height, small.assumed),
    (612.0, 792.0, None)
  );
  // A resolution that is no default is believed for any size a sheet could have.
  let odd = stating(Some(150), 900, 600).page().size_as_read();
  assert_eq!((odd.width, odd.height, odd.assumed), (432.0, 288.0, None));
}

#[test]
fn a_picture_that_states_no_resolution_is_the_sheet_its_shape_is_nearest_to() {
  let tall = stating(None, 2480, 3508).page().size_as_read();
  assert_eq!(tall.assumed, Some(PaperSize::A4));
  // The sheet's size is the one the shared table of papers has, in whole points.
  assert_eq!((tall.width, tall.height), (595.0, 842.0));
  // A picture wider than it is tall is the sheet on its side.
  let wide = stating(None, 2200, 1700).page().size_as_read();
  assert_eq!((wide.width, wide.height), (792.0, 612.0));
  assert_eq!(wide.assumed, Some(PaperSize::Letter));
  let long = stating(None, 1700, 2800).page().size_as_read();
  assert_eq!(long.assumed, Some(PaperSize::Legal));
}

#[test]
fn a_picture_kept_on_its_side_is_sized_as_it_is_to_be_looked_at() {
  let kept = file(&[
    jfif(1, 200, 200),
    exif(true, &[(0x0112, Exif::Short(6))]),
    frame(0xC0, 8, 2200, 1700, 1),
  ]);
  let page = Jpeg::open(kept).unwrap().page().clone();
  // As it lies in the file it is wide; turned as its orientation says, it is a letter page.
  assert_eq!(page.size_in_points(), Some((792.0, 612.0)));
  let read = page.size_as_read();
  assert_eq!(
    (read.width, read.height, read.assumed),
    (612.0, 792.0, None)
  );
}

#[test]
fn the_picture_is_given_turned_the_way_its_orientation_says() {
  let plain = encoded(&gray(4, 2), Some(200));
  let upright = Jpeg::open(plain.clone()).unwrap().raster().unwrap();
  assert_eq!((upright.width(), upright.height()), (4, 2));
  let turned = with_segment(&plain, &exif(false, &[(0x0112, Exif::Short(6))]));
  let jpeg = Jpeg::open(turned).unwrap();
  assert_eq!((jpeg.page().width, jpeg.page().height), (4, 2));
  let raster = jpeg.raster().unwrap();
  assert_eq!((raster.width(), raster.height()), (2, 4));
  assert!(matches!(raster, DynamicImage::ImageLuma8(_)));
}

#[test]
fn the_picture_as_a_png_has_the_size_it_is_looked_at_in() {
  let png = Jpeg::open(encoded(&gray(4, 2), None))
    .unwrap()
    .png()
    .unwrap();
  assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
  assert_eq!(&png[16..24], &[0, 0, 0, 4, 0, 0, 0, 2]);
}

#[test]
fn codings_that_are_not_read_are_refused_by_name_and_the_page_still_listed() {
  let refused = |marker: u8, precision: u8| {
    let jpeg = Jpeg::open(file(&[frame(marker, precision, 4, 4, 1)])).unwrap();
    assert_eq!((jpeg.page().width, jpeg.page().height), (4, 4));
    jpeg.raster().unwrap_err()
  };
  assert_eq!(
    refused(0xC3, 8),
    JpegError::UnsupportedFormat(
      "the picture is kept without loss, a coding of JPEG that is not read".to_string()
    )
  );
  assert_eq!(
    refused(0xC1, 12),
    JpegError::UnsupportedFormat("the picture has samples of 12 bits, and 8 are read".to_string())
  );
  assert_eq!(
    refused(0xC9, 8),
    JpegError::UnsupportedFormat(
      "the picture is kept in a coding of JPEG that is not read (frame marker 0xC9)".to_string()
    )
  );
}

#[test]
fn a_picture_whose_data_cannot_be_decoded_is_damaged() {
  // A header and no picture data after it.
  let error = stating(Some(200), 8, 8).raster().unwrap_err();
  let JpegError::Malformed(why) = error else {
    panic!("not malformed: {error:?}");
  };
  assert!(
    why.starts_with("the picture could not be decoded: "),
    "{why}"
  );
}

#[test]
fn a_picture_larger_than_any_page_is_refused_before_it_is_decoded() {
  assert_eq!(
    stating(Some(200), 65_535, 65_535).raster().unwrap_err(),
    JpegError::LimitExceeded(
      "the picture has 4294836225 pixels, and no more than 268435456 are decoded".to_string()
    )
  );
}

#[test]
fn the_file_knows_how_many_bytes_it_has() {
  let bytes = encoded(&gray(4, 2), None);
  let length = u64::try_from(bytes.len()).unwrap();
  assert_eq!(Jpeg::open(bytes).unwrap().byte_size(), length);
}
