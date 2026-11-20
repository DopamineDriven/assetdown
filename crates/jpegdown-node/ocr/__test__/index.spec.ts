import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { JpegTest } from "@/__test__/jpeg-test.ts";
import {
  JpegDown,
  jpegDocumentOcr,
  jpegDocumentOcrAsync,
  jpegInvoice,
  jpegInvoiceAsync,
  jpegLayout,
  jpegLayoutAsync,
  jpegPage,
  jpegText,
  jpegTextAsync
} from "@/ocr/index.js";
import type { JpegPageLayout } from "@/ocr/index.js";

/**
 * The repository holds no scan, so the page here is drawn: "TOTAL 42" in a block face of five
 * dots by seven, which Tesseract reads, kept as a literal.
 */
class JpegOcrTest extends JpegTest {
  /**
   * A white picture of 400 pixels by 200 that states 100 pixels to the inch: 288 points by
   * 144. "TOTAL 42" is set in dots of 4 pixels, its top left corner 100 across and 86 down,
   * so "TOTAL" is 116 pixels wide and 28 tall.
   */
  public get total42() {
    return Buffer.from(
      "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" +
        "AAAAAAAAAAAAAAAAAAAAAAAAAAA//9k=",
      "base64"
    );
  }

  public get enlarged() {
    return "page 1: the picture has 100 pixels to the inch, and was enlarged to 200 before it was read";
  }

  public linesOf(page: JpegPageLayout) {
    return page.lines.map((line) => line.text);
  }

  /**
   * Within four points. A pixel of this picture is nearly three quarters of a point, and the
   * edges of a picture that was enlarged are soft, so a box lies a few pixels outside the ink.
   */
  public near(a: number, b: number) {
    return Math.abs(a - b) < 4;
  }

  /** "TOTAL" where the picture has it, on a page 288 by 144 points, counted up from its foot. */
  public totalIsInItsPlace(page: JpegPageLayout) {
    const [total] = page.words;
    assert.ok(total);
    assert.equal(total.text, "TOTAL");
    assert.equal(total.line, 0);
    assert.ok(this.near(total.x0, 72) && this.near(total.x1, 155.52));
    assert.ok(this.near(total.y1, 82.08) && this.near(total.y0, 61.92));
    assert.ok(total.confidence > 0.5 && total.confidence <= 1);
  }
}

const ocrTest = new JpegOcrTest();

describe("jpegLayout", () => {
  it("should find a drawn word where it lies on the page", {}, () => {
    const page = jpegLayout(ocrTest.total42);
    assert.deepEqual(ocrTest.linesOf(page), ["TOTAL 42"]);
    assert.deepEqual(
      [page.width, page.height, page.contentRotation],
      [288, 144, 0]
    );
    ocrTest.totalIsInItsPlace(page);
  });

  it(
    "should say that a picture with few pixels to the inch was enlarged",
    {},
    () => {
      assert.deepEqual(jpegLayout(ocrTest.total42).warnings, [
        ocrTest.enlarged
      ]);
    }
  );

  it("should say so when Tesseract has no data for the language", {}, () => {
    const page = jpegLayout(ocrTest.total42, { lang: "no-such-language" });
    assert.deepEqual(page.words, []);
    assert.deepEqual(page.warnings, [
      ocrTest.enlarged,
      "page 1: not read: Tesseract has no data here for the language `no-such-language`"
    ]);
  });

  it("should throw for a picture it cannot decode", {}, async () => {
    assert.throws(
      () => jpegLayout(ocrTest.lossless),
      /^Error: unsupported format: the picture is kept without loss/
    );
    await assert.rejects(
      jpegLayoutAsync(ocrTest.lossless),
      /^Error: unsupported format: /
    );
  });

  it("should give the same answer from its promise", {}, async () => {
    assert.deepEqual(
      await jpegLayoutAsync(ocrTest.total42),
      jpegLayout(ocrTest.total42)
    );
  });
});

describe("jpegText", () => {
  it("should give the page's lines", {}, async () => {
    assert.equal(jpegText(ocrTest.total42), "TOTAL 42");
    assert.equal(
      await jpegTextAsync(ocrTest.total42, { lang: "eng" }),
      "TOTAL 42"
    );
  });
});

describe("jpegInvoice", () => {
  it(
    "should read from the recognized page and carry what the page warns of",
    {},
    async () => {
      const invoice = jpegInvoice(ocrTest.total42);
      assert.equal(invoice.source, "Ocr");
      assert.equal(invoice.warnings[0], ocrTest.enlarged);
      assert.deepEqual(await jpegInvoiceAsync(ocrTest.total42), invoice);
    }
  );

  it("should take the invoice's options and the recognition's", {}, () => {
    const invoice = jpegInvoice(
      ocrTest.total42,
      { languages: ["en"], includeCandidates: true },
      { lang: "eng" }
    );
    assert.equal(invoice.source, "Ocr");
  });
});

describe("jpegDocumentOcr", () => {
  it(
    "should give the facts, the layout, the text, and the invoice from one reading",
    {},
    async () => {
      const file = ocrTest.total42;
      const found = jpegDocumentOcr(file);
      assert.equal(found.byteSize, file.length);
      assert.deepEqual(found.page, jpegPage(file));
      assert.deepEqual(found.layout, jpegLayout(file));
      assert.equal(found.text, "TOTAL 42");
      assert.deepEqual(found.invoice, jpegInvoice(file));
      assert.deepEqual(await jpegDocumentOcrAsync(file), found);
    }
  );
});

describe("JpegDown", () => {
  it(
    "should read words as the functions do, and keep what the base package has",
    {},
    async () => {
      const file = ocrTest.total42;
      const jpeg = new JpegDown(file);
      assert.deepEqual(jpeg.page(), jpegPage(file));
      const layout = jpegLayout(file);
      assert.deepEqual(jpeg.layout(), layout);
      assert.deepEqual(await jpeg.layoutAsync(), layout);
      assert.equal(jpeg.text(), "TOTAL 42");
      assert.equal(await jpeg.textAsync(), "TOTAL 42");
      assert.deepEqual(jpeg.invoice(), jpegInvoice(file));
      assert.deepEqual(await jpeg.invoiceAsync(), jpegInvoice(file));
      assert.deepEqual(jpeg.documentOcr(), jpegDocumentOcr(file));
      assert.deepEqual(await jpeg.documentOcrAsync(), jpegDocumentOcr(file));
    }
  );
});
