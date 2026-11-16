use super::*;
use crate::model::{Align, Inline};
use crate::testing::{Setup, p, read, read_with};

fn paragraph(block: &Block) -> &Paragraph {
  match block {
    Block::Paragraph(paragraph) => paragraph,
    Block::Table(_) => panic!("a table where a paragraph was expected"),
  }
}

fn table(block: &Block) -> &Table {
  match block {
    Block::Table(table) => table,
    Block::Paragraph(_) => panic!("a paragraph where a table was expected"),
  }
}

fn texts(blocks: &[Block]) -> Vec<String> {
  blocks.iter().map(Block::text).collect()
}

#[test]
fn text_is_taken_wherever_the_document_keeps_it() {
  let body = format!(
    r#"{}<w:tbl><w:tr><w:tc>{}<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl></w:tc>
       <w:sdt><w:sdtContent><w:tc>{}</w:tc></w:sdtContent></w:sdt></w:tr></w:tbl>
       <w:sdt><w:sdtContent>{}</w:sdtContent></w:sdt>
       <w:p><w:smartTag><w:r><w:t>tagged </w:t></w:r></w:smartTag><w:hyperlink w:anchor="top"><w:r><w:t>linked</w:t></w:r></w:hyperlink></w:p>"#,
    p("before"),
    p("cell"),
    p("nested"),
    p("controlled cell"),
    p("controlled block"),
  );
  let read = read(&body);
  assert_eq!(
    texts(&read.blocks),
    [
      "before",
      "cell nested\tcontrolled cell",
      "controlled block",
      "tagged linked"
    ]
  );
  let outer = table(&read.blocks[1]);
  let first = &outer.rows[0].cells[0];
  assert_eq!(texts(&first.blocks), ["cell", "nested"]);
  assert!(matches!(first.blocks[1], Block::Table(_)));
}

#[test]
fn a_text_box_is_read_once_and_follows_the_paragraph_it_is_anchored_in() {
  let body = format!(
    r#"<w:p><w:r><w:t>anchor</w:t></w:r><w:r><mc:AlternateContent>
         <mc:Choice Requires="wps"><w:drawing><wp:anchor><a:graphic><a:graphicData><wps:wsp><wps:txbx><w:txbxContent>{}</w:txbxContent></wps:txbx></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice>
         <mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent>{}</w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback>
       </mc:AlternateContent></w:r></w:p>{}
       <w:p><w:r><w:pict><v:shape><v:textbox><w:txbxContent>{}</w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>"#,
    p("label"),
    p("label"),
    p("after"),
    p("legacy label"),
  );
  let read = read(&body);
  assert_eq!(
    texts(&read.blocks),
    ["anchor", "label", "after", "", "legacy label"]
  );
  let boxed: Vec<bool> = read
    .blocks
    .iter()
    .map(|block| paragraph(block).text_box)
    .collect();
  assert_eq!(boxed, [false, true, false, false, true]);
}

#[test]
fn tracked_changes_read_as_the_final_or_the_original_view() {
  let body = r#"<w:p><w:r><w:t>kept </w:t></w:r><w:ins w:id="1" w:author="A"><w:r><w:t>added</w:t></w:r></w:ins><w:del w:id="2" w:author="A"><w:r><w:delText>removed</w:delText></w:r></w:del></w:p>"#;
  let last = read(body);
  assert_eq!(texts(&last.blocks), ["kept added"]);
  assert!(last.tracked);
  let original = read_with(
    body,
    Setup {
      view: View::Original,
      ..Setup::default()
    },
  )
  .unwrap();
  assert_eq!(texts(&original.blocks), ["kept removed"]);
  assert!(!read(&p("plain")).tracked);
}

fn link(inline: &Inline) -> &Link {
  match inline {
    Inline::Link(link) => link,
    other => panic!("{other:?} where a link was expected"),
  }
}

