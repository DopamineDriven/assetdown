import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { UTR } from "@d0paminedriven/type-utils";

import { WorkupService } from "@/__test__/workup.ts";
import {
  DocxDown,
  docxAnnotations,
  docxAnnotationsAsync,
  docxBlocks,
  docxBlocksAsync,
  docxDocument,
  docxDocumentAsync,
  docxFormFields,
  docxFormFieldsAsync,
  docxImages,
  docxImagesAsync,
  docxInvoice,
  docxInvoiceAsync,
  docxLayout,
  docxLayoutAsync,
  docxMarkdown,
  docxMarkdownAsync,
  docxMetadata,
  docxMetadataAsync,
  docxNotes,
  docxNotesAsync,
  docxTables,
  docxTablesAsync,
  docxText,
  docxTextAsync,
  docxTextPerPage,
  docxTextPerPageAsync
} from "@/index.js";
import type {
  Block,
  DocxAnnotation,
  DocxPage,
  Inline,
  PaperSize
} from "@/index.js";

/** The unions as records keyed by `type`, each member still tied to its key. */
type Blocks = UTR<Block, "type">;
type Inlines = UTR<Inline, "type">;

class DocxTest extends WorkupService {
  /**
   * Each fixture with what its own properties record and what its XML holds when counted by
   * hand: the pages the application recorded, the pages the reader counts from page breaks,
   * and its tables, hyperlinks and pictures.
   */
  public get known() {
    return [
      // Word left no marker for one page of this document, so the count falls one short.
      ["Catullus-and-Lucan-on-Pompey-and-Caesar.docx", 6, 5, 0, 0, 0],
      ["Harvati-2003.docx", 8, 8, 1, 0, 3],
      ["Lucans-Pharsalia-1.129-157.docx", 1, 1, 0, 0, 0],
      ["Muscles-of-mastication.docx", 5, 5, 2, 2, 7],
      ["Muscles-of-the-Skull.docx", 4, 4, 0, 0, 4],
      ["Ongoing-Research-Notes.docx", 9, 9, 2, 0, 0],
      ["Poem-29.docx", 1, 1, 0, 0, 0],
      ["Ramal-Morphology-Articles-Summary.docx", 3, 2, 0, 0, 0],
      ["Summary-of-Mandibular-Ontogenetic-Patterns.docx", 3, 3, 0, 0, 0],
      ["andrew-ross-oct-2026.docx", 3, 3, 0, 3, 0],
      ["fonts-sample-dos-word.docx", 1, 1, 0, 0, 0],
      ["lo-pandoc.docx", 1, 1, 1, 1, 0],
      ["pandoc.docx", 1, 1, 1, 1, 0]
    ] as const;
  }

  /** The item at `index`, failing the test when there is none. */
  public at<const T>(items: readonly T[], index: number) {
    const item = items[index];
    if (item === undefined) {
      assert.fail(`no item at index ${index} of ${items.length}`);
    }
    return item;
  }

  /** Characters that are not white space. */
  public visible(text: string) {
    return [...text.replace(/\s+/gu, "")].length;
  }

  /** Every paragraph under `blocks`, the ones in table cells included, in reading order. */
  public paragraphs(blocks: Block[]) {
    const found = Array.of<Blocks["paragraph"]>();
    for (const block of blocks) {
      if (block.type === "paragraph") {
        found.push(block);
        continue;
      }
      for (const row of block.rows) {
        for (const cell of row.cells) {
          found.push(...this.paragraphs(cell.blocks));
        }
      }
    }
    return found;
  }

  /** Every table under `blocks`, a table inside a cell after the table that holds it. */
  public tables(blocks: Block[]) {
    const found = Array.of<Blocks["table"]>();
    for (const block of blocks) {
      if (block.type !== "table") continue;
      found.push(block);
      for (const row of block.rows) {
        for (const cell of row.cells) {
          found.push(...this.tables(cell.blocks));
        }
      }
    }
    return found;
  }

