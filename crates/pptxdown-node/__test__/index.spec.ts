import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { UTR } from "@d0paminedriven/type-utils";

import { WorkupService } from "@/__test__/workup.ts";
import {
  PptxDown,
  pptxAnnotations,
  pptxAnnotationsAsync,
  pptxCharts,
  pptxChartsAsync,
  pptxDocument,
  pptxDocumentAsync,
  pptxImages,
  pptxImagesAsync,
  pptxInvoice,
  pptxInvoiceAsync,
  pptxLayout,
  pptxLayoutAsync,
  pptxMarkdown,
  pptxMarkdownAsync,
  pptxMedia,
  pptxMediaAsync,
  pptxMetadata,
  pptxMetadataAsync,
  pptxNotes,
  pptxNotesAsync,
  pptxSlides,
  pptxSlidesAsync,
  pptxStructuredText,
  pptxStructuredTextAsync,
  pptxTables,
  pptxTablesAsync,
  pptxText,
  pptxTextAsync,
  pptxTextPerPage,
  pptxTextPerPageAsync
} from "@/index.js";
import type { Paragraph, PptxPage, PptxSlide, Shape } from "@/index.js";

/** The union as a record keyed by `type`, each member still tied to its key. */
type Shapes = UTR<Shape, "type">;

class PptxTest extends WorkupService {
  /**
   * Each fixture with what its XML holds when counted by hand: its slides, the size of a slide
   * in points and the usual words for its shape, the two standard paper sizes that size lies
   * between by area, the title of each slide that has a title placeholder, the pictures on each
   * slide, how many picture parts those are and in which formats.
   *
   * Neither deck holds a table, a link, a comment, a chart, SmartArt, a group or a clip. What
   * JavaScript is given for those is checked in the binding's Rust tests, on decks put together
   * as data.
   */
  public get known() {
    return [
      {
        name: "Activity-7.pptx",
        slides: 4,
        width: 720,
        height: 540,
        ratio: "4:3",
        between: ["B5", "EXECUTIVE"],
        // Three slides type their heading into a text box; the last has a title.
        titles: [undefined, undefined, undefined, "Point Group Table"],
        layouts: ["Title Slide", "Title and Content"],
        pictures: [0, 4, 4, 1],
        pictureParts: 9,
        formats: ["image/jpeg", "image/tiff"]
      },
      {
        name: "HMEM-Horizon-Mediated-Episodic-Memory.pptx",
        slides: 13,
        width: 960,
        height: 540,
        ratio: "16:9",
        between: ["A4", "LEGAL"],
        titles: [
          "Horizon-Mediated Episodic Memory",
          "The cost of long conversations",
          // The third slide has no title placeholder: its large figure is a text box.
          undefined,
          "Three horizons",
          "One write path, three read paths",
          "The write path",
          "The watermark",
          "The summarizer roster",
          "No fold editor",
          "Substitution assembly",
          "Deliberate retrieval",
          "Bounded and self-healing",
          "The case for HMEM"
        ],
        layouts: ["Title and body"],
        pictures: [1, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 1],
        pictureParts: 5,
        formats: ["image/png"]
      }
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

  /** The sum of `numbers`. */
  public sum(numbers: readonly number[]) {
    return numbers.reduce((total, n) => total + n, 0);
  }

  /** Characters that are not white space. */
  public visible(text: string) {
    return [...text.replace(/\s+/gu, "")].length;
  }

  /** Every shape under `shapes`, a group or a diagram before the shapes it holds. */
  public flat(shapes: Shape[]) {
    const found = Array.of<Shape>();
    for (const shape of shapes) {
      found.push(shape);
      if (shape.type === "group" || shape.type === "diagram") {
        found.push(...this.flat(shape.shapes));
      }
    }
    return found;
  }

  /** Every text shape under `shapes`, in reading order. */
  public texts(shapes: Shape[]) {
    const found = Array.of<Shapes["text"]>();
    for (const shape of this.flat(shapes)) {
      if (shape.type === "text") found.push(shape);
    }
    return found;
  }

  /** Every picture under `shapes`, in reading order. */
  public pictures(shapes: Shape[]) {
    const found = Array.of<Shapes["picture"]>();
    for (const shape of this.flat(shapes)) {
      if (shape.type === "picture") found.push(shape);
    }
    return found;
  }

  /** Every paragraph under `shapes`: the text shapes' and the table cells', in reading order. */
  public paragraphs(shapes: Shape[]) {
    const found = Array.of<Paragraph>();
    for (const shape of this.flat(shapes)) {
      if (shape.type === "text") found.push(...shape.paragraphs);
      if (shape.type === "table") {
        for (const row of shape.rows) {
          for (const cell of row.cells) found.push(...cell.paragraphs);
        }
      }
    }
    return found;
  }

  /** The text of each line of a page, top to bottom. */
  public lines(page: PptxPage) {
    return page.lines.map((line) => line.text);
  }

  /** The format a picture's first bytes say it is. */
  public sniff(data: Buffer) {
    const starts = (...bytes: number[]) =>
      data.subarray(0, bytes.length).equals(Buffer.from(bytes));
    if (starts(0x89, 0x50, 0x4e, 0x47)) return "image/png";
    if (starts(0xff, 0xd8, 0xff)) return "image/jpeg";
    if (starts(0x49, 0x49, 0x2a, 0x00) || starts(0x4d, 0x4d, 0x00, 0x2a)) {
      return "image/tiff";
    }
    return undefined;
  }

  /** A workbook, from the package next door: a readable Office file that is not a deck. */
  public get workbook() {
    return this.fileToBuffer("../xlsxdown-node/__test__/fixtures/minimal.xlsx");
  }

  /** A Word document, from the other package next door. */
  public get wordDocument() {
    return this.fileToBuffer(
      "../docxdown-node/__test__/fixtures/lo-pandoc.docx"
    );
  }

  /** The first bytes of an OLE compound file, which a legacy `.ppt` and an encrypted `.pptx` are. */
  public get legacy() {
    return Buffer.concat([
      Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]),
      Buffer.alloc(64)
    ]);
  }
}