#[test]
fn a_link_is_an_element_or_a_field_and_reads_the_same_either_way() {
  let body = r#"<w:p><w:hyperlink r:id="rId7"><w:r><w:t>element</w:t></w:r></w:hyperlink>
      <w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> HYPERLINK "https://example.com/a b" \l "part" \o "tip" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>field</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>
      <w:fldSimple w:instr="HYPERLINK \l bookmark"><w:r><w:t>simple</w:t></w:r></w:fldSimple>
      <w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>PAGE</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>7</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#;
  let read = read_with(
    body,
    Setup {
      rels: &[("rId7", "https://example.com/element", true)],
      ..Setup::default()
    },
  )
  .unwrap();
  let inlines = &paragraph(&read.blocks[0]).inlines;
  let element = link(&inlines[0]);
  assert_eq!(element.uri.as_deref(), Some("https://example.com/element"));
  assert_eq!(element.dest, None);
  let field = link(&inlines[1]);
  assert_eq!(field.uri.as_deref(), Some("https://example.com/a b"));
  assert_eq!(field.dest.as_deref(), Some("part"));
  let simple = link(&inlines[2]);
  assert_eq!(
    (simple.uri.as_deref(), simple.dest.as_deref()),
    (None, Some("bookmark"))
  );
  // A field's code is not text; its result is.
  assert_eq!(read.blocks[0].text(), "elementfieldsimple7");
}

#[test]
fn form_fields_merge_fields_and_content_controls_are_recorded_with_their_values() {
  let field = |data: &str, code: &str, result: &str| {
    format!(
      r#"<w:r><w:fldChar w:fldCharType="begin">{data}</w:fldChar></w:r><w:r><w:instrText>{code}</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>{result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
    )
  };
  let body = format!(
    r#"<w:p>{}{}{}{}</w:p>
       <w:p><w:sdt><w:sdtPr><w:alias w:val="Customer"/><w:tag w:val="customer"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>Acme</w:t></w:r></w:sdtContent></w:sdt>
            <w:sdt><w:sdtPr><w:tag w:val="due"/><w:showingPlcHdr/><w:date/></w:sdtPr><w:sdtContent><w:r><w:t>Click to enter a date</w:t></w:r></w:sdtContent></w:sdt>
            <w:sdt><w:sdtPr><w:tag w:val="paid"/><w14:checkbox><w14:checked w14:val="1"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:t>☒</w:t></w:r></w:sdtContent></w:sdt>
            <w:sdt><w:sdtPr><w:docPartObj/></w:sdtPr><w:sdtContent><w:r><w:t>Contents</w:t></w:r></w:sdtContent></w:sdt></w:p>"#,
    field(
      r#"<w:ffData><w:name w:val="Name"/><w:textInput/></w:ffData>"#,
      "FORMTEXT",
      "Jo"
    ),
    field(
      r#"<w:ffData><w:name w:val="Agree"/><w:checkBox><w:checked/></w:checkBox></w:ffData>"#,
      "FORMCHECKBOX",
      ""
    ),
    field(
      r#"<w:ffData><w:name w:val="Tier"/><w:ddList><w:result w:val="1"/><w:listEntry w:val="Basic"/><w:listEntry w:val="Plus"/></w:ddList></w:ffData>"#,
      "FORMDROPDOWN",
      ""
    ),
    field(
      "",
      r#" MERGEFIELD "First Name" \* MERGEFORMAT "#,
      "«First Name»"
    ),
  );
  let read = read(&body);
  let found: Vec<(FieldKind, Option<&str>, &str)> = read
    .fields
    .iter()
    .map(|f| (f.kind, f.name.as_deref(), f.value.as_str()))
    .collect();
  assert_eq!(
    found,
    [
      (FieldKind::FormText, Some("Name"), "Jo"),
      (FieldKind::FormCheckbox, Some("Agree"), "true"),
      (FieldKind::FormDropDown, Some("Tier"), "Plus"),
      (FieldKind::Merge, Some("First Name"), "«First Name»"),
      (FieldKind::Text, Some("customer"), "Acme"),
      // A control that still shows its placeholder holds nothing.
      (FieldKind::Date, Some("due"), ""),
      (FieldKind::Checkbox, Some("paid"), "true"),
    ]
  );
  assert_eq!(read.fields[4].title.as_deref(), Some("Customer"));
  // The text of a control stays in the paragraph; a table of contents is not a field.
  assert!(read.blocks[1].text().contains("Acme"));
  assert!(read.blocks[1].text().contains("Contents"));
}