  /** Every link under `inlines`, a link before the links it holds. */
  public links(inlines: Inline[]) {
    const found = Array.of<Inlines["link"]>();
    for (const inline of inlines) {
      if (inline.type === "link") {
        found.push(inline, ...this.links(inline.inlines));
      }
    }
    return found;
  }

  /** The text of each line of a page, top to bottom. */
  public lines(page: DocxPage) {
    return page.lines.map((line) => line.text);
  }

  /** A workbook, from the package next door: a readable Office file that is not a document. */
  public get workbook() {
    return this.fileToBuffer("../xlsxdown-node/__test__/fixtures/minimal.xlsx");
  }

  /** The first bytes of an OLE compound file, which a legacy `.doc` and an encrypted `.docx` are. */
  public get legacy() {
    return Buffer.concat([
      Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]),
      Buffer.alloc(64)
    ]);
  }
}

const t = new DocxTest();

/** The document pandoc wrote from a page of Markdown and LibreOffice saved again. */
const sample = t.getFixtureBuff("lo-pandoc.docx");
const doc = new DocxDown(sample);
const letter = {
  type: "LETTER",
  width: 612,
  height: 792
} satisfies UTR<PaperSize, "type">["LETTER"];

describe("DocxDown constructor", () => {
  it("should return a DocxDown for a document", {}, () => {
    assert.equal(doc instanceof DocxDown, true);
  });
  it("should open every fixture with nothing left out", {}, () => {
    for (const [name] of t.known) {
      assert.deepEqual(
        new DocxDown(t.getFixtureBuff(name)).metadata().warnings,
        [],
        name
      );
    }
  });
  it(
    "should throw unsupported format for a legacy .doc or an encrypted document, which are OLE files",
    {},
    () => {
      assert.throws(() => new DocxDown(t.legacy), {
        message:
          "unsupported format: OLE compound file: a legacy .doc, or a document protected with a password"
      });
    }
  );
  it(
    "should throw unsupported format naming a workbook for what it is",
    {},
    () => {
      assert.throws(() => new DocxDown(t.workbook), {
        message: "unsupported format: workbook"
      });
    }
  );
  it(
    "should throw malformed document for bytes that are not a package",
    {},
    () => {
      assert.throws(() => new DocxDown(Buffer.from("not a zip at all")), {
        message: /^malformed document: /
      });
      assert.throws(() => new DocxDown(Buffer.alloc(0)), {
        message: /^malformed document: /
      });
    }
  );
  it(
    "should read the same document in either view when it has no tracked changes",
    {},
    () => {
      assert.equal(doc.metadata().hasTrackedChanges, false);
      assert.equal(
        new DocxDown(sample, { view: "original" }).text(),
        doc.text()
      );
      assert.equal(new DocxDown(sample, { view: "final" }).text(), doc.text());
    }
  );
});

describe("DocxDown.openAsync", () => {
  it(
    "should resolve to a DocxDown that reads what the constructor's reads",
    {},
    async () => {
      const opened = await DocxDown.openAsync(sample);
      assert.equal(opened instanceof DocxDown, true);
      assert.deepEqual(opened.metadata(), doc.metadata());
      assert.equal(
        (await DocxDown.openAsync(sample, { view: "original" })).text(),
        doc.text()
      );
    }
  );
  it("should reject with the errors the constructor throws", {}, async () => {
    await assert.rejects(DocxDown.openAsync(t.workbook), {
      message: "unsupported format: workbook"
    });
    await assert.rejects(DocxDown.openAsync(Buffer.from("not a zip at all")), {
      message: /^malformed document: /
    });
  });
});