const t = new PptxTest();

/** Four slides from 2014, 4:3: long text boxes with numbered steps, and pictures. */
const older = t.getFixtureBuff("Activity-7.pptx");
const activity = new PptxDown(older);
/** Thirteen slides, 16:9: a title on all but one, many small text boxes, a few pictures. */
const wide = t.getFixtureBuff("HMEM-Horizon-Mediated-Episodic-Memory.pptx");
const deck = new PptxDown(wide);

describe("PptxDown constructor", () => {
  it("should return a PptxDown for a deck", {}, () => {
    assert.equal(deck instanceof PptxDown, true);
  });
  it("should open every fixture with nothing left out", {}, () => {
    for (const { name } of t.known) {
      assert.deepEqual(
        new PptxDown(t.getFixtureBuff(name)).metadata().warnings,
        [],
        name
      );
    }
  });
  it(
    "should throw unsupported format for a legacy .ppt or an encrypted deck, which are OLE files",
    {},
    () => {
      assert.throws(() => new PptxDown(t.legacy), {
        message:
          "unsupported format: OLE compound file: a legacy .ppt, or a presentation protected with a password"
      });
    }
  );
  it(
    "should throw unsupported format naming a workbook or a Word document for what it is",
    {},
    () => {
      assert.throws(() => new PptxDown(t.workbook), {
        message: "unsupported format: workbook"
      });
      assert.throws(() => new PptxDown(t.wordDocument), {
        message: "unsupported format: Word document package"
      });
    }
  );
  it(
    "should throw malformed presentation for bytes that are not a package",
    {},
    () => {
      assert.throws(() => new PptxDown(Buffer.from("not a zip at all")), {
        message: /^malformed presentation: /
      });
      assert.throws(() => new PptxDown(Buffer.alloc(0)), {
        message: /^malformed presentation: /
      });
    }
  );
  it(
    "should read the same deck with or without hidden slides when it hides none",
    {},
    () => {
      assert.equal(deck.metadata().hiddenSlideCount, 0);
      assert.equal(
        new PptxDown(wide, { hiddenSlides: false }).text(),
        deck.text()
      );
      assert.equal(
        new PptxDown(wide, { hiddenSlides: true }).text(),
        deck.text()
      );
    }
  );
});

describe("PptxDown.openAsync", () => {
  it(
    "should resolve to a PptxDown that reads what the constructor's reads",
    {},
    async () => {
      const opened = await PptxDown.openAsync(wide);
      assert.equal(opened instanceof PptxDown, true);
      assert.deepEqual(opened.metadata(), deck.metadata());
      assert.equal(
        (await PptxDown.openAsync(wide, { hiddenSlides: false })).text(),
        deck.text()
      );
    }
  );
  it("should reject with the errors the constructor throws", {}, async () => {
    await assert.rejects(PptxDown.openAsync(t.workbook), {
      message: "unsupported format: workbook"
    });
    await assert.rejects(PptxDown.openAsync(Buffer.from("not a zip at all")), {
      message: /^malformed presentation: /
    });
  });
});

