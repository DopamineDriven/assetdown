use super::*;
use crate::testing::{Parts, SheetSpec, sheet_path};

fn a1(range: Option<CellRange>) -> Option<String> {
  range.map(|range| range.a1())
}

/// A sheet whose rows are `rows`, with nothing else in it.
fn rows(rows: &str) -> String {
  format!("<sheetData>{rows}</sheetData>")
}

#[test]
fn a_workbook_counts_its_sheets_and_what_their_cells_hold() {
  let first = format!(
    r#"<dimension ref="A1:Z99"/>{}<mergeCells count="1"><mergeCell ref="A1:C1"/></mergeCells>"#,
    rows(
      r#"<row r="1"><c r="A1" t="inlineStr"><is><t>Invoice total</t></is></c></row>
         <row r="2"><c r="B2"><v>4</v></c></row>
         <row r="3"><c r="C3"><f>B2*2</f><v>8</v></c></row>
         <row r="5"><c r="D5" t="str"><v>net 30  days</v></c></row>"#
    )
  );
  let second = rows(r#"<row r="7"><c r="B7" t="inlineStr"><is><t>secret</t></is></c></row>"#);
  let sheets = [
    SheetSpec::visible("Form", &first),
    SheetSpec {
      hidden: true,
      ..SheetSpec::visible("Notes", &second)
    },
  ];
  let meta = Parts::workbook(&sheets, "").open().unwrap().metadata();

  assert_eq!((meta.sheet_count, meta.hidden_sheet_count), (2, 1));
  assert_eq!(meta.cell_count, 5);
  assert_eq!(meta.formula_count, 1);
  assert_eq!(meta.merged_range_count, 1);
  // "Invoice total", "net 30  days" and "secret": the numbers are not words.
  assert_eq!(meta.word_count, 6);
  assert_eq!(meta.character_count, 12 + 9 + 6);

  let [form, notes] = meta.sheets.as_slice() else {
    panic!("two sheets were expected, found {}", meta.sheets.len());
  };
  assert_eq!(
    (form.name.as_str(), form.index, form.hidden),
    ("Form", 0, false)
  );
  // The file says more than it holds; the used range is what holds something.
  assert_eq!(a1(form.dimension).as_deref(), Some("A1:Z99"));
  assert_eq!(a1(form.used_range).as_deref(), Some("A1:D5"));
  assert_eq!((form.row_count, form.column_count), (5, 4));
  assert_eq!((form.cell_count, form.formula_count), (4, 1));
  assert_eq!(form.merged_range_count, 1);

  assert_eq!(
    (notes.name.as_str(), notes.index, notes.hidden),
    ("Notes", 1, true)
  );
  assert_eq!(notes.dimension, None);
  assert_eq!(a1(notes.used_range).as_deref(), Some("B7:B7"));
  assert_eq!(
    (notes.row_count, notes.column_count, notes.cell_count),
    (1, 1, 1)
  );
}

#[test]
fn a_sheet_with_no_cells_has_no_used_range() {
  let sheets = [SheetSpec::visible("Empty", "<sheetData/>")];
  let meta = Parts::workbook(&sheets, "").open().unwrap().metadata();
  let sheet = &meta.sheets[0];
  assert_eq!(sheet.used_range, None);
  assert_eq!(
    (sheet.row_count, sheet.column_count, sheet.cell_count),
    (0, 0, 0)
  );
  assert_eq!((meta.cell_count, meta.word_count), (0, 0));
}

#[test]
fn the_cells_that_follow_a_shared_formula_are_formula_cells() {
  let inner = rows(
    r#"<row r="1"><c r="A1"><f t="shared" ref="A1:A3" si="0">B1*2</f><v>2</v></c></row>
       <row r="2"><c r="A2"><f t="shared" si="0"/><v>4</v></c></row>
       <row r="3"><c r="A3"><f t="shared" si="0"/><v>6</v></c></row>
       <row r="4"><c r="A4"><v>6</v></c></row>"#,
  );
  let meta = Parts::workbook(&[SheetSpec::visible("S", &inner)], "")
    .open()
    .unwrap()
    .metadata();
  assert_eq!((meta.cell_count, meta.formula_count), (4, 3));
}

#[test]
fn links_and_tables_are_counted_from_the_sheet_itself() {
  let inner = format!(
    r#"{}<hyperlinks><hyperlink ref="A1" r:id="rId1"/><hyperlink ref="B2:B4" location="Notes!A1"/></hyperlinks><tableParts count="2"><tablePart r:id="rId2"/><tablePart r:id="rId3"/></tableParts>"#,
    rows(r#"<row r="1"><c r="A1" t="str"><v>site</v></c></row>"#)
  );
  let meta = Parts::workbook(&[SheetSpec::visible("S", &inner)], "")
    .open()
    .unwrap()
    .metadata();
  assert_eq!((meta.link_count, meta.table_count), (2, 2));
  assert_eq!(
    (meta.sheets[0].link_count, meta.sheets[0].table_count),
    (2, 2)
  );
  assert_eq!((meta.comment_count, meta.annotation_count), (0, 2));
}

const DRAWING: &str = r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor><xdr:pic><xdr:nvPicPr/></xdr:pic></xdr:twoCellAnchor>
  <xdr:twoCellAnchor><xdr:grpSp><xdr:sp/><xdr:grpSp><xdr:pic><xdr:nvPicPr/></xdr:pic></xdr:grpSp></xdr:grpSp></xdr:twoCellAnchor>
  <xdr:twoCellAnchor><xdr:graphicFrame><a:graphic><a:graphicData><c:chart r:id="rId1"/></a:graphicData></a:graphic></xdr:graphicFrame></xdr:twoCellAnchor>
  <xdr:twoCellAnchor><mc:AlternateContent>
    <mc:Choice Requires="cx1"><xdr:graphicFrame><a:graphic><a:graphicData><cx:chart r:id="rId2"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice>
    <mc:Fallback><xdr:pic><xdr:nvPicPr/></xdr:pic></mc:Fallback>
  </mc:AlternateContent></xdr:twoCellAnchor>
</xdr:wsDr>"#;

const COMMENTS: &str = r#"<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><authors><author>A</author></authors><commentList>
  <comment ref="A1" authorId="0"><text><t>check</t></text></comment>
  <comment ref="B2" authorId="0"><text><t>[Threaded comment]</t></text></comment>
  <comment ref="C3" authorId="0"><text><t>paid</t></text></comment>
</commentList></comments>"#;

#[test]
fn comments_pictures_charts_and_pivot_tables_are_found_through_the_sheets_relationships() {
  let cells = rows(r#"<row r="1"><c r="A1"><v>1</v></c></row>"#);
  let sheets = [
    SheetSpec::visible("Data", &cells),
    SheetSpec::visible("Plain", &cells),
  ];
  let mut parts = Parts::workbook(&sheets, "")
    .part("xl/comments1.xml", COMMENTS)
    .part("xl/drawings/drawing1.xml", DRAWING);
  let data = sheet_path(1);
  parts.relate(&data, "comments", "xl/comments1.xml");
  parts.relate(&data, "drawing", "xl/drawings/drawing1.xml");
  parts.relate(&data, "pivotTable", "xl/pivotTables/pivotTable1.xml");
  parts.relate(&data, "pivotTable", "xl/pivotTables/pivotTable2.xml");
  // Neither of these is a drawing of pictures or a part of the package.
  parts.relate(&data, "vmlDrawing", "xl/drawings/vmlDrawing1.vml");
  parts.relate_outside(&data, "hyperlink", "https://example.com/");

  let workbook = parts.open().unwrap();
  assert!(
    workbook
      .sheets
      .iter()
      .all(|sheet| sheet.warnings.is_empty())
  );
  let meta = workbook.metadata();
  let [data, plain] = meta.sheets.as_slice() else {
    panic!("two sheets were expected, found {}", meta.sheets.len());
  };
  // Two pictures, one of them inside a group; the third is only the fallback for a chart.
  assert_eq!((data.image_count, data.chart_count), (2, 2));
  assert_eq!((data.comment_count, data.pivot_table_count), (3, 2));
  assert_eq!(
    (
      plain.image_count,
      plain.chart_count,
      plain.comment_count,
      plain.pivot_table_count
    ),
    (0, 0, 0, 0)
  );
  assert_eq!((meta.image_count, meta.chart_count), (2, 2));
  assert_eq!((meta.comment_count, meta.pivot_table_count), (3, 2));
  assert_eq!((meta.link_count, meta.annotation_count), (0, 3));
}

#[test]
fn a_sheet_that_is_only_a_chart_has_the_chart_and_no_cells() {
  let sheets = [SheetSpec {
    root: "chartsheet",
    ..SheetSpec::visible("Chart1", r#"<drawing r:id="rId1"/>"#)
  }];
  let mut parts = Parts::workbook(&sheets, "").part("xl/drawings/drawing1.xml", DRAWING);
  parts.relate(&sheet_path(1), "drawing", "xl/drawings/drawing1.xml");
  let meta = parts.open().unwrap().metadata();
  assert_eq!(
    (meta.sheet_count, meta.cell_count, meta.chart_count),
    (1, 0, 2)
  );
}

#[test]
fn a_related_part_that_cannot_be_read_is_left_out_with_a_warning() {
  let cells = rows(r#"<row r="1"><c r="A1"><v>1</v></c></row>"#);
  let mut parts = Parts::workbook(&[SheetSpec::visible("S", &cells)], "")
    .part("xl/drawings/drawing1.xml", "this is not XML");
  let sheet = sheet_path(1);
  parts.relate(&sheet, "comments", "xl/comments1.xml");
  parts.relate(&sheet, "drawing", "xl/drawings/drawing1.xml");

  let workbook = parts.open().unwrap();
  let warnings = &workbook.sheets[0].warnings;
  assert_eq!(warnings.len(), 2, "{warnings:?}");
  assert!(warnings[0].starts_with("xl/comments1.xml was left out"));
  assert!(warnings[1].starts_with("xl/drawings/drawing1.xml was left out"));
  let meta = workbook.metadata();
  assert_eq!(
    (meta.cell_count, meta.comment_count, meta.image_count),
    (1, 0, 0)
  );
}

#[test]
fn built_in_names_are_not_among_the_names_a_workbook_defines() {
  let cells = rows(r#"<row r="1"><c r="A1"><v>1</v></c></row>"#);
  let names = r#"<definedName name="_xlnm.Print_Area" localSheetId="0">S!$A$1:$B$2</definedName>
    <definedName name="_xlnm._FilterDatabase" localSheetId="0" hidden="1">S!$A$1:$B$9</definedName>
    <definedName name="Rate">S!$B$1</definedName>
    <definedName name="Totals">S!$B$2:$B$9</definedName>"#;
  let workbook = Parts::workbook(&[SheetSpec::visible("S", &cells)], names)
    .open()
    .unwrap();
  assert_eq!(workbook.metadata().defined_name_count, 2);
  // The print area is still read from the name that is not counted.
  assert_eq!(a1(workbook.sheets[0].print_area).as_deref(), Some("A1:B2"));
}