describe("metadata", () => {
  it(
    "should report the page count the application recorded beside the pages the reader counts",
    {},
    () => {
      for (const [name, recorded, counted] of t.known) {
        const meta = new DocxDown(t.getFixtureBuff(name)).metadata();
        assert.partialDeepStrictEqual(
          meta,
          {
            pageCount: recorded,
            pagesRecorded: recorded,
            pagesCounted: counted
          },
          name
        );
      }
    }
  );
  it(
    "should count tables, links and pictures as the readers list them",
    {},
    () => {
      for (const [name, , , tables, links, pictures] of t.known) {
        const fixture = new DocxDown(t.getFixtureBuff(name));
        const meta = fixture.metadata();
        assert.partialDeepStrictEqual(
          meta,
          {
            tableCount: tables,
            linkCount: links,
            imageCount: pictures,
            commentCount: 0,
            annotationCount: links,
            fieldCount: 0
          },
          name
        );
        assert.equal(fixture.tables().length, meta.tableCount, name);
        assert.equal(fixture.annotations().length, meta.annotationCount, name);
        assert.equal(fixture.images().length, meta.imageCount, name);
        assert.equal(fixture.formFields().length, meta.fieldCount, name);
        assert.equal(
          fixture.notes().length,
          meta.footnoteCount + meta.endnoteCount,
          name
        );
      }
    }
  );
  it("should name the paper each section is nearest to", {}, () => {
    for (const [name] of t.known) {
      const meta = new DocxDown(t.getFixtureBuff(name)).metadata();
      assert.equal(meta.pageSizeUniform, true, name);
      for (const section of meta.sections) {
        assert.partialDeepStrictEqual(
          section,
          { width: 612, height: 792, landscape: false, paperSize: letter },
          name
        );
      }
    }
    // One document has two sections; the second starts where its first block is.
    assert.deepEqual(
      new DocxDown(t.getFixtureBuff("Harvati-2003.docx"))
        .metadata()
        .sections.map((section) => section.firstPage),
      [1, 6]
    );
  });
  it("should return what the document says about itself", {}, () => {
    assert.partialDeepStrictEqual(doc.metadata(), {
      title: "Invoice 4711",
      author: "Trial",
      language: "en-US",
      hasMacros: false,
      isTemplate: false,
      hasTrackedChanges: false,
      paragraphCount: 20,
      headingCount: 1,
      listItemCount: 3,
      footnoteCount: 1,
      endnoteCount: 0
    });
  });
  it(
    "should pass on the counts the application recorded when it saved the file",
    {},
    () => {
      // The essay's docProps/app.xml: Pages 6, Words 1887, Characters 9665, Lines 182, Paragraphs 96.
      assert.partialDeepStrictEqual(
        new DocxDown(
          t.getFixtureBuff("Catullus-and-Lucan-on-Pompey-and-Caesar.docx")
        ).metadata(),
        {
          pagesRecorded: 6,
          wordsRecorded: 1887,
          charactersRecorded: 9665,
          linesRecorded: 182,
          paragraphsRecorded: 96
        }
      );
    }
  );
  it(
    "should count words within two percent of the count Word recorded for plain prose",
    {},
    () => {
      for (const name of [
        "Harvati-2003.docx",
        "Muscles-of-mastication.docx",
        "Ongoing-Research-Notes.docx",
        "Ramal-Morphology-Articles-Summary.docx",
        "Summary-of-Mandibular-Ontogenetic-Patterns.docx"
      ] as const) {
        const { wordCount, wordsRecorded } = new DocxDown(
          t.getFixtureBuff(name)
        ).metadata();
        assert.ok(wordsRecorded, name);
        assert.ok(
          Math.abs(wordCount - wordsRecorded) / wordsRecorded < 0.02,
          `${name}: ${wordCount} counted, ${wordsRecorded} recorded`
        );
      }
    }
  );
});

