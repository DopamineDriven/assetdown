use super::*;
use crate::images::jbig2_tests::jbig2_page;
use lopdf::{Stream, dictionary};

/// A page box that holds everything the tests paint unless a test says otherwise.
const PAGE: [f64; 4] = [0.0, 0.0, 100.0, 100.0];

/// A raster drawn in text: `#` is ink, anything else paper.
fn raster(rows: &[&str]) -> GrayImage {
  let width = rows.first().map_or(0, |row| row.len());
  let pixels = rows
    .iter()
    .flat_map(|row| row.bytes())
    .map(|b| if b == b'#' { INK } else { PAPER })
    .collect();
  GrayImage::from_raw(width as u32, rows.len() as u32, pixels).expect("rows of one width")
}

/// A raster back as the text [`raster`] takes.
fn drawn(image: &GrayImage) -> Vec<String> {
  image
    .rows()
    .map(|row| {
      row
        .map(|pixel| if pixel.0[0] < INK_BELOW { '#' } else { '.' })
        .collect()
    })
    .collect()
}

/// The matrix that paints an image upright, `width` by `height` points with its lower left
/// corner at `(x, y)`.
fn upright(x: f64, y: f64, width: f64, height: f64) -> Matrix {
  Matrix {
    a: width,
    b: 0.0,
    c: 0.0,
    d: height,
    e: x,
    f: y,
  }
}

fn piece(image: &GrayImage, ctm: Matrix, tone: InkTone) -> Piece<'_> {
  Piece { image, ctm, tone }
}

#[test]
fn pieces_are_painted_where_their_matrices_put_them() {
  let (left, right) = (raster(&["#..", "..#"]), raster(&["##"]));
  let pieces = [
    piece(&left, upright(10.0, 20.0, 3.0, 2.0), InkTone::Dark),
    piece(&right, upright(14.0, 21.0, 2.0, 1.0), InkTone::Dark),
  ];
  let composites = paint_composites(&pieces, PAGE);
  let [composite] = composites.as_slice() else {
    panic!("one raster was expected, found {}", composites.len());
  };
  assert_eq!(composite.bbox, [10.0, 20.0, 16.0, 22.0]);
  assert_eq!((composite.tone, composite.pieces), (InkTone::Dark, 2));
  assert_eq!(drawn(&composite.image), ["#...##", "..#..."]);
}

#[test]
fn a_piece_painted_through_a_quarter_turn_is_turned() {
  // The image's top left pixel lands at the area's bottom left: a quarter turn counterclockwise.
  let image = raster(&["#..", "..."]);
  let turned = Matrix {
    a: 0.0,
    b: 3.0,
    c: -2.0,
    d: 0.0,
    e: 2.0,
    f: 0.0,
  };
  let composites = paint_composites(&[piece(&image, turned, InkTone::Dark)], PAGE);
  assert_eq!(composites[0].bbox, [0.0, 0.0, 2.0, 3.0]);
  assert_eq!(drawn(&composites[0].image), ["..", "..", "#."]);
}

#[test]
fn a_piece_painted_mirrored_is_mirrored() {
  let image = raster(&["#.", ".."]);
  let top_to_bottom = Matrix {
    d: -2.0,
    f: 2.0,
    ..upright(0.0, 0.0, 2.0, 2.0)
  };
  let composites = paint_composites(&[piece(&image, top_to_bottom, InkTone::Dark)], PAGE);
  assert_eq!(drawn(&composites[0].image), ["..", "#."]);
}

#[test]
fn light_masks_are_painted_on_a_raster_of_their_own() {
  // A bar with its lettering cut out, and the lettering painted light into the gap: on one
  // raster the two would be a solid bar.
  let (bar, lettering) = (raster(&["#.#"]), raster(&[".#."]));
  let place = upright(5.0, 5.0, 3.0, 1.0);
  let pieces = [
    piece(&bar, place, InkTone::Dark),
    piece(&lettering, place, InkTone::Light),
  ];
  let composites = paint_composites(&pieces, PAGE);
  let [dark, light] = composites.as_slice() else {
    panic!("two rasters were expected, found {}", composites.len());
  };
  assert_eq!((dark.tone, dark.pieces), (InkTone::Dark, 1));
  assert_eq!(drawn(&dark.image), ["#.#"]);
  assert_eq!((light.tone, light.pieces), (InkTone::Light, 1));
  assert_eq!(drawn(&light.image), [".#."]);
}

