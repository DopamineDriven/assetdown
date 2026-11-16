//! The public surface, against packages built in memory: each test states the parts it needs.

use std::io::{Cursor, Write};

use ooxmldown::{
  DocProps, Limits, Package, PackageError, PackageKind, Relationship, parse_xml, rels_path,
};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const WORKBOOK_MAIN: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const SML: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const SML_STRICT: &str = "http://purl.oclc.org/ooxml/spreadsheetml/main";
const OFFICE_DOCUMENT: &str =
  "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const OFFICE_DOCUMENT_STRICT: &str =
  "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument";

/// A zip holding `parts` in order, deflated.
fn zip_of(parts: &[(&str, &str)]) -> Vec<u8> {
  let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
  for (name, body) in parts {
    zip.start_file(*name, SimpleFileOptions::default()).unwrap();
    zip.write_all(body.as_bytes()).unwrap();
  }
  zip.finish().unwrap().into_inner()
}

fn content_types(main_part: &str, main_type: &str) -> String {
  format!(
    r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/{main_part}" ContentType="{main_type}"/></Types>"#
  )
}

fn root_rels(relationship_type: &str, main_part: &str) -> String {
  format!(
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{relationship_type}" Target="{main_part}"/></Relationships>"#
  )
}

/// The smallest package whose main part is `main_part`, typed `main_type`, with `main_xml` in it.
fn package_of(main_part: &str, main_type: &str, main_xml: &str) -> Vec<u8> {
  zip_of(&[
    ("[Content_Types].xml", &content_types(main_part, main_type)),
    ("_rels/.rels", &root_rels(OFFICE_DOCUMENT, main_part)),
    (main_part, main_xml),
  ])
}

fn workbook_xml(namespace: &str) -> String {
  format!(
    r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="{namespace}"><sheets><sheet name="S" sheetId="1"/></sheets></workbook>"#
  )
}

fn minimal_workbook() -> Vec<u8> {
  package_of("xl/workbook.xml", WORKBOOK_MAIN, &workbook_xml(SML))
}

fn kind_of(bytes: Vec<u8>) -> PackageKind {
  Package::open(bytes).unwrap().kind().unwrap()
}

#[test]
fn a_workbook_names_its_main_part_its_kind_and_its_content_types() {
  let mut package = Package::open(minimal_workbook()).unwrap();
  assert_eq!(package.officedocument().unwrap(), "xl/workbook.xml");
  assert_eq!(package.kind().unwrap(), PackageKind::Workbook);
  assert_eq!(
    package.content_type("xl/workbook.xml").unwrap().as_deref(),
    Some(WORKBOOK_MAIN),
    "an Override by part name"
  );
  assert_eq!(
    package
      .content_type("xl/worksheets/sheet1.xml")
      .unwrap()
      .as_deref(),
    Some("application/xml"),
    "a Default by extension"
  );
  assert_eq!(
    package
      .content_type("xl/media/image1.PNG")
      .unwrap()
      .as_deref(),
    Some("image/png"),
    "extensions compare without case"
  );
  assert_eq!(package.content_type("xl/vbaProject.bin").unwrap(), None);
}

#[test]
fn the_kind_follows_the_main_parts_content_type() {
  let cases = [
    (
      "xl/workbook.xml",
      "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
      PackageKind::MacroWorkbook,
    ),
    (
      "xl/workbook.xml",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
      PackageKind::TemplateWorkbook,
    ),
    (
      "xl/workbook.xml",
      "application/vnd.ms-excel.template.macroEnabled.main+xml",
      PackageKind::TemplateWorkbook,
    ),
    (
      "word/document.xml",
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
      PackageKind::WordDocument,
    ),
    (
      "word/document.xml",
      "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
      PackageKind::TemplateDocument,
    ),
    (
      "word/document.xml",
      "application/vnd.ms-word.document.macroEnabled.main+xml",
      PackageKind::MacroDocument,
    ),
    (
      "word/document.xml",
      "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
      PackageKind::MacroDocument,
    ),
    (
      "ppt/presentation.xml",
      "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
      PackageKind::Presentation,
    ),
    ("content.xml", "application/xml", PackageKind::Other),
  ];
  for (main_part, main_type, expected) in cases {
    assert_eq!(
      kind_of(package_of(main_part, main_type, "<root/>")),
      expected,
      "{main_type}"
    );
  }
}