describe("metadata", () => {
  it(
    "should count the slides the deck lists, which is the count the application recorded",
    {},
    () => {
      for (const { name, slides } of t.known) {
        assert.partialDeepStrictEqual(
          new PptxDown(t.getFixtureBuff(name)).metadata(),
          {
            slideCount: slides,
            slidesRecorded: slides,
            hiddenSlideCount: 0,
            hiddenSlidesRecorded: 0
          },
          name
        );
      }
    }
  );
  it("should count what the readers list", {}, () => {
    for (const { name, pictures } of t.known) {
      const fixture = new PptxDown(t.getFixtureBuff(name));
      const meta = fixture.metadata();
      assert.partialDeepStrictEqual(
        meta,
        {
          imageCount: t.sum(pictures),
          tableCount: 0,
          linkCount: 0,
          commentCount: 0,
          annotationCount: 0,
          chartCount: 0,
          diagramCount: 0,
          mediaCount: 0,
          notesCount: 0
        },
        name
      );
      assert.equal(fixture.images().length, meta.imageCount, name);
      assert.deepEqual(fixture.tables(), [], name);
      assert.deepEqual(fixture.annotations(), [], name);
      assert.deepEqual(fixture.charts(), [], name);
      assert.deepEqual(fixture.media(), [], name);
      assert.deepEqual(fixture.notes(), [], name);
    }
  });
  it(
    "should give the slide's size, its shape, and the paper sizes it lies between",
    {},
    () => {
      for (const { name, width, height, ratio, between } of t.known) {
        const meta = new PptxDown(t.getFixtureBuff(name)).metadata();
        assert.partialDeepStrictEqual(
          meta,
          { width, height, aspectRatio: ratio },
          name
        );
        // A slide sized for a screen is no paper size.
        const paper = meta.paperSize;
        if (paper.type !== "UNKNOWN") {
          assert.fail(`${name} is ${paper.type}`);
        }
        assert.deepEqual(
          paper.closestTo.map((size) => size?.type),
          between,
          name
        );
      }
    }
  );
  it(
    "should say what a deck is sized for when the file says, in the file's word",
    {},
    () => {
      assert.equal(activity.metadata().sizeKind, "screen4x3");
      // A size the file gives no word for.
      assert.equal(deck.metadata().sizeKind, undefined);
    }
  );
  it("should return what the deck says about itself", {}, () => {
    assert.partialDeepStrictEqual(deck.metadata(), {
      title: "HMEM: Horizon-Mediated Episodic Memory",
      language: "en-US",
      application: "Microsoft Office PowerPoint",
      appVersion: "16.0000",
      presentationFormat: "On-screen Show (16:9)",
      revision: "1",
      created: "2026-11-16T06:58:16Z",
      hasMacros: false,
      isTemplate: false,
      isShow: false,
      layouts: ["Title and body"],
      sections: []
    });
    assert.partialDeepStrictEqual(activity.metadata(), {
      title: "PowerPoint Presentation",
      application: "Microsoft Office PowerPoint",
      appVersion: "14.0000",
      presentationFormat: "On-screen Show (4:3)",
      revision: "4",
      created: "2014-02-06T14:55:10Z",
      layouts: ["Title Slide", "Title and Content"]
    });
  });
  it(
    "should pass on the counts the application recorded beside its own",
    {},
    () => {
      // The application's counts follow rules of its own, and are whatever it last wrote.
      assert.partialDeepStrictEqual(activity.metadata(), {
        wordsRecorded: 32,
        paragraphsRecorded: 39,
        notesRecorded: 1
      });
      // This deck's properties record no words at all, and a notes page for every slide.
      assert.partialDeepStrictEqual(deck.metadata(), {
        wordsRecorded: 0,
        paragraphsRecorded: 0,
        notesRecorded: 13
      });
      assert.equal(deck.metadata().wordCount > 0, true);
    }
  );
  it("should count words and characters from the text it reads", {}, () => {
    for (const { name } of t.known) {
      const fixture = new PptxDown(t.getFixtureBuff(name));
      const meta = fixture.metadata();
      // A paragraph's own text: its bullet or its number is not one of its words.
      const texts = fixture
        .slides()
        .flatMap((slide) => t.paragraphs(slide.shapes))
        .map((paragraph) => paragraph.text)
        .filter((text) => text.trim().length > 0);
      assert.equal(meta.paragraphCount, texts.length, name);
      assert.equal(
        meta.characterCount,
        t.sum(texts.map((text) => t.visible(text))),
        name
      );
      assert.equal(
        meta.wordCount,
        t.sum(
          texts.map(
            (text) =>
              text.split(/\s+/u).filter((word) => word.length > 0).length
          )
        ),
        name
      );
    }
  });
});