#[test]
fn each_tone_covers_only_the_area_its_own_pieces_occupy() {
  let (body, label) = (raster(&["#.", ".#"]), raster(&["#"]));
  let pieces = [
    piece(&body, upright(0.0, 0.0, 2.0, 2.0), InkTone::Dark),
    piece(&label, upright(50.0, 60.0, 1.0, 1.0), InkTone::Light),
  ];
  let composites = paint_composites(&pieces, PAGE);
  assert_eq!(composites[0].bbox, [0.0, 0.0, 2.0, 2.0]);
  assert_eq!(composites[1].bbox, [50.0, 60.0, 51.0, 61.0]);
  assert_eq!(drawn(&composites[1].image), ["#"]);
}

#[test]
fn nothing_is_painted_outside_the_clip() {
  let (across, away) = (raster(&["#.#."]), raster(&["#"]));
  let pieces = [
    // Starts one point left of the clip and ends one point right of it.
    piece(&across, upright(-1.0, 0.0, 4.0, 1.0), InkTone::Dark),
    piece(&away, upright(40.0, 40.0, 1.0, 1.0), InkTone::Dark),
  ];
  let composites = paint_composites(&pieces, [0.0, 0.0, 2.0, 1.0]);
  let [composite] = composites.as_slice() else {
    panic!("one raster was expected, found {}", composites.len());
  };
  assert_eq!(composite.bbox, [0.0, 0.0, 2.0, 1.0]);
  assert_eq!(composite.pieces, 1);
  assert_eq!(drawn(&composite.image), [".#"]);
}

#[test]
fn pieces_that_show_nothing_make_no_raster() {
  let image = raster(&["#"]);
  let flat = Matrix {
    d: 0.0,
    ..upright(1.0, 1.0, 1.0, 1.0)
  };
  let pieces = [
    piece(&image, flat, InkTone::Dark),
    piece(&image, upright(500.0, 500.0, 1.0, 1.0), InkTone::Dark),
  ];
  assert!(paint_composites(&pieces, PAGE).is_empty());
}

#[test]
fn the_raster_takes_the_resolution_most_of_the_pixels_have() {
  // 32 pixels at two to the point and 16 at eight to the point: the raster is two to the point,
  // and the small image is sampled down onto it.
  let (large, small) = (
    raster(&["#.......", "........", "........", "........"]),
    raster(&["####", "####", "####", "####"]),
  );
  let pieces = [
    piece(&large, upright(0.0, 0.0, 4.0, 2.0), InkTone::Dark),
    piece(&small, upright(4.0, 0.0, 0.5, 0.5), InkTone::Dark),
  ];
  let composites = paint_composites(&pieces, PAGE);
  assert_eq!(composites[0].bbox, [0.0, 0.0, 4.5, 2.0]);
  assert_eq!(
    drawn(&composites[0].image),
    ["#........", ".........", ".........", "........#"]
  );
}

#[test]
fn an_area_too_large_to_hold_is_painted_at_a_lower_resolution() {
  let (width, height, density) =
    raster_size([0.0, 0.0, 20_000.0, 10_000.0], 10.0).expect("an area with a size");
  assert!(density < 10.0);
  assert_eq!(width, height * 2);
  let pixels = f64::from(width) * f64::from(height);
  assert!(pixels <= MAX_COMPOSITE_PIXELS && pixels > MAX_COMPOSITE_PIXELS * 0.99);
  // An area within the limit keeps its resolution, and a sliver still gets a pixel.
  assert_eq!(
    raster_size([0.0, 0.0, 612.0, 0.001], 4.0),
    Some((2448, 1, 4.0))
  );
}

/// The dictionary of an 8 by 2 JBIG2 stencil mask.
fn mask() -> Dictionary {
  dictionary! {
    "Type" => "XObject",
    "Subtype" => "Image",
    "Width" => 8,
    "Height" => 2,
    "BitsPerComponent" => 1,
    "ImageMask" => true,
    "Filter" => "JBIG2Decode",
  }
}

/// The dictionary of an 8 by 2 JBIG2 picture.
fn picture() -> Dictionary {
  let mut image = mask();
  image.remove(b"ImageMask");
  image.set("ColorSpace", "DeviceGray");
  image
}