#[test]
fn a_strict_workbook_is_told_apart_by_its_namespace() {
  let strict = zip_of(&[
    (
      "[Content_Types].xml",
      &content_types("xl/workbook.xml", WORKBOOK_MAIN),
    ),
    (
      "_rels/.rels",
      &root_rels(OFFICE_DOCUMENT_STRICT, "xl/workbook.xml"),
    ),
    ("xl/workbook.xml", &workbook_xml(SML_STRICT)),
  ]);
  assert_eq!(kind_of(strict), PackageKind::StrictWorkbook);
  assert_eq!(kind_of(minimal_workbook()), PackageKind::Workbook);
}

#[test]
fn every_kind_has_words_that_make_no_claim_about_support() {
  let kinds = [
    PackageKind::Workbook,
    PackageKind::MacroWorkbook,
    PackageKind::TemplateWorkbook,
    PackageKind::StrictWorkbook,
    PackageKind::WordDocument,
    PackageKind::MacroDocument,
    PackageKind::TemplateDocument,
    PackageKind::Presentation,
    PackageKind::Other,
  ];
  for kind in kinds {
    let words = kind.description();
    assert!(!words.is_empty(), "{kind:?}");
    assert!(!words.contains("supported"), "{kind:?}: {words}");
  }
}

#[test]
fn relationships_resolve_internal_targets_and_keep_external_ones() {
  let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="/xl/styles.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml" Target="../customXml/item1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/a/../b" TargetMode="External"/></Relationships>"#;
  let bytes = zip_of(&[
    (
      "[Content_Types].xml",
      &content_types("xl/workbook.xml", WORKBOOK_MAIN),
    ),
    (
      "_rels/.rels",
      &root_rels(OFFICE_DOCUMENT, "xl/workbook.xml"),
    ),
    ("xl/workbook.xml", &workbook_xml(SML)),
    ("xl/_rels/workbook.xml.rels", rels),
  ]);
  let mut package = Package::open(bytes).unwrap();
  let found = package
    .relationships(&rels_path("xl/workbook.xml"))
    .unwrap();
  let targets: Vec<(&str, &str, bool)> = found
    .iter()
    .map(|r| (r.id.as_str(), r.target.as_str(), r.external))
    .collect();
  assert_eq!(
    targets,
    [
      ("rId1", "xl/worksheets/sheet1.xml", false),
      ("rId2", "xl/styles.xml", false),
      ("rId3", "customXml/item1.xml", false),
      ("rId4", "https://example.com/a/../b", true),
    ]
  );
  assert_eq!(
    found[0],
    Relationship {
      id: "rId1".to_string(),
      kind: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet"
        .to_string(),
      target: "xl/worksheets/sheet1.xml".to_string(),
      external: false,
    }
  );
}

#[test]
fn rels_paths_sit_beside_the_part_they_describe() {
  assert_eq!(rels_path("xl/workbook.xml"), "xl/_rels/workbook.xml.rels");
  assert_eq!(
    rels_path("xl/worksheets/sheet1.xml"),
    "xl/worksheets/_rels/sheet1.xml.rels"
  );
  assert_eq!(rels_path("workbook.xml"), "_rels/workbook.xml.rels");
  assert_eq!(rels_path(""), "_rels/.rels");
}

#[test]
fn a_part_past_the_cap_is_a_limit_error_not_an_allocation() {
  let mut package = Package::open_with_limits(
    minimal_workbook(),
    Limits {
      part_bytes: 64,
      ..Limits::default()
    },
  )
  .unwrap();
  assert!(matches!(
    package.part("xl/workbook.xml"),
    Err(PackageError::LimitExceeded(_))
  ));
  let mut package = Package::open(minimal_workbook()).unwrap();
  assert!(
    package
      .part("xl/workbook.xml")
      .unwrap()
      .starts_with(b"<?xml")
  );
  assert!(matches!(
    package.part("xl/missing.xml"),
    Err(PackageError::Malformed(_))
  ));
}

