use super::*;

#[test]
fn relationship_targets_resolve_against_their_part() {
  assert_eq!(
    resolve_target("xl/", "worksheets/sheet1.xml"),
    "xl/worksheets/sheet1.xml"
  );
  assert_eq!(resolve_target("xl/", "/xl/styles.xml"), "xl/styles.xml");
  assert_eq!(
    resolve_target("xl/worksheets/", "../drawings/d1.xml"),
    "xl/drawings/d1.xml"
  );
  assert_eq!(resolve_target("xl/", "./styles.xml"), "xl/styles.xml");
  assert_eq!(rels_base("_rels/.rels"), "");
  assert_eq!(rels_base("xl/_rels/workbook.xml.rels"), "xl/");
}

#[test]
fn a_target_cannot_climb_out_of_the_package() {
  assert_eq!(resolve_target("xl/", "../../../etc/passwd"), "etc/passwd");
  assert_eq!(resolve_target("", "../x.xml"), "x.xml");
}

#[test]
fn a_rels_path_and_its_base_agree() {
  for part in [
    "xl/workbook.xml",
    "xl/worksheets/sheet1.xml",
    "workbook.xml",
  ] {
    let dir = part
      .rsplit_once('/')
      .map(|(d, _)| format!("{d}/"))
      .unwrap_or_default();
    assert_eq!(rels_base(&rels_path(part)), dir, "{part}");
  }
  assert_eq!(rels_base(&rels_path("")), "");
}