/// A one-page document whose content stream is `content` and whose XObjects are `xobjects`,
/// and its page. The page box is [`PAGE`].
fn page(content: &str, xobjects: Vec<(&str, Stream)>) -> (Document, ObjectId) {
  let mut doc = Document::with_version("1.7");
  let mut named = Dictionary::new();
  for (name, stream) in xobjects {
    let id = doc.add_object(stream);
    named.set(name, Object::Reference(id));
  }
  // An ICC profile's own bytes are never read here: its family name is what counts.
  let profile = doc.add_object(Stream::new(dictionary! { "N" => 3 }, Vec::new()));
  let content_id = doc.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
  let pages_id = doc.new_object_id();
  let page_id = doc.add_object(dictionary! {
    "Type" => "Page",
    "Parent" => Object::Reference(pages_id),
    "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
    "Contents" => Object::Reference(content_id),
    "Resources" => Object::Dictionary(dictionary! {
      "XObject" => Object::Dictionary(named),
      "ColorSpace" => Object::Dictionary(dictionary! {
        "Profile" => vec![Object::Name(b"ICCBased".to_vec()), Object::Reference(profile)],
        "Spot" => vec![
          Object::Name(b"Separation".to_vec()),
          Object::Name(b"Gold".to_vec()),
          Object::Name(b"DeviceCMYK".to_vec()),
        ],
      }),
    }),
  });
  doc.objects.insert(
    pages_id,
    Object::Dictionary(
      dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 },
    ),
  );
  let catalog =
    doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
  doc.trailer.set("Root", Object::Reference(catalog));
  (doc, page_id)
}

fn jbig2(dict: Dictionary) -> Stream {
  Stream::new(dict, jbig2_page())
}

/// The tone of each paint on a page, with the name of the image painted.
fn tones(doc: &Document, page_id: ObjectId) -> Vec<(String, InkTone)> {
  let sources = page_image_sources(doc, page_id);
  one_bit_paints(doc, page_id, &sources)
    .iter()
    .map(|paint| (sources[paint.source].name.clone(), paint.tone))
    .collect()
}

#[test]
fn a_mask_takes_its_tone_from_the_fill_color_in_force() {
  let content = "
    q 8 0 0 2 10 90 cm /M Do Q
    1 g q 8 0 0 2 10 80 cm /M Do Q
    q 0.2 0.2 0.6 rg 8 0 0 2 10 70 cm /M Do Q
    q 8 0 0 2 10 60 cm /M Do Q
    0 0 0 0 k q 8 0 0 2 10 50 cm /M Do Q
    0 0 0 1 k q 8 0 0 2 10 40 cm /M Do Q";
  let (doc, page_id) = page(content, vec![("M", jbig2(mask()))]);
  let expected = [
    // Black until a color is set.
    InkTone::Dark,
    InkTone::Light,
    // A dark blue, set inside a `q` that the `Q` then undoes.
    InkTone::Dark,
    InkTone::Light,
    // No ink in any of the four plates is paper; all of the black plate is black.
    InkTone::Light,
    InkTone::Dark,
  ];
  let found: Vec<InkTone> = tones(&doc, page_id)
    .into_iter()
    .map(|(_, tone)| tone)
    .collect();
  assert_eq!(found, expected);
}

#[test]
fn a_color_is_read_through_a_named_color_space_when_its_components_are_levels() {
  let content = "
    /Profile cs 1 1 1 sc q 8 0 0 2 10 90 cm /M Do Q
    /Profile cs q 8 0 0 2 10 80 cm /M Do Q
    /Spot cs 0 scn q 8 0 0 2 10 70 cm /M Do Q
    /DeviceGray cs 0.9 sc q 8 0 0 2 10 60 cm /M Do Q";
  let (doc, page_id) = page(content, vec![("M", jbig2(mask()))]);
  let expected = [
    InkTone::Light,
    // Selecting a space selects black.
    InkTone::Dark,
    // No tint of a spot ink says how light it prints: the mask is taken for ink.
    InkTone::Dark,
    InkTone::Light,
  ];
  let found: Vec<InkTone> = tones(&doc, page_id)
    .into_iter()
    .map(|(_, tone)| tone)
    .collect();
  assert_eq!(found, expected);
}

#[test]
fn a_one_bit_picture_is_dark_whatever_the_fill_color() {
  let content = "1 g q 8 0 0 2 10 90 cm /P Do Q q 8 0 0 2 10 80 cm /M Do Q";
  let (doc, page_id) = page(content, vec![("P", jbig2(picture())), ("M", jbig2(mask()))]);
  assert_eq!(
    tones(&doc, page_id),
    [
      ("P".to_string(), InkTone::Dark),
      ("M".to_string(), InkTone::Light)
    ]
  );
}