#[test]
fn the_part_cap_measures_inflated_bytes_not_the_zip() {
  let megabyte_of_spaces = " ".repeat(1 << 20);
  let bytes = zip_of(&[("xl/big.xml", &megabyte_of_spaces)]);
  let cap = 64 << 10;
  assert!(
    bytes.len() < cap,
    "the zip itself is small: {} bytes",
    bytes.len()
  );
  let mut package = Package::open_with_limits(
    bytes,
    Limits {
      input_bytes: cap,
      part_bytes: cap,
      ..Limits::default()
    },
  )
  .unwrap();
  let error = package.part("xl/big.xml").unwrap_err();
  assert_eq!(
    error,
    PackageError::LimitExceeded(format!("part xl/big.xml inflates past {cap} bytes"))
  );
}

#[test]
fn the_total_cap_counts_every_read_including_a_repeat() {
  let body = "x".repeat(40);
  let mut package = Package::open_with_limits(
    zip_of(&[("a.xml", &body)]),
    Limits {
      total_bytes: 60,
      ..Limits::default()
    },
  )
  .unwrap();
  assert_eq!(package.part("a.xml").unwrap().len(), 40);
  assert!(matches!(
    package.part("a.xml"),
    Err(PackageError::LimitExceeded(_))
  ));
}

#[test]
fn the_input_and_entry_caps_are_checked_on_open() {
  let bytes = minimal_workbook();
  let too_big = Package::open_with_limits(
    bytes.clone(),
    Limits {
      input_bytes: bytes.len() - 1,
      ..Limits::default()
    },
  );
  assert!(matches!(too_big, Err(PackageError::LimitExceeded(_))));
  let too_many = Package::open_with_limits(
    bytes.clone(),
    Limits {
      entries: 2,
      ..Limits::default()
    },
  );
  assert!(matches!(too_many, Err(PackageError::LimitExceeded(_))));
  let exact = Package::open_with_limits(
    bytes.clone(),
    Limits {
      input_bytes: bytes.len(),
      entries: 3,
      ..Limits::default()
    },
  );
  assert!(exact.is_ok(), "a package at the caps opens");
}

#[test]
fn what_is_not_a_package_is_malformed() {
  assert!(matches!(
    Package::open(b"hello".to_vec()),
    Err(PackageError::Malformed(_))
  ));
  let no_main_part = zip_of(&[
    (
      "[Content_Types].xml",
      &content_types("x.xml", "application/xml"),
    ),
    (
      "_rels/.rels",
      r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
    ),
  ]);
  let mut package = Package::open(no_main_part).unwrap();
  assert_eq!(
    package.officedocument().unwrap_err(),
    PackageError::Malformed("no officeDocument relationship".to_string())
  );
  let mut no_rels = Package::open(zip_of(&[("a.xml", "<a/>")])).unwrap();
  assert!(matches!(no_rels.kind(), Err(PackageError::Malformed(_))));
}

#[test]
fn xml_is_parsed_without_dtds_and_only_as_utf8() {
  assert_eq!(
    parse_xml(b"<a><b/></a>")
      .unwrap()
      .root_element()
      .tag_name()
      .name(),
    "a"
  );
  let entity_bomb =
    br#"<!DOCTYPE a [<!ENTITY x "xxxxxxxxxx"><!ENTITY y "&x;&x;&x;&x;">]><a>&y;</a>"#;
  assert!(matches!(
    parse_xml(entity_bomb),
    Err(PackageError::Malformed(_))
  ));
  assert!(matches!(
    parse_xml(b"<a>\xff</a>"),
    Err(PackageError::Malformed(_))
  ));
  assert!(matches!(
    parse_xml(b"<a><b></a>"),
    Err(PackageError::Malformed(_))
  ));
}