describe("text and textPerPage", () => {
  it("should return the body in reading order, a row on a line", {}, () => {
    assert.equal(
      doc.text(),
      [
        "Invoice 4711",
        "Trial",
        "Invoice 4711",
        "Bill to Acme Corp, see the terms.",
        "Item\tQty\tUnit price\tTotal",
        "Widget\t2\t10.00\t20.00",
        "Gadget\t1\t5.50\t5.50",
        "1. First step",
        "2. Second step",
        "• nested bullet",
        "Total due: 25.50 USD"
      ].join("\n")
    );
  });
  it("should split the text into the pages the reader counted", {}, () => {
    for (const [name, , counted] of t.known) {
      const fixture = new DocxDown(t.getFixtureBuff(name));
      const pages = fixture.textPerPage();
      assert.deepEqual(
        pages.map((page) => page.page),
        Array.from({ length: counted }, (_, i) => i + 1),
        name
      );
      assert.equal(
        pages.map((page) => page.text).join("\n"),
        fixture.text(),
        name
      );
    }
  });
  it("should leave a page that holds only a picture without text", {}, () => {
    const figures = new DocxDown(t.getFixtureBuff("Harvati-2003.docx"));
    const empty = figures
      .textPerPage()
      .filter((page) => t.visible(page.text) === 0)
      .map((page) => page.page);
    assert.deepEqual(empty, [4]);
    assert.equal(
      figures.images().some((image) => image.page === 4),
      true
    );
  });
});

describe("blocks", () => {
  const blocks = doc.blocks();
  const paragraphs = t.paragraphs(blocks);

  it("should return paragraphs and tables in reading order", {}, () => {
    assert.deepEqual(
      blocks.map((block) => block.type),
      [
        "paragraph",
        "paragraph",
        "paragraph",
        "paragraph",
        "table",
        "paragraph",
        "paragraph",
        "paragraph",
        "paragraph"
      ]
    );
  });
  it(
    "should mark a heading with its level and a list item with its marker",
    {},
    () => {
      assert.deepEqual(
        paragraphs
          .filter((paragraph) => paragraph.heading !== undefined)
          .map((paragraph) => [paragraph.heading, paragraph.text]),
        [[1, "Invoice 4711"]]
      );
      assert.deepEqual(
        paragraphs.flatMap((paragraph) =>
          paragraph.list ? [paragraph.list] : []
        ),
        [
          { level: 0, ordered: true, marker: "1." },
          { level: 0, ordered: true, marker: "2." },
          { level: 1, ordered: false, marker: "•" }
        ]
      );
    }
  );
  it(
    "should carry a paragraph's text beside its inlines, formatting resolved",
    {},
    () => {
      const billTo = t.at(blocks, 3);
      assert.equal(billTo.type, "paragraph");
      if (billTo.type !== "paragraph") return;
      assert.equal(billTo.text, "Bill to Acme Corp, see the terms.");
      assert.deepEqual(
        billTo.inlines.map((inline) => inline.type),
        ["text", "text", "text", "link", "text", "noteRef"]
      );
      const bold = billTo.inlines.flatMap((inline) =>
        inline.type === "text" && inline.bold ? [inline.text] : []
      );
      assert.deepEqual(bold, ["Acme Corp"]);
      const link = {
        type: "link",
        uri: "https://example.com/terms",
        text: "terms",
        page: 1
      } satisfies Partial<Inlines["link"]>;
      assert.partialDeepStrictEqual(t.at(t.links(billTo.inlines), 0), link);
      assert.deepEqual(billTo.inlines.at(-1), {
        type: "noteRef",
        note: 0
      } satisfies Inlines["noteRef"]);
    }
  );
  it("should hold a table's cells as blocks of their own", {}, () => {
    const [table] = t.tables(blocks);
    assert.ok(table);
    assert.deepEqual(
      table.rows.map((row) => row.cells.map((cell) => cell.text)),
      [
        ["Item", "Qty", "Unit price", "Total"],
        ["Widget", "2", "10.00", "20.00"],
        ["Gadget", "1", "5.50", "5.50"]
      ]
    );
    assert.deepEqual(
      t
        .at(table.rows, 1)
        .cells.map((cell) => [cell.column, cell.colSpan, cell.rowSpan]),
      [
        [0, 1, 1],
        [1, 1, 1],
        [2, 1, 1],
        [3, 1, 1]
      ]
    );
    assert.equal(
      t.paragraphs(t.at(t.at(table.rows, 0).cells, 0).blocks).length,
      1
    );
  });
  it("should read text boxes and say which paragraphs sit in one", {}, () => {
    const labelled = t.paragraphs(
      new DocxDown(t.getFixtureBuff("Muscles-of-the-Skull.docx")).blocks()
    );
    assert.equal(labelled.filter((paragraph) => paragraph.textBox).length, 24);
    assert.equal(
      paragraphs.some((paragraph) => paragraph.textBox),
      false
    );
  });
  it("should find a table inside a table's cell", {}, () => {
    const nested = t.tables(
      new DocxDown(t.getFixtureBuff("Ongoing-Research-Notes.docx")).blocks()
    );
    assert.equal(nested.length, 2);
  });
  it(
    "should put every block on a page the reader counted, never going back",
    {},
    () => {
      for (const [name, , counted] of t.known) {
        let last = 1;
        for (const block of new DocxDown(t.getFixtureBuff(name)).blocks()) {
          assert.ok(block.page >= last && block.page <= counted, name);
          last = block.page;
        }
      }
    }
  );
});

