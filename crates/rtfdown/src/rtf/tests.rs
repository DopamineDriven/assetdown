use super::*;
use crate::testing::{compound, document, hex, object, object_data};

const PDF: &[u8] = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF\n";

#[test]
fn a_file_attached_to_a_saved_message_comes_out_whole() {
  let attached = compound(&[
    ("AttachDesc", b"\x03\x02"),
    ("AttachContents", PDF),
    ("AttachPres", b"\x01"),
  ]);
  let file = document(&object(
    "Outlook.FileAttach",
    &object_data("Outlook.FileAttach", &attached),
  ));
  let rtf = Rtf::open(&file).unwrap();
  let [found] = rtf.embedded() else {
    panic!(
      "one file was attached, and {} came out",
      rtf.embedded().len()
    );
  };
  assert_eq!(found.class.as_deref(), Some("Outlook.FileAttach"));
  assert_eq!(found.bytes, PDF);
  assert_eq!(rtf.warnings(), Vec::<String>::new());
}

#[test]
fn the_picture_that_stands_for_an_object_is_not_one_of_the_documents_pictures() {
  let attached = compound(&[("AttachContents", PDF)]);
  let file = document(&object(
    "Outlook.FileAttach",
    &object_data("Outlook.FileAttach", &attached),
  ));
  assert_eq!(Rtf::open(&file).unwrap().pictures().len(), 0);
}

#[test]
fn an_office_document_kept_in_a_package_stream_comes_out_as_the_document() {
  let docx: Vec<u8> = [&b"PK\x03\x04"[..], &[9u8; 6000]].concat();
  let held = compound(&[("\u{1}CompObj", b"x"), ("Package", &docx)]);
  let file = document(&object(
    "Word.Document.12",
    &object_data("Word.Document.12", &held),
  ));
  let rtf = Rtf::open(&file).unwrap();
  assert_eq!(rtf.embedded()[0].bytes, docx);
  assert_eq!(rtf.embedded()[0].class.as_deref(), Some("Word.Document.12"));
}

#[test]
fn a_pdf_kept_in_a_contents_stream_comes_out_as_the_pdf() {
  let held = compound(&[("CONTENTS", PDF)]);
  let file = document(&object(
    "AcroExch.Document.DC",
    &object_data("AcroExch.Document.DC", &held),
  ));
  assert_eq!(Rtf::open(&file).unwrap().embedded()[0].bytes, PDF);
}

#[test]
fn a_compound_file_with_no_stream_that_holds_a_file_is_itself_the_file() {
  // A workbook in the format before 2007 is its own compound file.
  let workbook = compound(&[("Workbook", &[3u8; 300]), ("\u{5}SummaryInformation", b"s")]);
  let file = document(&object(
    "Excel.Sheet.8",
    &object_data("Excel.Sheet.8", &workbook),
  ));
  assert_eq!(Rtf::open(&file).unwrap().embedded()[0].bytes, workbook);
}

#[test]
fn a_packaged_file_comes_out_with_the_name_on_its_label() {
  let mut packed = 2u16.to_le_bytes().to_vec();
  packed.extend(b"invoice.pdf\0C:\\Users\\a\\invoice.pdf\0");
  packed.extend(0x0003_0000u32.to_le_bytes());
  packed.extend(19u32.to_le_bytes());
  packed.extend(b"C:\\Temp\\invoice.pdf\0");
  packed.extend(u32::try_from(PDF.len()).unwrap().to_le_bytes());
  packed.extend(PDF);
  packed.extend(b"what follows the data");
  let file = document(&object("Package", &object_data("Package", &packed)));
  let rtf = Rtf::open(&file).unwrap();
  assert_eq!(rtf.embedded()[0].bytes, PDF);
  assert_eq!(rtf.embedded()[0].name.as_deref(), Some("invoice.pdf"));
}

#[test]
fn data_of_a_class_that_is_not_known_is_given_as_it_is() {
  let file = document(&object(
    "Paint.Picture",
    &object_data("Paint.Picture", b"BM some bitmap"),
  ));
  let rtf = Rtf::open(&file).unwrap();
  assert_eq!(rtf.embedded()[0].bytes, b"BM some bitmap");
  assert_eq!(rtf.embedded()[0].name, None);
}

#[test]
fn object_data_given_as_bytes_and_as_hex_is_one_run() {
  let data = object_data("Paint.Picture", b"0123456789");
  let (first, rest) = data.split_at(data.len() - 4);
  // The last four bytes are given with \bin, as they are; the rest in hex.
  let body = format!(
    "{{\\object\\objemb{{\\*\\objclass Paint.Picture}}{{\\*\\objdata {}\\bin4 {}}}}}",
    hex(first),
    String::from_utf8_lossy(rest)
  );
  assert_eq!(
    Rtf::open(&document(&body)).unwrap().embedded()[0].bytes,
    b"0123456789"
  );
}