#[test]
fn a_paint_carries_the_matrix_in_force() {
  let content = "q 2 0 0 2 0 0 cm q 8 0 0 2 10 30 cm /M Do Q Q";
  let (doc, page_id) = page(content, vec![("M", jbig2(mask()))]);
  let sources = page_image_sources(&doc, page_id);
  let paints = one_bit_paints(&doc, page_id, &sources);
  assert_eq!(paints.len(), 1);
  assert_eq!(paints[0].ctm, upright(20.0, 60.0, 16.0, 4.0));
}

#[test]
fn a_page_that_paints_one_one_bit_image_keeps_it_by_name() {
  let (doc, page_id) = page("q 8 0 0 2 10 50 cm /M Do Q", vec![("M", jbig2(mask()))]);
  let images = collect_page_ocr_images(&doc, page_id, PAGE);
  assert!(images.composites.is_empty());
  let [(name, image)] = images.named.as_slice() else {
    panic!("one image was expected, found {}", images.named.len());
  };
  assert_eq!(name, "M");
  assert_eq!((image.width(), image.height()), (8, 2));
}

#[test]
fn one_bit_images_painted_twice_or_more_are_merged_and_not_returned_by_name() {
  // The same mask side by side, and an 8-bit gray image that is none of this module's business.
  let gray = Stream::new(
    dictionary! {
      "Type" => "XObject",
      "Subtype" => "Image",
      "Width" => 2,
      "Height" => 1,
      "BitsPerComponent" => 8,
      "ColorSpace" => "DeviceGray",
    },
    vec![0, 255],
  );
  let content = "
    q 8 0 0 2 10 50 cm /M Do Q
    q 8 0 0 2 18 50 cm /M Do Q
    q 2 0 0 1 60 60 cm /G Do Q";
  let (doc, page_id) = page(content, vec![("M", jbig2(mask())), ("G", gray)]);
  let images = collect_page_ocr_images(&doc, page_id, PAGE);
  let names: Vec<&str> = images.named.iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["G"]);
  let [composite] = images.composites.as_slice() else {
    panic!("one raster was expected, found {}", images.composites.len());
  };
  assert_eq!(composite.bbox, [10.0, 50.0, 26.0, 52.0]);
  assert_eq!((composite.tone, composite.pieces), (InkTone::Dark, 2));
  assert_eq!(
    drawn(&composite.image),
    ["..###.....###...", "................"]
  );
}

#[test]
fn a_mask_painted_dark_and_again_light_makes_two_rasters() {
  let content = "q 8 0 0 2 10 50 cm /M Do Q 1 g q 8 0 0 2 18 50 cm /M Do Q";
  let (doc, page_id) = page(content, vec![("M", jbig2(mask()))]);
  let images = collect_page_ocr_images(&doc, page_id, PAGE);
  assert!(images.named.is_empty());
  let [dark, light] = images.composites.as_slice() else {
    panic!(
      "two rasters were expected, found {}",
      images.composites.len()
    );
  };
  assert_eq!(
    (dark.tone, dark.bbox),
    (InkTone::Dark, [10.0, 50.0, 18.0, 52.0])
  );
  assert_eq!(
    (light.tone, light.bbox),
    (InkTone::Light, [18.0, 50.0, 26.0, 52.0])
  );
  assert_eq!(drawn(&dark.image), drawn(&light.image));
}

#[test]
fn a_one_bit_image_painted_only_inside_a_form_is_left_by_name() {
  let form = Stream::new(
    dictionary! { "Type" => "XObject", "Subtype" => "Form" },
    b"q 8 0 0 2 10 30 cm /C Do Q".to_vec(),
  );
  let content = "q 8 0 0 2 10 50 cm /A Do Q q 8 0 0 2 18 50 cm /B Do Q /F Do";
  let xobjects = vec![
    ("A", jbig2(mask())),
    ("B", jbig2(mask())),
    ("C", jbig2(mask())),
    ("F", form),
  ];
  let (doc, page_id) = page(content, xobjects);
  let images = collect_page_ocr_images(&doc, page_id, PAGE);
  let names: Vec<&str> = images.named.iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["C"]);
  assert_eq!(images.composites.len(), 1);
  assert_eq!(images.composites[0].pieces, 2);
}

#[test]
fn one_bit_images_painted_only_off_the_page_are_read_by_name_as_before() {
  let content = "q 8 0 0 2 300 300 cm /A Do Q q 8 0 0 2 400 300 cm /B Do Q";
  let (doc, page_id) = page(content, vec![("A", jbig2(mask())), ("B", jbig2(mask()))]);
  let images = collect_page_ocr_images(&doc, page_id, PAGE);
  assert!(images.composites.is_empty());
  assert_eq!(images.named.len(), 2);
}
