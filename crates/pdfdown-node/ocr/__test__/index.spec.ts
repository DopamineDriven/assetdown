import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/ocr/__test__/workup.ts";
import {
  PdfDown,
  capabilities,
  extractLayoutWithOcrPerPage,
  extractLayoutWithOcrPerPageAsync,
  extractTextWithOcrPerPage,
  extractTextWithOcrPerPageAsync,
  pdfDocumentOcr,
  pdfDocumentOcrAsync
} from "@/ocr/index.js";

// WorkupService extends Fs
class OcrPdfTest extends WorkupService {
  /** A fixture of the base package, which the OCR package reads as well. */
  public baseFixture(name: string) {
    return this.fileToBuffer(`__test__/fixtures/${name}.pdf`);
  }

  /** The item at `index`, failing the test when there is none. */
  public at<const T>(items: readonly T[], index: number) {
    const item = items[index];
    if (item === undefined) {
      assert.fail(`no item at index ${index} of ${items.length}`);
    }
    return item;
  }
}

const t = new OcrPdfTest();
const hello = t.getFixtureBuff("hello-ocr");
const multipage = t.getFixtureBuff("multipage-ocr");
const rotated = t.getFixtureBuff("rotated-scan");
const stamped = t.getFixtureBuff("stamped-scan");
const native = t.baseFixture("invoices/wordpress-pdf-invoice-plugin-sample");
const scan = t.baseFixture("ascii85-dct-scan");

describe("the OCR package", () => {
  it("should report OCR as available", {}, () => {
    const caps = capabilities();
    assert.equal(caps.ocr, true);
    assert.equal(typeof caps.render, "boolean");
  });
  it("should carry every reader of the base package", {}, () => {
    const doc = new PdfDown(native);
    assert.equal(doc.textPerPage().length, 1);
    assert.equal(doc.metadata().pageCount, 1);
    assert.equal(doc.imagesPerPage().length, 1);
    assert.equal(doc.layout().length, 1);
  });
  it(
    "should have bounded Tesseract's OpenMP pool before the addon loaded",
    {},
    () => {
      assert.ok(
        process.env.OMP_THREAD_LIMIT !== undefined ||
          process.env.OMP_NUM_THREADS !== undefined
      );
    }
  );
});

describe("textWithOcrPerPage", () => {
  it("should read an image-only page with OCR", {}, () => {
    const pages = new PdfDown(hello).textWithOcrPerPage();
    assert.equal(pages.length, 1);
    const page = t.at(pages, 0);
    assert.equal(page.page, 1);
    assert.equal(page.source, "Ocr");
    const text = page.text.toLowerCase();
    assert.ok(text.includes("hello"), page.text);
    assert.ok(text.includes("world"), page.text);
  });
  it("should read every page of a scanned document, in order", {}, () => {
    const pages = new PdfDown(multipage).textWithOcrPerPage();
    assert.deepEqual(
      pages.map((p) => [p.page, p.source]),
      [
        [1, "Ocr"],
        [2, "Ocr"],
        [3, "Ocr"]
      ]
    );
    for (const [index, word] of ["one", "second", "third"].entries()) {
      assert.ok(t.at(pages, index).text.toLowerCase().includes(word), word);
    }
  });
  it(
    "should leave a page with a text layer of its own on the native tier",
    {},
    () => {
      const pages = new PdfDown(native).textWithOcrPerPage();
      assert.deepEqual(
        pages.map((p) => [p.page, p.source]),
        [[1, "Native"]]
      );
      assert.deepEqual(
        pages.map((p) => p.text),
        new PdfDown(native).textPerPage().map((p) => p.text)
      );
    }
  );
  it("should keep a page's own text in front of what OCR adds", {}, () => {
    const page = t.at(
      new PdfDown(stamped).textWithOcrPerPage({ render: "Never" }),
      0
    );
    assert.equal(page.source, "Ocr");
    assert.ok(page.text.includes("PO 4711"), page.text);
    assert.ok(page.text.toLowerCase().includes("hello"), page.text);
  });
  it("should accept its options", {}, () => {
    const pages = new PdfDown(hello).textWithOcrPerPage({
      lang: "eng",
      minTextLength: 1,
      maxThreads: 1,
      render: "Never"
    });
    assert.equal(t.at(pages, 0).source, "Ocr");
  });
  it("the async and free-function forms should agree", {}, async () => {
    const expected = new PdfDown(hello).textWithOcrPerPage();
    assert.deepEqual(
      await new PdfDown(hello).textWithOcrPerPageAsync(),
      expected
    );
    assert.deepEqual(extractTextWithOcrPerPage(hello), expected);
    assert.deepEqual(await extractTextWithOcrPerPageAsync(hello), expected);
  });
});