#[test]
fn an_attachment_that_is_named_and_not_in_the_file_is_said() {
  let body = "{\\object\\objattph {\\*\\objclass Outlook.FileAttach}}\\par text";
  let rtf = Rtf::open(&document(body)).unwrap();
  assert_eq!(rtf.embedded().len(), 0);
  assert_eq!(
    rtf.warnings(),
    ["an attachment has its place marked in the text, and is not in the file"]
  );
}

#[test]
fn a_documents_pictures_come_out_with_what_they_are() {
  let png = b"\x89PNG\r\n\x1a\nrest of a png";
  let jpeg = b"\xff\xd8\xff\xe0 rest of a jpeg";
  let body = format!(
    "{{\\pict\\pngblip\\picw10\\pich20\\picwgoal1440\\pichgoal2880 {}}} and {{\\*\\shppict{{\\pict\\jpegblip {}}}}}{{\\nonshppict{{\\pict\\wmetafile8 0102}}}}{{\\pict\\emfblip\\bin3 emf}}",
    hex(png),
    hex(jpeg)
  );
  let rtf = Rtf::open(&document(&body)).unwrap();
  let found: Vec<(PictureFormat, &[u8])> = rtf
    .pictures()
    .iter()
    .map(|picture| (picture.format, picture.bytes.as_slice()))
    .collect();
  assert_eq!(
    found,
    [
      (PictureFormat::Png, &png[..]),
      (PictureFormat::Jpeg, &jpeg[..]),
      (PictureFormat::Emf, &b"emf"[..]),
    ]
  );
  // The size it is to be shown at, in twentieths of a point.
  assert_eq!(rtf.pictures()[0].goal, Some((1440, 2880)));
  assert_eq!(rtf.pictures()[1].goal, None);
}

#[test]
fn bytes_that_are_not_rtf_are_refused_by_name() {
  assert_eq!(
    Rtf::open(b"%PDF-1.7").unwrap_err(),
    RtfError::UnsupportedFormat("the file does not open as RTF does".to_string())
  );
  assert!(Rtf::open(b"").is_err());
  // A byte-order mark and blank lines before the opening are passed over.
  assert!(Rtf::open(b"\xEF\xBB\xBF\r\n {\\rtf1 x}").is_ok());
}

#[test]
fn groups_nested_deeper_than_any_document_are_refused() {
  let deep = format!("{{\\rtf1 {}", "{".repeat(MOST_DEPTH + 1));
  assert_eq!(
    Rtf::open(deep.as_bytes()).unwrap_err(),
    RtfError::LimitExceeded("groups are nested more than 10000 deep".to_string())
  );
}

#[test]
fn what_a_picture_group_says_of_the_picture_is_not_the_pictures_data() {
  // As Word writes one: the shape's properties and an identifier, each in a group of its own
  // and each full of letters that are also hex digits, and then the data.
  let jpeg = b"\xff\xd8\xff\xe0 a jpeg \xff\xd9";
  let body = format!(
    "{{\\*\\shppict{{\\pict{{\\*\\picprop\\shplid1025{{\\sp{{\\sn shapeType}}{{\\sv 75}}}}{{\\sp{{\\sn fFlipH}}{{\\sv 0}}}}}}\\picscalex100\\picscaley100\\picwgoal8640\\pichgoal11520\\jpegblip\\bliptag-1{{\\*\\blipuid abcdef0123456789abcdef0123456789}}{}}}}}",
    hex(jpeg)
  );
  let rtf = Rtf::open(&document(&body)).unwrap();
  let [picture] = rtf.pictures() else {
    panic!("one picture, and {} came out", rtf.pictures().len());
  };
  assert_eq!(picture.format, PictureFormat::Jpeg);
  assert_eq!(picture.bytes, jpeg);
  assert_eq!(picture.goal, Some((8640, 11520)));
}

#[test]
fn a_pictures_format_is_what_its_bytes_are_where_they_say() {
  // One writer marks the pattern a shape is filled with as a PNG, and it is not one.
  let not_a_png = b"\x92\x14\x00\x00 a fill pattern";
  let jpeg = b"\xff\xd8\xff\xe0 a jpeg";
  let body = format!(
    "{{\\pict\\pngblip {}}}{{\\pict {}}}",
    hex(not_a_png),
    hex(jpeg)
  );
  let rtf = Rtf::open(&document(&body)).unwrap();
  let formats: Vec<PictureFormat> = rtf
    .pictures()
    .iter()
    .map(|picture| picture.format)
    .collect();
  assert_eq!(formats, [PictureFormat::Unknown, PictureFormat::Jpeg]);
}