describe("text", () => {
  it(
    "should read a slide's title first, then the rest from the top",
    {},
    () => {
      // On the slide the label sits above the title; the title is read first all the same.
      const first = t.at(deck.textPerPage(), 0);
      assert.equal(first.page, 1);
      assert.deepEqual(first.text.split("\n"), [
        "Horizon-Mediated Episodic Memory",
        "CONVERSATION MEMORY SUBSYSTEM",
        "HMEM turns every conversation into a self-condensing archive.",
        "Live in apps/ws-server"
      ]);
    }
  );
  it(
    "should put a number in front of a numbered paragraph, counted as the slide shows it",
    {},
    () => {
      // The slide types its first step as text and numbers the rest. Two of the numbered
      // paragraphs are empty: they show no number and take none.
      const lines = t.at(activity.textPerPage(), 1).text.split("\n");
      const steps = lines.filter((line) => /^\s*\d\./u.test(line));
      assert.deepEqual(
        steps.map((line) => line.trim().slice(0, 12)),
        ["1.  Using th", "2. Each tetr", "3. Using the", "4. Using the"]
      );
      assert.equal(
        lines.at(-1),
        "4. Using the chart given at the end of this activity to determine the point group."
      );
    }
  );
  it("should be the slides' text with a blank line between slides", {}, () => {
    for (const { name, slides } of t.known) {
      const fixture = new PptxDown(t.getFixtureBuff(name));
      const pages = fixture.textPerPage();
      assert.deepEqual(
        pages.map((page) => page.page),
        Array.from({ length: slides }, (_, at) => at + 1),
        name
      );
      assert.equal(
        fixture.text(),
        pages.map((page) => page.text).join("\n\n"),
        name
      );
    }
  });
  it("should give each slide the text its shapes hold", {}, () => {
    for (const { name } of t.known) {
      const fixture = new PptxDown(t.getFixtureBuff(name));
      const pages = fixture.textPerPage();
      for (const slide of fixture.slides()) {
        const page = t.at(pages, slide.page - 1);
        assert.equal(slide.text, page.text, `${name} slide ${slide.page}`);
        // Neither deck has a date, a footer or a slide number on a slide, so the text of a
        // slide is the text of its shapes.
        assert.equal(
          t.visible(slide.text),
          t.sum(
            slide.shapes.map((shape) =>
              shape.type === "text" ? t.visible(shape.text) : 0
            )
          ),
          `${name} slide ${slide.page}`
        );
      }
    }
  });
});

describe("structuredText", () => {
  it(
    "should put all of a slide in its body when nothing on it repeats",
    {},
    () => {
      for (const { name } of t.known) {
        const fixture = new PptxDown(t.getFixtureBuff(name));
        assert.deepEqual(
          fixture.structuredText(),
          fixture.textPerPage().map(({ page, text }) => ({
            page,
            header: "",
            body: text,
            footer: ""
          })),
          name
        );
      }
    }
  );
});