describe("markdown", () => {
  it("should return the Markdown the sample was written from", {}, () => {
    assert.equal(
      doc.markdown(),
      [
        "**Invoice 4711**",
        "",
        "Trial",
        "",
        "# Invoice 4711",
        "",
        "Bill to **Acme Corp**, see the [terms](<https://example.com/terms>).[^1]",
        "",
        "| Item | Qty | Unit price | Total |",
        "| --- | --- | --- | --- |",
        "| Widget | 2 | 10.00 | 20.00 |",
        "| Gadget | 1 | 5.50 | 5.50 |",
        "",
        "1. First step",
        "2. Second step",
        "    - nested bullet",
        "",
        "Total due: 25.50 USD",
        "",
        "[^1]: Net thirty days."
      ].join("\n")
    );
  });
  it("should write every fixture without leaving a character out", {}, () => {
    for (const [name] of t.known) {
      const fixture = new DocxDown(t.getFixtureBuff(name));
      // Markdown adds markers and escapes and drops nothing a reader sees.
      assert.ok(
        t.visible(fixture.markdown()) >= t.visible(fixture.text()) * 0.98,
        name
      );
    }
  });
});

describe("tables", () => {
  it("should key a table's cells by row and then column", {}, () => {
    const [table] = doc.tables();
    assert.ok(table);
    assert.partialDeepStrictEqual(table, {
      index: 0,
      page: 1,
      rowCount: 3,
      columnCount: 4,
      merges: []
    });
    assert.equal(table.parent, undefined);
    assert.deepEqual(Object.keys(table.rows), ["1", "2", "3"]);
    assert.deepEqual(Object.keys(table.rows["2"] ?? {}), ["A", "B", "C", "D"]);
    assert.deepEqual(table.rows["2"]?.["D"], {
      ref: "D2",
      row: 1,
      column: 3,
      colSpan: 1,
      rowSpan: 1,
      text: "20.00"
    });
    assert.equal(table.rows["1"]?.["C"]?.text, "Unit price");
    assert.equal(table.rows["4"], undefined);
  });
  it(
    "should list a table inside a cell after the table that holds it",
    {},
    () => {
      const tables = new DocxDown(
        t.getFixtureBuff("Ongoing-Research-Notes.docx")
      ).tables();
      assert.deepEqual(
        tables.map((table) => [table.index, table.parent]),
        [
          [0, undefined],
          [1, 0]
        ]
      );
    }
  );
});