#[test]
fn errors_read_as_category_then_detail() {
  assert_eq!(
    PackageError::LimitExceeded("the package has 3 entries; the limit is 2".to_string())
      .to_string(),
    "limit exceeded: the package has 3 entries; the limit is 2"
  );
  assert_eq!(
    PackageError::Malformed("part xl/missing.xml is missing".to_string()).to_string(),
    "malformed package: part xl/missing.xml is missing"
  );
}

const WORD_MAIN: &str =
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// A Word package whose root relationships also name the two property parts.
fn package_with_props(core: Option<&str>, app: Option<&str>) -> Vec<u8> {
  let rels = format!(
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{OFFICE_DOCUMENT}" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#
  );
  let types = content_types("word/document.xml", WORD_MAIN);
  let mut parts = vec![
    ("[Content_Types].xml", types.as_str()),
    ("_rels/.rels", rels.as_str()),
    ("word/document.xml", "<root/>"),
  ];
  if let Some(core) = core {
    parts.push(("docProps/core.xml", core));
  }
  if let Some(app) = app {
    parts.push(("docProps/app.xml", app));
  }
  zip_of(&parts)
}

#[test]
fn document_properties_come_from_the_core_and_app_parts() {
  let core = r#"<?xml version="1.0" encoding="UTF-8"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:title>Quarterly report</dc:title><dc:creator>A. Writer</dc:creator><cp:keywords>alpha, beta</cp:keywords><dc:subject> </dc:subject><cp:lastModifiedBy>B. Editor</cp:lastModifiedBy><cp:revision>7</cp:revision><dcterms:created>2026-01-02T03:04:05Z</dcterms:created><dcterms:modified>2026-02-03T04:05:06Z</dcterms:modified></cp:coreProperties>"#;
  let app = r#"<?xml version="1.0" encoding="UTF-8"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Microsoft Office Word</Application><AppVersion>16.0000</AppVersion><Pages>3</Pages><Words>871</Words><Characters>4925</Characters><Paragraphs>11</Paragraphs><TotalTime>42</TotalTime><Company>Example</Company></Properties>"#;
  let props = Package::open(package_with_props(Some(core), Some(app)))
    .unwrap()
    .doc_props()
    .unwrap();
  assert_eq!(
    props,
    DocProps {
      title: Some("Quarterly report".into()),
      creator: Some("A. Writer".into()),
      keywords: Some("alpha, beta".into()),
      last_modified_by: Some("B. Editor".into()),
      revision: Some("7".into()),
      created: Some("2026-01-02T03:04:05Z".into()),
      modified: Some("2026-02-03T04:05:06Z".into()),
      application: Some("Microsoft Office Word".into()),
      app_version: Some("16.0000".into()),
      company: Some("Example".into()),
      pages: Some(3),
      words: Some(871),
      characters: Some(4925),
      paragraphs: Some(11),
      total_time: Some(42),
      // An element that holds only whitespace states nothing.
      ..DocProps::default()
    }
  );
}

#[test]
fn missing_or_unreadable_property_parts_leave_their_fields_absent() {
  // Named by the relationships but not in the zip.
  let none = Package::open(package_with_props(None, None))
    .unwrap()
    .doc_props()
    .unwrap();
  assert_eq!(none, DocProps::default());
  // One part is not XML; the other is still read.
  let app = r#"<Properties><Pages>2</Pages><Words>many</Words></Properties>"#;
  let partial = Package::open(package_with_props(Some("<not xml"), Some(app)))
    .unwrap()
    .doc_props()
    .unwrap();
  assert_eq!(partial.title, None);
  assert_eq!(partial.pages, Some(2));
  // A count that is not a number is not a count.
  assert_eq!(partial.words, None);
  // A package with no property relationships at all.
  let bare = Package::open(package_of("word/document.xml", WORD_MAIN, "<root/>"))
    .unwrap()
    .doc_props()
    .unwrap();
  assert_eq!(bare, DocProps::default());
}