describe("slides", () => {
  it(
    "should name each slide by its title placeholder and the layout it is built on",
    {},
    () => {
      for (const { name, titles, layouts } of t.known) {
        const fixture = new PptxDown(t.getFixtureBuff(name));
        const slides = fixture.slides();
        assert.deepEqual(
          slides.map((slide) => slide.title),
          titles,
          name
        );
        assert.deepEqual(
          slides.map((slide) => [slide.page, slide.name, slide.hidden]),
          titles.map((_, at) => [
            at + 1,
            `ppt/slides/slide${at + 1}.xml`,
            false
          ]),
          name
        );
        assert.deepEqual(fixture.metadata().layouts, layouts, name);
        // The layouts are met in the order of the slides that use them.
        assert.deepEqual(
          [...new Set(slides.map((slide) => slide.layout))],
          layouts,
          name
        );
      }
    }
  );
  it(
    "should list a slide's shapes title first, then from the top, each in its own box",
    {},
    () => {
      const first = t.at(deck.slides(), 0);
      // As the file has them, in points from the slide's top-left corner: the title is the
      // fourth thing stacked and sits under the label, and the picture is the first.
      assert.deepEqual(
        first.shapes.map((shape) => ({
          type: shape.type,
          placeholder: shape.placeholder,
          shapeName: shape.shapeName,
          stacking: shape.stacking,
          y: shape.frame?.y
        })),
        [
          {
            type: "text",
            placeholder: "title",
            shapeName: "Text 2",
            stacking: 3,
            y: 94.4
          },
          {
            type: "text",
            placeholder: undefined,
            shapeName: "Text 1",
            stacking: 2,
            y: 64
          },
          {
            type: "text",
            placeholder: undefined,
            shapeName: "Text 3",
            stacking: 4,
            y: 219.6
          },
          {
            type: "picture",
            placeholder: undefined,
            shapeName: "Image 0",
            stacking: 0,
            y: 320
          },
          {
            type: "text",
            placeholder: undefined,
            shapeName: "Text 4",
            stacking: 5,
            y: 420
          }
        ]
      );
      assert.deepEqual(t.at(first.shapes, 0).frame, {
        x: 64,
        y: 94.4,
        width: 618,
        height: 111.2,
        rotation: 0
      });
    }
  );
  it(
    "should give a placeholder that states no box the one its layout or master gives it",
    {},
    () => {
      // The last slide's title says only that it is a title. Its body placeholder is empty
      // and is not listed, so the picture is third in the stack and second in the list.
      const last = t.at(activity.slides(), 3);
      assert.deepEqual(
        last.shapes.map((shape) => [
          shape.type,
          shape.placeholder,
          shape.shapeName,
          shape.stacking
        ]),
        [
          ["text", "title", "Title 1", 0],
          ["picture", undefined, "Picture 7", 2]
        ]
      );
      const title = t.at(last.shapes, 0).frame;
      assert.ok(title);
      assert.equal(title.width > 0 && title.height > 0, true);
      assert.equal(title.x >= 0 && title.x + title.width <= 720, true);
      assert.equal(title.y >= 0 && title.y + title.height <= 540, true);
      // The picture states its own: 8504016 by 4343400 EMU, 12,700 to the point.
      assert.partialDeepStrictEqual(t.at(last.shapes, 1).frame, {
        y: 156,
        height: 342
      });
    }
  );
  it("should give every shape a box that is on the slide", {}, () => {
    for (const { name, width, height } of t.known) {
      for (const slide of new PptxDown(t.getFixtureBuff(name)).slides()) {
        for (const shape of slide.shapes) {
          const frame = shape.frame;
          assert.ok(frame, `${name} slide ${slide.page}`);
          assert.equal(frame.width > 0 && frame.height > 0, true);
          // A shape may hang over an edge, but some of it is on the slide.
          assert.equal(
            frame.x < width && frame.x + frame.width > 0,
            true,
            `${name} slide ${slide.page}`
          );
          assert.equal(
            frame.y < height && frame.y + frame.height > 0,
            true,
            `${name} slide ${slide.page}`
          );
        }
      }
    }
  });
  it(
    "should give a text shape its paragraphs, a numbered one with its number",
    {},
    () => {
      const second = t.at(activity.slides(), 1);
      const steps = t.at(t.texts(second.shapes), 0);
      // Nine paragraphs. Five are numbered, and say they start at 2, 2, 3, 3 and 3; the two
      // of them that are empty show no number, so the three with text are 2, 3 and 4.
      assert.equal(steps.paragraphs.length, 9);
      assert.deepEqual(
        steps.paragraphs.map(({ bullet, ordered, text }) => [
          bullet,
          ordered,
          text.trim().length > 0
        ]),
        [
          [undefined, false, true],
          [undefined, false, false],
          [undefined, false, true],
          [undefined, false, false],
          ["2.", true, true],
          [undefined, false, false],
          ["3.", true, true],
          [undefined, false, false],
          ["4.", true, true]
        ]
      );
      // A shape's text is its paragraphs as lines, each with its number in front.
      assert.equal(
        steps.text,
        steps.paragraphs
          .map((paragraph) =>
            paragraph.bullet === undefined
              ? paragraph.text
              : `${paragraph.bullet} ${paragraph.text}`
          )
          .join("\n")
      );
      // A paragraph's text is its runs'.
      for (const paragraph of steps.paragraphs) {
        assert.equal(
          paragraph.text,
          paragraph.runs.map((run) => run.text).join("")
        );
      }
    }
  );
  it("should give a run the size and weight the file states", {}, () => {
    const first = t.at(deck.slides(), 0);
    const [title, label] = t.texts(first.shapes);
    assert.ok(title);
    assert.ok(label);
    const sizes = (shape: Shapes["text"]) =>
      shape.paragraphs.flatMap((paragraph) =>
        paragraph.runs.map((run) => [run.size, run.bold])
      );
    assert.deepEqual(sizes(title), [[52, true]]);
    assert.deepEqual(sizes(label), [[12, true]]);
  });
  it(
    "should list every picture of a slide, by the name images() gives it",
    {},
    () => {
      for (const { name, pictures } of t.known) {
        const fixture = new PptxDown(t.getFixtureBuff(name));
        const images = fixture.images();
        const slides = fixture.slides();
        assert.deepEqual(
          slides.map((slide) => t.pictures(slide.shapes).length),
          pictures,
          name
        );
        for (const slide of slides) {
          assert.deepEqual(
            t.pictures(slide.shapes).map((picture) => picture.name),
            images
              .filter((image) => image.page === slide.page)
              .map((image) => image.name),
            `${name} slide ${slide.page}`
          );
        }
      }
    }
  );
  it("should keep the order the file stacks a slide's shapes in", {}, () => {
    for (const { name } of t.known) {
      for (const slide of new PptxDown(t.getFixtureBuff(name)).slides()) {
        // Back to front, from 0, with no place used twice. A place is skipped where the file
        // stacks something the reader does not list, such as a shape with no text in it.
        const stacking = slide.shapes.map((shape) => shape.stacking);
        assert.equal(
          stacking.every((place) => Number.isInteger(place) && place >= 0),
          true,
          `${name} slide ${slide.page}`
        );
        assert.equal(
          new Set(stacking).size,
          stacking.length,
          `${name} slide ${slide.page}`
        );
      }
    }
  });
});