describe("images", () => {
  it(
    "should return each picture where it is placed with its bytes as stored",
    {},
    () => {
      const fixture = new DocxDown(
        t.getFixtureBuff("Muscles-of-mastication.docx")
      );
      const images = fixture.images();
      const counted = fixture.metadata().pagesCounted;
      assert.equal(images.length, 7);
      const counts: Record<string, number> = {};
      const onPage = new Map<number, number>();
      for (const image of images) {
        const mime = image.mime ?? "unknown";
        counts[mime] = (counts[mime] ?? 0) + 1;
        assert.equal(image.part, "body");
        assert.equal(image.linked, false);
        assert.equal(image.name.startsWith("word/media/"), true);
        assert.equal(Buffer.isBuffer(image.data), true);
        const magic = mime === "image/gif" ? "GIF8" : "ÿØÿ";
        assert.equal(
          image.data.subarray(0, magic.length).toString("latin1"),
          magic
        );
        assert.ok((image.width ?? 0) > 0 && (image.height ?? 0) > 0);
        assert.ok(
          (image.displayWidth ?? 0) > 0 && (image.displayHeight ?? 0) > 0
        );
        assert.ok(image.page !== undefined && image.page <= counted);
        // The index counts the pictures of one page.
        const index = onPage.get(image.page) ?? 0;
        assert.equal(image.imageIndex, index);
        onPage.set(image.page, index + 1);
      }
      assert.deepEqual(counts, { "image/jpeg": 6, "image/gif": 1 });
    }
  );
  it("should report a vector picture without a size in pixels", {}, () => {
    const images = new DocxDown(t.getFixtureBuff("Harvati-2003.docx")).images();
    assert.deepEqual(
      images.map((image) => [
        image.page,
        image.mime,
        image.width,
        image.height
      ]),
      [
        [1, "image/emf", undefined, undefined],
        [4, "image/emf", undefined, undefined],
        [5, "image/emf", undefined, undefined]
      ]
    );
  });
});

describe("annotations", () => {
  it(
    "should return a link with where it leads and the text it covers",
    {},
    () => {
      assert.deepEqual(doc.annotations(), [
        {
          page: 1,
          part: "body",
          subtype: "Link",
          uri: "https://example.com/terms",
          text: "terms"
        }
      ] satisfies DocxAnnotation[]);
    }
  );
  it("should list the links of documents written in Word", {}, () => {
    for (const name of [
      "Muscles-of-mastication.docx",
      "andrew-ross-oct-2026.docx"
    ] as const) {
      const fixture = new DocxDown(t.getFixtureBuff(name));
      const counted = fixture.metadata().pagesCounted;
      for (const annotation of fixture.annotations()) {
        assert.equal(annotation.subtype, "Link", name);
        assert.equal(annotation.part, "body", name);
        assert.match(annotation.uri ?? "", /^[a-z]+:/, name);
        assert.ok(annotation.text.trim().length > 0, name);
        assert.ok(
          annotation.page !== undefined && annotation.page <= counted,
          name
        );
      }
    }
  });
  it("should agree with the link inlines of the block tree", {}, () => {
    for (const [name] of t.known) {
      const fixture = new DocxDown(t.getFixtureBuff(name));
      const inTree = t
        .paragraphs(fixture.blocks())
        .flatMap((paragraph) => t.links(paragraph.inlines));
      assert.deepEqual(
        fixture
          .annotations()
          .filter((annotation) => annotation.part === "body")
          .map((annotation) => [
            annotation.uri,
            annotation.dest,
            annotation.text
          ]),
        inTree.map((link) => [link.uri, link.dest, link.text]),
        name
      );
    }
  });
});

describe("formFields and notes", () => {
  it("should return no fields for documents that have none", {}, () => {
    assert.deepEqual(doc.formFields(), []);
  });
  it(
    "should return a footnote with its text and the page that refers to it",
    {},
    () => {
      const [note] = doc.notes();
      assert.ok(note);
      assert.partialDeepStrictEqual(note, { kind: "footnote", page: 1 });
      assert.equal(note.text.trim(), "Net thirty days.");
      assert.equal(t.paragraphs(note.blocks).length > 0, true);
    }
  );
});

