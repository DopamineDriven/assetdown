use crate::testing::{Setup, document, p};

fn markdown(body: &str) -> String {
  document(body, Setup::default()).markdown()
}

/// A run of `text` with the run properties `properties`.
fn run(properties: &str, text: &str) -> String {
  format!(r#"<w:r><w:rPr>{properties}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r>"#)
}

fn paragraph(runs: &[String]) -> String {
  format!("<w:p>{}</w:p>", runs.concat())
}

#[test]
fn emphasis_keeps_white_space_outside_its_markers_and_like_runs_are_one() {
  let body = paragraph(&[
    run("<w:b/>", "Total: "),
    // The same emphasis in another size is still the same emphasis.
    run(r#"<w:b/><w:sz w:val="30"/>"#, "due"),
    run("", " now, "),
    run("<w:b/><w:i/>", "both"),
    run("<w:i/>", " leaning "),
    run("<w:strike/>", "gone"),
    run("<w:b/>", "   "),
    run("<w:u w:val=\"single\"/>", "plain"),
  ]);
  assert_eq!(
    markdown(&body),
    "**Total: due** now, ***both*** *leaning* ~~gone~~   plain"
  );
}

#[test]
fn what_markdown_would_read_as_markup_is_escaped() {
  let body = [
    p(r"2 * 3 = 6, snake_case, _edge_, [ref], &lt;tag&gt;, C:\dir, `code`, a ~~ b, x|y"),
    p("# not a heading"),
    p("- not a list"),
    p("12. not a list"),
    p("12.5 is a number"),
    p("&gt; not a quote"),
    p("---"),
    p("+1 is a number"),
  ]
  .concat();
  assert_eq!(
    markdown(&body),
    [
      r"2 \* 3 = 6, snake_case, \_edge\_, \[ref\], \<tag>, C:\\dir, \`code\`, a \~\~ b, x|y",
      r"\# not a heading",
      r"\- not a list",
      r"12\. not a list",
      "12.5 is a number",
      r"\> not a quote",
      r"\---",
      "+1 is a number",
    ]
    .join("\n\n")
  );
}

#[test]
fn headings_stand_out_without_bold_and_go_six_deep() {
  let heading = |level: u8, runs: &str| {
    format!(r#"<w:p><w:pPr><w:outlineLvl w:val="{level}"/></w:pPr>{runs}</w:p>"#)
  };
  let body = [
    heading(
      0,
      &[run("<w:b/>", "Bold "), run("<w:b/><w:i/>", "title")].concat(),
    ),
    heading(
      2,
      &[
        run("", "Two"),
        "<w:r><w:br/></w:r>".to_owned(),
        run("", "lines"),
      ]
      .concat(),
    ),
    heading(7, &run("", "Deep")),
    heading(1, ""),
  ]
  .concat();
  assert_eq!(
    markdown(&body),
    "# Bold *title*\n\n### Two lines\n\n###### Deep"
  );
}

const LISTS: &str = r#"<w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:start w:val="3"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%2)"/></w:lvl>
    <w:lvl w:ilvl="2"><w:numFmt w:val="bullet"/><w:lvlText w:val="&#xF0A7;"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#;

#[test]
fn list_items_sit_on_consecutive_lines_indented_by_level() {
  let item = |level: u8, runs: &str| {
    format!(
      r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="1"/></w:numPr></w:pPr>{runs}</w:p>"#
    )
  };
  let body = [
    p("Before"),
    item(0, &run("", "three")),
    item(1, &run("", "letter")),
    item(2, &run("", "bullet")),
    item(
      0,
      &[
        run("", "two"),
        "<w:r><w:br/></w:r>".to_owned(),
        run("", "lines"),
      ]
      .concat(),
    ),
    p("After"),
  ]
  .concat();
  let document = document(
    &body,
    Setup {
      numbering: Some(LISTS),
      ..Setup::default()
    },
  );
  // A number keeps its value. A letter has no Markdown form, so it stays as text.
  assert_eq!(
    document.markdown(),
    "Before\n\n3. three\n    - a) letter\n        - bullet\n4. two\\\n   lines\n\nAfter"
  );
}

#[test]
fn links_pictures_tabs_and_breaks() {
  let body = r#"<w:p><w:r><w:t xml:space="preserve">See </w:t></w:r><w:hyperlink r:id="rId1" w:anchor="part two"><w:r><w:rPr><w:b/></w:rPr><w:t>the site</w:t></w:r></w:hyperlink><w:r><w:t>,</w:t><w:tab/><w:t>or</w:t></w:r><w:hyperlink w:anchor="top"><w:r><w:t xml:space="preserve"> above </w:t></w:r></w:hyperlink><w:hyperlink><w:r><w:t>nowhere</w:t></w:r></w:hyperlink><w:r><w:br/><w:t>next line</w:t></w:r></w:p>
    <w:p><w:r><w:drawing><wp:inline><wp:extent cx="12700" cy="12700"/><wp:docPr id="1" name="p" descr="A [logo]"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rId2"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#;
  let document = document(
    body,
    Setup {
      rels: &[
        ("rId1", "https://example.com/a page", true),
        ("rId2", "word/media/image1.png", false),
      ],
      ..Setup::default()
    },
  );
  assert_eq!(
    document.markdown(),
    "See [**the site**](<https://example.com/a page#part two>), or [above](<#top>) nowhere\\\nnext line\n\n![A \\[logo\\]](<word/media/image1.png>)"
  );
}

#[test]
fn a_table_is_a_pipe_table_with_its_first_row_as_the_header() {
  let cell =
    |properties: &str, inner: &str| format!("<w:tc><w:tcPr>{properties}</w:tcPr>{inner}</w:tc>");
  let body = format!(
    "<w:tbl><w:tr>{}{}</w:tr><w:tr>{}{}{}</w:tr><w:tr>{}{}{}</w:tr></w:tbl>",
    cell(r#"<w:gridSpan w:val="2"/>"#, &p("Wide")),
    cell("", &p("C")),
    cell(r#"<w:vMerge w:val="restart"/>"#, &p("Tall")),
    cell("", &[p("one | two"), p("second")].concat()),
    cell("", &paragraph(&[run("<w:b/>", "bold")])),
    cell("<w:vMerge/>", ""),
    cell(
      "",
      &format!(
        "<w:tbl><w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr><w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>",
        p("a"),
        p("b"),
        p("c"),
        p("d")
      )
    ),
    cell("", ""),
  );
  assert_eq!(
    markdown(&body),
    [
      "| Wide |  | C |",
      "| --- | --- | --- |",
      r"| Tall | one \| two<br>second | **bold** |",
      // A merged cell is empty where it continues, and a table in a cell is flattened.
      r"|  | a \| b<br>c \| d |  |",
    ]
    .join("\n")
  );
}

#[test]
fn notes_follow_the_body_under_the_numbers_that_refer_to_them() {
  let body = r#"<w:p><w:r><w:t>Claim</w:t></w:r><w:r><w:footnoteReference w:id="2"/></w:r><w:r><w:t xml:space="preserve"> and another</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p>"#;
  let document = document(
    body,
    Setup {
      footnotes: &[
        &p("First note."),
        &[p("Second note."), p("Its second paragraph.")].concat(),
      ],
      ..Setup::default()
    },
  );
  assert_eq!(
    document.markdown(),
    "Claim[^2] and another[^1]\n\n[^1]: First note.\n\n[^2]: Second note.\n\n    Its second paragraph."
  );
}