describe("markdown", () => {
  it(
    "should write a heading for each slide: its title, or its number when it has none",
    {},
    () => {
      for (const { name, titles } of t.known) {
        const markdown = new PptxDown(t.getFixtureBuff(name)).markdown();
        assert.deepEqual(
          markdown.split("\n").filter((line) => line.startsWith("## ")),
          titles.map((title, at) => `## ${title ?? `Slide ${at + 1}`}`),
          name
        );
      }
    }
  );
  it(
    "should write a slide's text under its heading, bold where it is bold",
    {},
    () => {
      assert.deepEqual(deck.markdown().split("\n").slice(0, 5), [
        "## Horizon-Mediated Episodic Memory",
        "",
        "**CONVERSATION MEMORY SUBSYSTEM**",
        "",
        "HMEM turns every conversation into a self-condensing archive."
      ]);
    }
  );
  it(
    "should write a numbered paragraph as a list item with its number",
    {},
    () => {
      const lines = activity.markdown().split("\n");
      assert.equal(
        lines.includes(
          "4. Using the chart given at the end of this activity to determine the point group."
        ),
        true
      );
      assert.deepEqual(
        lines
          .filter((line) => /^\d+\. /u.test(line))
          .map((line) => line.slice(0, 2)),
        // The second and the third slide each number their steps 2, 3 and 4.
        ["2.", "3.", "4.", "2.", "3.", "4."]
      );
    }
  );
  it("should write a picture by its description and its part", {}, () => {
    assert.equal(
      deck.markdown().includes("![ellipse](<ppt/media/image-1-1.png>)"),
      true
    );
  });
});

describe("images", () => {
  it("should return each place a picture is used, slide by slide", {}, () => {
    for (const { name, pictures, pictureParts } of t.known) {
      const images = new PptxDown(t.getFixtureBuff(name)).images();
      assert.deepEqual(
        pictures.map(
          (_, at) => images.filter((image) => image.page === at + 1).length
        ),
        pictures,
        name
      );
      assert.equal(
        new Set(images.map((image) => image.name)).size,
        pictureParts,
        name
      );
      // Its place among the pictures of its slide counts from 0.
      for (const [at, count] of pictures.entries()) {
        assert.deepEqual(
          images
            .filter((image) => image.page === at + 1)
            .map((image) => image.imageIndex),
          Array.from({ length: count }, (_, index) => index),
          `${name} slide ${at + 1}`
        );
      }
    }
  });
  it(
    "should return the bytes as stored, in the format their first bytes say",
    {},
    () => {
      for (const { name, formats } of t.known) {
        const images = new PptxDown(t.getFixtureBuff(name)).images();
        for (const image of images) {
          assert.equal(image.name.startsWith("ppt/media/"), true, name);
          assert.equal(image.linked, false, name);
          assert.equal(image.background, false, name);
          assert.equal(image.data.length > 0, true, image.name);
          assert.equal(image.mime, t.sniff(image.data), image.name);
        }
        assert.deepEqual(
          [...new Set(images.map((image) => image.mime))].toSorted(),
          formats,
          name
        );
      }
    }
  );
  it("should read a PNG's size in pixels from its own header", {}, () => {
    for (const image of deck.images()) {
      // A PNG states its width and its height in the eight bytes after its first chunk's name.
      assert.equal(image.width, image.data.readUInt32BE(16), image.name);
      assert.equal(image.height, image.data.readUInt32BE(20), image.name);
    }
  });
  it("should give a picture its box and its description", {}, () => {
    // 2032000 EMU square, 8940800 across and 4064000 down.
    assert.partialDeepStrictEqual(t.at(deck.images(), 0), {
      page: 1,
      imageIndex: 0,
      altText: "ellipse",
      frame: { x: 704, y: 320, width: 160, height: 160, rotation: 0 }
    });
    for (const { name, width, height } of t.known) {
      for (const image of new PptxDown(t.getFixtureBuff(name)).images()) {
        const frame = image.frame;
        assert.ok(frame, image.name);
        assert.equal(frame.width > 0 && frame.height > 0, true, image.name);
        // A picture may hang over an edge, but some of it is on the slide.
        assert.equal(frame.x < width && frame.x + frame.width > 0, true, name);
        assert.equal(
          frame.y < height && frame.y + frame.height > 0,
          true,
          name
        );
      }
    }
  });
});

describe("notes", () => {
  it(
    "should return no notes for a notes page that holds only a slide number",
    {},
    () => {
      // The third slide of this deck has a notes page, and nothing is written on it.
      assert.equal(activity.metadata().notesRecorded, 1);
      assert.deepEqual(activity.notes(), []);
      assert.equal(t.at(activity.slides(), 2).notes, undefined);
    }
  );
});