#[test]
fn merged_cells_become_spans() {
  let cell =
    |properties: &str, text: &str| format!("<w:tc><w:tcPr>{properties}</w:tcPr>{}</w:tc>", p(text));
  let body = format!(
    r#"<w:tbl><w:tblGrid><w:gridCol w:w="1440"/><w:gridCol w:w="2880"/><w:gridCol w:w="720"/></w:tblGrid>
       <w:tr><w:trPr><w:tblHeader/></w:trPr>{}{}</w:tr>
       <w:tr>{}{}{}</w:tr>
       <w:tr>{}{}{}</w:tr>
       <w:tr><w:trPr><w:gridBefore w:val="1"/></w:trPr>{}{}</w:tr></w:tbl>"#,
    cell(r#"<w:gridSpan w:val="2"/>"#, "wide"),
    cell("", "c"),
    cell(r#"<w:vMerge w:val="restart"/>"#, "tall"),
    cell("", "b1"),
    cell("", "c1"),
    cell("<w:vMerge/>", ""),
    cell("", "b2"),
    cell("", "c2"),
    cell("", "b3"),
    cell("", "c3"),
  );
  let read = read(&body);
  let t = table(&read.blocks[0]);
  assert_eq!(t.columns, [72.0, 144.0, 36.0]);
  let shape: Vec<Vec<(String, u32, u32, u32)>> = t
    .rows
    .iter()
    .map(|row| {
      row
        .cells
        .iter()
        .map(|c| (c.text(), c.column, c.col_span, c.row_span))
        .collect()
    })
    .collect();
  let cell = |text: &str, column, col_span, row_span| (text.to_owned(), column, col_span, row_span);
  assert_eq!(
    shape,
    [
      vec![cell("wide", 0, 2, 1), cell("c", 2, 1, 1)],
      vec![
        cell("tall", 0, 1, 2),
        cell("b1", 1, 1, 1),
        cell("c1", 2, 1, 1)
      ],
      // The cell the one above continues into is not listed.
      vec![cell("b2", 1, 1, 1), cell("c2", 2, 1, 1)],
      vec![cell("b3", 1, 1, 1), cell("c3", 2, 1, 1)],
    ]
  );
  assert_eq!(
    t.rows.iter().map(|row| row.header).collect::<Vec<_>>(),
    [true, false, false, false]
  );
}

const LISTS: &str = r#"<w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
    <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%1.%2)"/></w:lvl>
  </w:abstractNum>
  <w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/><w:lvlText w:val="&#xF0B7;"/></w:lvl></w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>
  <w:num w:numId="3"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="5"/></w:lvlOverride></w:num>"#;

#[test]
fn list_paragraphs_get_the_marker_that_stands_in_front_of_them() {
  let item = |id: u32, level: u8| {
    format!(
      r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{id}"/></w:numPr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>"#
    )
  };
  let body = [
    item(1, 0),
    item(1, 1),
    item(1, 1),
    item(1, 0),
    item(1, 1),
    item(2, 0),
    item(3, 0),
    item(3, 0),
    // A list the document does not define, and a list switched off.
    item(9, 0),
    item(0, 0),
  ]
  .concat();
  let read = read_with(
    &body,
    Setup {
      numbering: Some(LISTS),
      ..Setup::default()
    },
  )
  .unwrap();
  let markers: Vec<Option<(&str, u8, bool)>> = read
    .blocks
    .iter()
    .map(|block| {
      paragraph(block)
        .list
        .as_ref()
        .map(|item| (item.marker.as_str(), item.level, item.ordered))
    })
    .collect();
  assert_eq!(
    markers,
    [
      Some(("1.", 0, true)),
      Some(("1.a)", 1, true)),
      Some(("1.b)", 1, true)),
      Some(("2.", 0, true)),
      // The deeper level starts over under the next item.
      Some(("2.a)", 1, true)),
      Some(("•", 0, false)),
      Some(("5.", 0, true)),
      Some(("6.", 0, true)),
      None,
      None,
    ]
  );
}

const STYLES: &str = r#"<w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/><w:lang w:val="en-GB"/></w:rPr></w:rPrDefault></w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
  <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Title2"><w:name w:val="Chapter"/><w:basedOn w:val="Heading1"/><w:pPr><w:jc w:val="center"/></w:pPr></w:style>
  <w:style w:type="paragraph" w:styleId="H3"><w:name w:val="Heading 3"/></w:style>
  <w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/><w:rPr><w:b/></w:rPr></w:style>"#;

fn run(inline: &Inline) -> &TextRun {
  match inline {
    Inline::Text(run) => run,
    other => panic!("{other:?} where text was expected"),
  }
}

#[test]
fn headings_and_formatting_come_through_the_styles() {
  let styled = |style: &str, run: &str| {
    format!(r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r>{run}<w:t>x</w:t></w:r></w:p>"#)
  };
  let body = [
    styled("Heading1", ""),
    styled("Title2", ""),
    styled("H3", ""),
    p("plain"),
    r#"<w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>direct</w:t></w:r></w:p>"#.to_owned(),
    // Bold from the paragraph style and again from a character style cancel.
    styled("Heading1", r#"<w:rPr><w:rStyle w:val="Strong"/></w:rPr>"#),
    // What the run sets itself wins.
    styled("Heading1", r#"<w:rPr><w:b w:val="0"/><w:i/></w:rPr>"#),
  ]
  .concat();
  let read = read_with(
    &body,
    Setup {
      styles: Some(STYLES),
      ..Setup::default()
    },
  )
  .unwrap();
  let paragraphs: Vec<&Paragraph> = read.blocks.iter().map(paragraph).collect();
  let headings: Vec<Option<u8>> = paragraphs.iter().map(|p| p.heading).collect();
  assert_eq!(
    headings,
    [Some(1), Some(1), Some(3), None, Some(2), Some(1), Some(1)]
  );
  assert_eq!(paragraphs[0].style.as_deref(), Some("heading 1"));
  assert_eq!(paragraphs[1].style.as_deref(), Some("Chapter"));
  assert_eq!(paragraphs[1].align, Some(Align::Center));
  // A paragraph that names no style has the default one.
  assert_eq!(paragraphs[3].style.as_deref(), Some("Normal"));

  let heading = run(&paragraphs[0].inlines[0]);
  assert_eq!((heading.bold, heading.size), (true, Some(16.0)));
  let plain = run(&paragraphs[3].inlines[0]);
  assert_eq!((plain.bold, plain.size), (false, Some(11.0)));
  assert!(!run(&paragraphs[5].inlines[0]).bold);
  let direct = run(&paragraphs[6].inlines[0]);
  assert_eq!((direct.bold, direct.italic), (false, true));
}

/// A run that Word marked as the first on a page.
const MARKER: &str = r#"<w:r><w:lastRenderedPageBreak/><w:t>next</w:t></w:r>"#;

fn pages(blocks: &[Block]) -> Vec<u32> {
  blocks.iter().map(Block::page).collect()
}

#[test]
fn pages_are_counted_from_the_breaks_an_author_made_and_the_ones_word_left() {
  let body = format!(
    r#"{}
       <w:p><w:r><w:t>runs onto two </w:t></w:r>{MARKER}</w:p>
       <w:p><w:r><w:br w:type="page"/></w:r></w:p>
       <w:p>{MARKER}</w:p>
       <w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:r><w:t>four</w:t></w:r></w:p>
       <w:tbl><w:tr><w:tc><w:p>{MARKER}</w:p></w:tc><w:tc>{}</w:tc></w:tr></w:tbl>
       <w:p><w:r><w:t>anchor</w:t></w:r><w:r><w:pict><v:shape><v:textbox><w:txbxContent><w:p>{MARKER}</w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>
       <w:p><w:r><w:lastRenderedPageBreak/></w:r></w:p>"#,
    p("one"),
    p("beside"),
  );
  let read = read(&body);
  // A paragraph is on the page it starts on, however far it runs. The marker after the
  // author's break is the same break. A table is on the page its first cell starts on. A text
  // box counts nothing. An empty paragraph that carries a marker is on the page it opens.
  assert_eq!(pages(&read.blocks), [1, 1, 2, 3, 4, 5, 5, 5, 6]);
  assert_eq!(read.pages, 6);
  let row = &table(&read.blocks[5]).rows[0];
  assert_eq!(pages(&row.cells[1].blocks), [5]);
}

fn cell_pages(table: &Table) -> Vec<Vec<u32>> {
  table
    .rows
    .iter()
    .map(|row| {
      row
        .cells
        .iter()
        .flat_map(|cell| pages(&cell.blocks))
        .collect()
    })
    .collect()
}

#[test]
fn a_row_carried_over_a_page_is_one_break_however_often_word_marks_it() {
  let carried = format!(r#"<w:p><w:r><w:t>carried </w:t></w:r>{MARKER}</w:p>"#);
  let body = format!(
    r#"<w:tbl>
         <w:tr><w:tc>{}</w:tc><w:tc>{carried}</w:tc></w:tr>
         <w:tr><w:tc><w:p>{MARKER}</w:p></w:tc><w:tc>{}</w:tc></w:tr>
         <w:tr><w:tc>{carried}</w:tc><w:tc>{carried}</w:tc></w:tr>
       </w:tbl>
       <w:p>{MARKER}</w:p>
       <w:tbl>
         <w:tr><w:tc><w:p>{MARKER}</w:p></w:tc><w:tc><w:p>{MARKER}</w:p></w:tc></w:tr>
         <w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr>
       </w:tbl>"#,
    p("short"),
    p("beside"),
    p("below"),
    p("below"),
  );
  let read = read(&body);
  // The first row breaks in its second cell. Word marks the row after it as well: the same
  // page. The third row breaks in both cells: one page, and the paragraph after the table is
  // marked for it again.
  assert_eq!(cell_pages(table(&read.blocks[0])), [[1, 1], [2, 2], [2, 2]]);
  // A row that starts a page is on it in every cell, whether one cell says so or each does.
  assert_eq!(cell_pages(table(&read.blocks[2])), [[4, 4], [4, 4]]);
  assert_eq!(pages(&read.blocks), [1, 3, 4]);
  assert_eq!(read.pages, 4);
}

#[test]
fn a_field_inside_a_field_code_is_recorded_and_not_shown() {
  let part = |kind: &str| format!(r#"<w:r><w:fldChar w:fldCharType="{kind}"/></w:r>"#);
  let code = |text: &str| format!("<w:r><w:instrText>{text}</w:instrText></w:r>");
  let shown = |text: &str| format!("<w:r><w:t>{text}</w:t></w:r>");
  let body = [
    "<w:p>".to_owned(),
    shown("Total: "),
    part("begin"),
    code(" IF "),
    part("begin"),
    code(" MERGEFIELD Amount "),
    part("separate"),
    shown("250"),
    part("end"),
    code(r#" &gt; 100 "large" "small" "#),
    part("separate"),
    shown("large"),
    part("end"),
    shown(" order"),
    "</w:p>".to_owned(),
  ]
  .concat();
  let read = read(&body);
  assert_eq!(read.blocks[0].text(), "Total: large order");
  // Text formatted alike is one run again once the fields are done.
  assert_eq!(paragraph(&read.blocks[0]).inlines.len(), 1);
  let found: Vec<(FieldKind, Option<&str>, &str)> = read
    .fields
    .iter()
    .map(|f| (f.kind, f.name.as_deref(), f.value.as_str()))
    .collect();
  assert_eq!(found, [(FieldKind::Merge, Some("Amount"), "250")]);
}

#[test]
fn a_link_made_by_a_field_keeps_its_own_text_apart_from_the_text_before_it() {
  let body = r#"<w:p><w:r><w:t>see </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>HYPERLINK "https://example.com"</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>the site</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> today</w:t></w:r></w:p>"#;
  let read = read(body);
  let inlines = &paragraph(&read.blocks[0]).inlines;
  assert_eq!(inlines.len(), 3);
  assert_eq!(run(&inlines[0]).text, "see ");
  let found = link(&inlines[1]);
  assert_eq!(found.uri.as_deref(), Some("https://example.com"));
  assert_eq!(found.inlines.len(), 1);
  assert_eq!(run(&found.inlines[0]).text, "the site");
  assert_eq!(run(&inlines[2]).text, " today");
}

#[test]
fn sections_carry_their_page_setup_and_blocks_say_which_they_are_in() {
  let body = format!(
    r#"{}<w:p><w:pPr><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1080" w:bottom="1440" w:left="1080"/></w:sectPr></w:pPr></w:p>
       {}<w:p><w:pPr><w:sectPr><w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/><w:cols w:num="2"/></w:sectPr></w:pPr></w:p>
       {}<w:sectPr><w:type w:val="continuous"/><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#,
    p("first"),
    p("second"),
    p("third"),
  );
  let read = read(&body);
  /// Width, height, landscape, columns, and whether the section starts a page.
  type Setup = (Option<f64>, Option<f64>, bool, u32, bool);
  let setup: Vec<Setup> = read
    .sections
    .iter()
    .map(|s| (s.width, s.height, s.landscape, s.columns, s.starts_page))
    .collect();
  assert_eq!(
    setup,
    [
      (Some(612.0), Some(792.0), false, 1, true),
      (Some(792.0), Some(612.0), true, 2, true),
      (Some(595.3), Some(841.9), false, 1, false),
    ]
  );
  assert_eq!(read.sections[0].margins.left, 54.0);
  let sections: Vec<u32> = read.blocks.iter().map(|b| paragraph(b).section).collect();
  assert_eq!(sections, [0, 0, 1, 1, 2]);
  // The second section starts a new page; the third continues on the same one.
  let pages: Vec<u32> = read.blocks.iter().map(|b| paragraph(b).page).collect();
  assert_eq!(pages, [1, 1, 2, 2, 2]);
}

#[test]
fn comments_and_notes_are_referred_to_and_a_comment_knows_the_text_it_marks() {
  let body = r#"<w:p><w:r><w:t>before </w:t></w:r><w:commentRangeStart w:id="2"/><w:r><w:t>marked </w:t></w:r><w:r><w:t>text</w:t></w:r><w:commentRangeEnd w:id="2"/><w:r><w:commentReference w:id="2"/></w:r><w:r><w:t> after</w:t></w:r><w:r><w:footnoteReference w:id="3"/></w:r><w:r><w:footnoteReference w:id="99"/></w:r></w:p>"#;
  let read = read(body);
  assert_eq!(
    read.anchors.get(&2).map(String::as_str),
    Some("marked text")
  );
  let references: Vec<&Inline> = paragraph(&read.blocks[0])
    .inlines
    .iter()
    .filter(|i| matches!(i, Inline::CommentRef(_) | Inline::NoteRef(_)))
    .collect();
  // A reference to a note the document does not have is dropped.
  assert_eq!(references, [&Inline::CommentRef(1), &Inline::NoteRef(2)]);
}

#[test]
fn characters_that_are_elements_of_their_own_become_text() {
  let body = r#"<w:p><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t>c</w:t><w:noBreakHyphen/><w:t>d</w:t><w:softHyphen/><w:sym w:font="Wingdings" w:char="F0FC"/><w:sym w:font="Symbol" w:char="F0B7"/><w:sym w:font="Other" w:char="F123"/><w:ruby><w:rt><w:r><w:t>guide</w:t></w:r></w:rt><w:rubyBase><w:r><w:t>base</w:t></w:r></w:rubyBase></w:ruby></w:r><m:oMath><m:r><m:t>x=1</m:t></m:r></m:oMath></w:p>"#;
  assert_eq!(read(body).blocks[0].text(), "a\tb\nc\u{2011}d✓•□basex=1");
}

#[test]
fn pictures_are_found_with_the_size_they_are_shown_at() {
  let body = r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="1270000" cy="635000"/><wp:docPr id="1" name="Picture 1" descr="A chart"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rId4"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
      <w:r><w:pict><v:shape style="width:72pt;height:36pt" alt="Old"><v:imagedata r:id="rId4"/></v:shape></w:pict></w:r>
      <w:r><w:drawing><wp:anchor><wp:extent cx="12700" cy="12700"/><wp:docPr id="2" name="Picture 2"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:link="rId5"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>"#;
  let read = read_with(
    body,
    Setup {
      rels: &[
        ("rId4", "word/media/image1.png", false),
        ("rId5", "https://example.com/linked.png", true),
      ],
      ..Setup::default()
    },
  )
  .unwrap();
  let uses: Vec<&ImageRef> = paragraph(&read.blocks[0])
    .inlines
    .iter()
    .filter_map(|inline| match inline {
      Inline::Image(image) => Some(image),
      _ => None,
    })
    .collect();
  /// The image, the size it is shown at, and its description.
  type Use<'a> = (u32, Option<f64>, Option<f64>, Option<&'a str>);
  let summary: Vec<Use<'_>> = uses
    .iter()
    .map(|u| {
      (
        u.image,
        u.display_width,
        u.display_height,
        u.alt_text.as_deref(),
      )
    })
    .collect();
  assert_eq!(
    summary,
    [
      (0, Some(100.0), Some(50.0), Some("A chart")),
      // The same picture used again is the same image.
      (0, Some(72.0), Some(36.0), Some("Old")),
      (1, Some(1.0), Some(1.0), None),
    ]
  );
  assert_eq!(uses.iter().map(|u| u.page).collect::<Vec<_>>(), [1, 1, 1]);
  assert_eq!(
    read.pictures,
    [
      ("word/media/image1.png".to_owned(), false),
      ("https://example.com/linked.png".to_owned(), true),
    ]
  );
}

#[test]
fn nesting_past_the_limit_is_a_limit_error() {
  let depth = 40;
  let body = format!(
    "{}{}{}",
    "<w:tbl><w:tr><w:tc>".repeat(depth),
    p("deep"),
    "</w:tc></w:tr></w:tbl>".repeat(depth)
  );
  let error = read_with(&body, Setup::default()).err().unwrap();
  assert!(matches!(error, DocumentError::LimitExceeded(_)), "{error}");
  // A few levels are fine.
  let shallow = format!(
    "{}{}{}",
    "<w:tbl><w:tr><w:tc>".repeat(4),
    p("deep"),
    "</w:tc></w:tr></w:tbl>".repeat(4)
  );
  assert_eq!(read(&shallow).blocks[0].text(), "deep");
}