describe("layoutWithOcr", () => {
  it("should place OCR words on the page, each with a confidence", {}, () => {
    const page = t.at(new PdfDown(hello).layoutWithOcr(), 0);
    assert.equal(page.source, "Ocr");
    assert.equal(page.words.length, 3);
    assert.equal(page.lines.length, 1);
    for (const word of page.words) {
      assert.ok(
        word.confidence !== undefined &&
          word.confidence >= 0 &&
          word.confidence <= 1
      );
      assert.ok(word.x0 < word.x1 && word.y0 < word.y1);
      assert.equal(word.fontName, undefined);
    }
  });
  it("should make each line address a slice of the words", {}, () => {
    for (const page of new PdfDown(multipage).layoutWithOcr()) {
      for (const [index, line] of page.lines.entries()) {
        const words = page.words.slice(
          line.firstWord,
          line.firstWord + line.wordCount
        );
        assert.equal(words.map((w) => w.text).join(" "), line.text);
        for (const word of words) {
          assert.equal(word.line, index);
        }
      }
    }
  });
  it("should turn a sideways scan upright and say by how much", {}, () => {
    const page = t.at(
      new PdfDown(rotated).layoutWithOcr({ render: "Never" }),
      0
    );
    assert.equal(page.contentRotation, 270);
    assert.ok(page.words.length > 100);
    assert.ok(t.at(page.lines, 0).text.toLowerCase().includes("scanned"));
  });
  it(
    "should keep native words beside the OCR words on a hybrid page",
    {},
    () => {
      const page = t.at(
        new PdfDown(stamped).layoutWithOcr({ render: "Never" }),
        0
      );
      const recognized = page.words.filter((w) => w.confidence !== undefined);
      const own = page.words.filter((w) => w.confidence === undefined);
      assert.ok(recognized.length > 0);
      assert.ok(own.length > 0);
      assert.ok(page.warnings.length > 0);
    }
  );
  it("should match the base layout on a page with a text layer", {}, () => {
    const withOcr = new PdfDown(native).layoutWithOcr();
    assert.deepEqual(withOcr, new PdfDown(native).layout());
  });
  it("the async and free-function forms should agree", {}, async () => {
    const expected = new PdfDown(hello).layoutWithOcr();
    assert.deepEqual(await new PdfDown(hello).layoutWithOcrAsync(), expected);
    assert.deepEqual(extractLayoutWithOcrPerPage(hello), expected);
    assert.deepEqual(await extractLayoutWithOcrPerPageAsync(hello), expected);
  });
});

describe("documentOcr", () => {
  const whole = new PdfDown(multipage).documentOcr();

  it("should gather the document with its text read through OCR", {}, () => {
    assert.equal(whole.pageCount, 3);
    assert.equal(whole.totalImages, 3);
    assert.deepEqual(
      whole.text.map((p) => p.source),
      ["Ocr", "Ocr", "Ocr"]
    );
    assert.deepEqual(
      whole.structuredText.map((p) => [p.page, p.source]),
      [
        [1, "Ocr"],
        [2, "Ocr"],
        [3, "Ocr"]
      ]
    );
    assert.deepEqual(whole.text, new PdfDown(multipage).textWithOcrPerPage());
    // What the document says of itself comes with it, as `metadata()` gives it.
    const meta = new PdfDown(multipage).metadata();
    assert.deepEqual(
      [whole.title, whole.author, whole.subject, whole.keywords, whole.language],
      [meta.title, meta.author, meta.subject, meta.keywords, meta.language]
    );
    assert.equal(whole.isEncrypted, false);
    assert.equal(new PdfDown(multipage).imageCount(), whole.totalImages);
  });
  it("the async and free-function forms should agree", {}, async () => {
    assert.deepEqual(await new PdfDown(multipage).documentOcrAsync(), whole);
    assert.deepEqual(pdfDocumentOcr(multipage), whole);
    assert.deepEqual(await pdfDocumentOcrAsync(multipage), whole);
  });
});

describe("invoice in the OCR package", () => {
  it("should read a scanned page that the base package cannot", {}, () => {
    assert.equal(new PdfDown(scan).invoice().source, "Ocr");
  });
  it(
    "should give the base package's answer for a page with a text layer",
    {},
    () => {
      const result = new PdfDown(native).invoice();
      assert.equal(result.source, "Native");
      assert.equal(result.fields.invoiceId?.value, "INV-3337");
    }
  );
});