describe("layout", () => {
  it("should return a page for each slide, of the slide's size", {}, () => {
    for (const { name, slides, width, height } of t.known) {
      const pages = new PptxDown(t.getFixtureBuff(name)).layout();
      assert.deepEqual(
        pages.map((page) => [page.page, page.width, page.height]),
        Array.from({ length: slides }, (_, at) => [at + 1, width, height]),
        name
      );
    }
  });
  it("should set every word and every picture inside the slide", {}, () => {
    for (const { name, pictures } of t.known) {
      const pages = new PptxDown(t.getFixtureBuff(name)).layout();
      assert.deepEqual(
        pages.map((page) => page.images.length),
        pictures,
        name
      );
      for (const page of pages) {
        for (const box of [...page.words, ...page.images]) {
          assert.equal(
            0 <= box.x0 && box.x0 < box.x1 && box.x1 <= page.width,
            true,
            `${name} slide ${page.page}`
          );
          assert.equal(
            0 <= box.y0 && box.y0 < box.y1 && box.y1 <= page.height,
            true,
            `${name} slide ${page.page}`
          );
        }
      }
    }
  });
  it(
    "should set the words it reads, bullets and all, and group them in lines",
    {},
    () => {
      for (const { name } of t.known) {
        const fixture = new PptxDown(t.getFixtureBuff(name));
        const slides = fixture.slides();
        for (const page of fixture.layout()) {
          const slide = t.at(slides, page.page - 1);
          assert.equal(
            t.sum(page.words.map((word) => t.visible(word.text))),
            t.visible(slide.text),
            `${name} slide ${page.page}`
          );
          // A line is a run of the page's words, and the lines cover all of them in order.
          let next = 0;
          for (const [at, line] of page.lines.entries()) {
            assert.equal(line.firstWord, next, `${name} slide ${page.page}`);
            const words = page.words.slice(next, next + line.wordCount);
            assert.equal(
              words.every((word) => word.line === at),
              true
            );
            assert.equal(line.text, words.map((word) => word.text).join(" "));
            next += line.wordCount;
          }
          assert.equal(next, page.words.length, `${name} slide ${page.page}`);
        }
      }
    }
  );
  it(
    "should set a word where its box is, at the size the file gives it",
    {},
    () => {
      const page = t.at(deck.layout(), 0);
      // The label is the topmost thing on the slide: a box 64 points in and 64 down, with no
      // space kept inside it, and bold text of 12 points. A page is measured up from its
      // bottom edge, and a line is 1.2 times its size tall.
      assert.equal(t.at(t.lines(page), 0), "CONVERSATION MEMORY SUBSYSTEM");
      assert.partialDeepStrictEqual(t.at(page.words, 0), {
        text: "CONVERSATION",
        x0: 64,
        y1: 540 - 64,
        y0: 540 - 64 - 14.4,
        fontSize: 12,
        bold: true,
        line: 0
      });
      // The title is under it, at 52 points.
      const title = page.words.find((word) => word.text === "Horizon-Mediated");
      assert.ok(title);
      assert.equal(title.fontSize, 52);
      assert.equal(title.y1 < 540 - 64 - 14.4, true);
      assert.deepEqual(page.warnings, [
        "the boxes are the file's own; where lines wrap inside a box is estimated"
      ]);
      // The picture's box: 160 points square, 704 across and 320 down.
      assert.deepEqual(page.images, [
        {
          name: "ppt/media/image-1-1.png",
          x0: 704,
          y0: 540 - 480,
          x1: 864,
          y1: 540 - 320
        }
      ]);
    }
  );
});

describe("invoice", () => {
  it("should find no invoice in a deck that is not one", {}, () => {
    const invoice = deck.invoice();
    assert.equal(invoice.fields.invoiceId, undefined);
    assert.equal(invoice.fields.amountDue, undefined);
    assert.equal(invoice.fields.invoiceTotal, undefined);
    assert.equal(invoice.embeddedXml, undefined);
  });
  it("should take the same options as the other packages", {}, () => {
    assert.equal(
      deck.invoice({ languages: ["en"], currency: "USD" }).fields.invoiceId,
      undefined
    );
    assert.deepEqual(pptxInvoice(wide), deck.invoice());
  });
});

describe("document", () => {
  it(
    "should return what each reader returns, under the reader's name",
    {},
    () => {
      for (const { name } of t.known) {
        const fixture = new PptxDown(t.getFixtureBuff(name));
        assert.deepEqual(
          fixture.document(),
          {
            metadata: fixture.metadata(),
            text: fixture.text(),
            textPerPage: fixture.textPerPage(),
            structuredText: fixture.structuredText(),
            markdown: fixture.markdown(),
            slides: fixture.slides(),
            tables: fixture.tables(),
            images: fixture.images(),
            annotations: fixture.annotations(),
            notes: fixture.notes(),
            charts: fixture.charts(),
            media: fixture.media()
          },
          name
        );
      }
    }
  );
});