describe("layout", () => {
  const [page] = doc.layout();

  it(
    "should set a row of a table on one line and a list item after its marker",
    {},
    () => {
      assert.ok(page);
      assert.partialDeepStrictEqual(page, { page: 1, width: 612, height: 792 });
      assert.deepEqual(t.lines(page), [
        "Invoice 4711",
        "Trial",
        "Invoice 4711",
        "Bill to Acme Corp, see the terms.",
        "Item Qty Unit price Total",
        "Widget 2 10.00 20.00",
        "Gadget 1 5.50 5.50",
        "1. First step",
        "2. Second step",
        "• nested bullet",
        "Total due: 25.50 USD"
      ]);
      assert.deepEqual(page.warnings, [
        "word positions are estimated from the flow of the text; a docx records no layout"
      ]);
    }
  );
  it("should carry bold and the font size on each word", {}, () => {
    assert.ok(page);
    assert.deepEqual(
      page.words
        .filter((word) => word.bold)
        .map((word) => word.text)
        .slice(-2),
      ["Acme", "Corp,"]
    );
    assert.equal(
      page.words.every((word) => (word.fontSize ?? 0) > 0),
      true
    );
  });
  it(
    "should return a page for each one counted with every word on it",
    {},
    () => {
      for (const [name, , counted] of t.known) {
        const fixture = new DocxDown(t.getFixtureBuff(name));
        const pages = fixture.layout();
        assert.deepEqual(
          pages.map((laid) => laid.page),
          Array.from({ length: counted }, (_, i) => i + 1),
          name
        );
        let set = 0;
        for (const laid of pages) {
          assert.ok(laid.width === 612 && laid.height >= 792, name);
          for (const word of laid.words) {
            assert.ok(
              0 <= word.x0 && word.x0 < word.x1 && word.x1 <= laid.width,
              name
            );
            assert.ok(
              0 <= word.y0 && word.y0 < word.y1 && word.y1 <= laid.height,
              name
            );
            set += t.visible(word.text);
          }
        }
        // Every character of the body is set; a header adds its own on each page.
        const body = t.visible(fixture.text());
        const { headerCount, footerCount } = fixture.metadata();
        if (headerCount + footerCount === 0) assert.equal(set, body, name);
        else assert.ok(set >= body, name);
      }
    }
  );
});

describe("invoice", () => {
  it(
    "should read invoice fields from the pages as layout sets them",
    {},
    () => {
      const result = doc.invoice();
      assert.partialDeepStrictEqual(result, {
        source: "Native",
        documentType: "Invoice"
      });
      assert.equal(result.fields.invoiceId?.value, "4711");
      assert.partialDeepStrictEqual(result.fields.amountDue, {
        value: 25.5,
        text: "25.50 USD"
      });
    }
  );
  it("should take the options the other packages take", {}, () => {
    assert.equal(
      doc.invoice({ languages: ["en"], currency: "USD" }).fields.invoiceId
        ?.value,
      "4711"
    );
  });
});

describe("document", () => {
  it(
    "should return what each reader returns, under the reader's own name",
    {},
    () => {
      const whole = doc.document();
      assert.deepEqual(whole.metadata, doc.metadata());
      assert.equal(whole.text, doc.text());
      assert.deepEqual(whole.textPerPage, doc.textPerPage());
      assert.equal(whole.markdown, doc.markdown());
      assert.deepEqual(whole.blocks, doc.blocks());
      assert.deepEqual(whole.tables, doc.tables());
      assert.deepEqual(whole.images, doc.images());
      assert.deepEqual(whole.annotations, doc.annotations());
      assert.deepEqual(whole.formFields, doc.formFields());
      assert.deepEqual(whole.notes, doc.notes());
      assert.deepEqual([whole.headers, whole.footers], [[], []]);
    }
  );
  it("should return a document's header with its text", {}, () => {
    const { headers } = new DocxDown(
      t.getFixtureBuff("Poem-29.docx")
    ).document();
    const [header] = headers;
    assert.ok(header);
    assert.match(header.name, /^word\/header\d+\.xml$/);
    assert.ok(t.visible(header.text) > 0);
    assert.equal(
      t
        .paragraphs(header.blocks)
        .map((paragraph) => paragraph.text)
        .join("\n"),
      header.text
    );
  });
});