describe("free functions", () => {
  it("should return what the class returns for the same buffer", {}, () => {
    assert.deepEqual(pptxMetadata(wide), deck.metadata());
    assert.equal(pptxText(wide), deck.text());
    assert.deepEqual(pptxTextPerPage(wide), deck.textPerPage());
    assert.deepEqual(pptxStructuredText(wide), deck.structuredText());
    assert.deepEqual(pptxSlides(wide), deck.slides());
    assert.equal(pptxMarkdown(wide), deck.markdown());
    assert.deepEqual(pptxTables(wide), deck.tables());
    assert.deepEqual(pptxImages(wide), deck.images());
    assert.deepEqual(pptxAnnotations(wide), deck.annotations());
    assert.deepEqual(pptxNotes(wide), deck.notes());
    assert.deepEqual(pptxCharts(wide), deck.charts());
    assert.deepEqual(pptxMedia(wide), deck.media());
    assert.deepEqual(pptxLayout(wide), deck.layout());
    assert.deepEqual(pptxInvoice(wide), deck.invoice());
    assert.deepEqual(pptxDocument(wide), deck.document());
  });
  it("should throw what the constructor throws", {}, () => {
    assert.throws(() => pptxText(t.workbook), {
      message: "unsupported format: workbook"
    });
    assert.throws(() => pptxSlides(Buffer.from("not a zip at all")), {
      message: /^malformed presentation: /
    });
  });
});

describe("async forms", () => {
  it(
    "should resolve each method to what its sync twin returns",
    {},
    async () => {
      assert.deepEqual(await deck.metadataAsync(), deck.metadata());
      assert.equal(await deck.textAsync(), deck.text());
      assert.deepEqual(await deck.textPerPageAsync(), deck.textPerPage());
      assert.deepEqual(await deck.structuredTextAsync(), deck.structuredText());
      assert.deepEqual(await deck.slidesAsync(), deck.slides());
      assert.equal(await deck.markdownAsync(), deck.markdown());
      assert.deepEqual(await deck.tablesAsync(), deck.tables());
      assert.deepEqual(await deck.imagesAsync(), deck.images());
      assert.deepEqual(await deck.annotationsAsync(), deck.annotations());
      assert.deepEqual(await deck.notesAsync(), deck.notes());
      assert.deepEqual(await deck.chartsAsync(), deck.charts());
      assert.deepEqual(await deck.mediaAsync(), deck.media());
      assert.deepEqual(await deck.layoutAsync(), deck.layout());
      assert.deepEqual(await deck.invoiceAsync(), deck.invoice());
      assert.deepEqual(await deck.documentAsync(), deck.document());
    }
  );
  it(
    "should resolve each free function to what its sync twin returns",
    {},
    async () => {
      assert.deepEqual(await pptxMetadataAsync(older), activity.metadata());
      assert.equal(await pptxTextAsync(older), activity.text());
      assert.deepEqual(
        await pptxTextPerPageAsync(older),
        activity.textPerPage()
      );
      assert.deepEqual(
        await pptxStructuredTextAsync(older),
        activity.structuredText()
      );
      assert.deepEqual(await pptxSlidesAsync(older), activity.slides());
      assert.equal(await pptxMarkdownAsync(older), activity.markdown());
      assert.deepEqual(await pptxTablesAsync(older), activity.tables());
      assert.deepEqual(await pptxImagesAsync(older), activity.images());
      assert.deepEqual(
        await pptxAnnotationsAsync(older),
        activity.annotations()
      );
      assert.deepEqual(await pptxNotesAsync(older), activity.notes());
      assert.deepEqual(await pptxChartsAsync(older), activity.charts());
      assert.deepEqual(await pptxMediaAsync(older), activity.media());
      assert.deepEqual(await pptxLayoutAsync(older), activity.layout());
      assert.deepEqual(await pptxInvoiceAsync(older), activity.invoice());
      assert.deepEqual(await pptxDocumentAsync(older), activity.document());
    }
  );
  it("should reject with what the sync forms throw", {}, async () => {
    await assert.rejects(pptxTextAsync(t.workbook), {
      message: "unsupported format: workbook"
    });
    await assert.rejects(pptxDocumentAsync(Buffer.from("not a zip at all")), {
      message: /^malformed presentation: /
    });
    await assert.rejects(pptxInvoiceAsync(t.legacy), {
      message: /^unsupported format: OLE compound file/
    });
  });
  it(
    "should return the same slides to 32 readers running at once",
    {},
    async () => {
      const expected = deck.slides();
      const readers = Array.of<Promise<PptxSlide[]>>();
      for (let i = 0; i < 16; i++) {
        readers.push(deck.slidesAsync());
        readers.push(pptxSlidesAsync(wide));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should leave the caller's buffer alone and read the bytes it was given",
    {},
    async () => {
      // The async forms copy the buffer before the work is queued, so changing it afterwards
      // changes nothing.
      const copy = Buffer.from(wide);
      const pending = pptxTextAsync(copy);
      copy.fill(0);
      assert.equal(await pending, deck.text());
    }
  );
});