describe("async readers", () => {
  const pictures = new DocxDown(
    t.getFixtureBuff("Muscles-of-mastication.docx")
  );

  it("should resolve each to what its sync twin returns", {}, async () => {
    assert.deepEqual(await doc.metadataAsync(), doc.metadata());
    assert.equal(await doc.textAsync(), doc.text());
    assert.deepEqual(await doc.textPerPageAsync(), doc.textPerPage());
    assert.deepEqual(await doc.blocksAsync(), doc.blocks());
    assert.equal(await doc.markdownAsync(), doc.markdown());
    assert.deepEqual(await doc.tablesAsync(), doc.tables());
    assert.deepEqual(await pictures.imagesAsync(), pictures.images());
    assert.deepEqual(await doc.annotationsAsync(), doc.annotations());
    assert.deepEqual(await doc.formFieldsAsync(), doc.formFields());
    assert.deepEqual(await doc.notesAsync(), doc.notes());
    assert.deepEqual(await doc.layoutAsync(), doc.layout());
    assert.deepEqual(await doc.invoiceAsync(), doc.invoice());
    assert.deepEqual(await pictures.documentAsync(), pictures.document());
  });
  it(
    "should return the same blocks to 32 readers running at once",
    {},
    async () => {
      const expected = pictures.blocks();
      const readers = Array.of<Promise<Block[]>>();
      for (let i = 0; i < 16; i++) {
        readers.push(pictures.blocksAsync());
        readers.push(
          docxBlocksAsync(t.getFixtureBuff("Muscles-of-mastication.docx"))
        );
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
});

describe("free functions", () => {
  it("should read a buffer as the same method of a DocxDown does", {}, () => {
    assert.deepEqual(docxMetadata(sample), doc.metadata());
    assert.equal(docxText(sample), doc.text());
    assert.deepEqual(docxTextPerPage(sample), doc.textPerPage());
    assert.deepEqual(docxBlocks(sample), doc.blocks());
    assert.equal(docxMarkdown(sample), doc.markdown());
    assert.deepEqual(docxTables(sample), doc.tables());
    assert.deepEqual(docxImages(sample), doc.images());
    assert.deepEqual(docxAnnotations(sample), doc.annotations());
    assert.deepEqual(docxFormFields(sample), doc.formFields());
    assert.deepEqual(docxNotes(sample), doc.notes());
    assert.deepEqual(docxLayout(sample), doc.layout());
    assert.deepEqual(docxInvoice(sample), doc.invoice());
    assert.deepEqual(docxDocument(sample), doc.document());
  });
  it("should resolve the async forms to the same", {}, async () => {
    assert.deepEqual(await docxMetadataAsync(sample), doc.metadata());
    assert.equal(await docxTextAsync(sample), doc.text());
    assert.deepEqual(await docxTextPerPageAsync(sample), doc.textPerPage());
    assert.deepEqual(await docxBlocksAsync(sample), doc.blocks());
    assert.equal(await docxMarkdownAsync(sample), doc.markdown());
    assert.deepEqual(await docxTablesAsync(sample), doc.tables());
    assert.deepEqual(await docxImagesAsync(sample), doc.images());
    assert.deepEqual(await docxAnnotationsAsync(sample), doc.annotations());
    assert.deepEqual(await docxFormFieldsAsync(sample), doc.formFields());
    assert.deepEqual(await docxNotesAsync(sample), doc.notes());
    assert.deepEqual(await docxLayoutAsync(sample), doc.layout());
    assert.deepEqual(await docxInvoiceAsync(sample), doc.invoice());
    assert.deepEqual(await docxDocumentAsync(sample), doc.document());
  });
  it(
    "should throw from the sync forms and reject from the async ones for a file that is not a document",
    {},
    async () => {
      const message = "unsupported format: workbook";
      assert.throws(() => docxText(t.workbook), { message });
      assert.throws(() => docxInvoice(t.workbook), { message });
      await assert.rejects(docxTextAsync(t.workbook), { message });
      await assert.rejects(docxInvoiceAsync(t.workbook), { message });
      await assert.rejects(docxDocumentAsync(t.workbook), { message });
    }
  );
});
