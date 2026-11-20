import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { TiffTest } from "@/__test__/tiff-test.ts";
import {
  TiffDown,
  tiffDocumentOcr,
  tiffDocumentOcrAsync,
  tiffInvoice,
  tiffInvoiceAsync,
  tiffLayout,
  tiffLayoutAsync,
  tiffPages,
  tiffTextPerPage,
  tiffTextPerPageAsync
} from "@/ocr/index.js";
import type { TiffPageLayout } from "@/ocr/index.js";

/**
 * The repository holds no scan, so the pages here are drawn: words in a block face of five dots
 * by seven, which Tesseract reads, on a white page written out as a TIFF.
 */
class TiffOcrTest extends TiffTest {
  /** Five dots across and seven down, a `#` where there is ink. */
  private get glyphs() {
    return {
      T: ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
      O: [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
      A: [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
      L: ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
      "4": ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
      "2": [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
      " ": [".....", ".....", ".....", ".....", ".....", ".....", "....."]
    } as const;
  }

  private isDrawn(letter: string) {
    return (
      letter === "T" ||
      letter === "O" ||
      letter === "A" ||
      letter === "L" ||
      letter === "4" ||
      letter === "2" ||
      letter === " "
    );
  }

  /**
   * A white page of `width` by `height` pixels with "TOTAL 42" in black, its top left corner
   * 170 across and 220 down. Each dot of a glyph is 8 pixels square and a glyph with its gap
   * is six dots wide, so "TOTAL" is 232 pixels wide and 56 tall.
   */
  public drawn(width: number, height: number) {
    const [dot, left, top] = [8, 170, 220];
    const gray = Buffer.alloc(width * height, 255);
    [..."TOTAL 42"].forEach((letter, place) => {
      if (!this.isDrawn(letter)) throw new Error(`no glyph for ${letter}`);
      this.glyphs[letter].forEach((dots, row) => {
        [...dots].forEach((mark, column) => {
          if (mark !== "#") return;
          for (let down = 0; down < dot; down++) {
            const y = top + row * dot + down;
            const x = left + (place * 6 + column) * dot;
            gray.fill(0, y * width + x, y * width + x + dot);
          }
        });
      });
    });
    return gray;
  }

  /** A quarter turn clockwise of a picture of gray. */
  public turned(gray: Buffer, width: number, height: number) {
    const out = Buffer.alloc(gray.length);
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        out[x * height + (height - 1 - y)] = gray.readUInt8(y * width + x);
      }
    }
    return out;
  }

  /** "TOTAL 42" on a page 1700 by 600 at 200 pixels to the inch: 612 by 216 points. */
  public get total42() {
    return this.tiff([
      {
        width: 1700,
        height: 600,
        gray: this.drawn(1700, 600),
        dpi: { x: 200, y: 200 }
      }
    ]);
  }

  /** The drawn page, a blank page, and a page in a coding that is not read. */
  public get threePages() {
    const dpi = { x: 200, y: 200 };
    return this.tiff([
      { width: 1700, height: 600, gray: this.drawn(1700, 600), dpi },
      { width: 400, height: 200, dpi },
      { width: 400, height: 200, dpi, compression: 7 }
    ]);
  }

  public linesOf(page: TiffPageLayout) {
    return page.lines.map((line) => line.text);
  }

  /** Within a point and a half: Tesseract's box hugs the ink to a pixel or two. */
  public near(a: number, b: number) {
    return Math.abs(a - b) < 1.5;
  }

  /** "TOTAL" where `drawn` puts it on a page 612 by 216 points, counted up from its foot. */
  public totalIsInItsPlace(page: TiffPageLayout) {
    const [total] = page.words;
    assert.ok(total);
    assert.equal(total.text, "TOTAL");
    assert.equal(total.line, 0);
    assert.ok(this.near(total.x0, 61.2) && this.near(total.x1, 144.72));
    assert.ok(this.near(total.y1, 136.8) && this.near(total.y0, 116.64));
    assert.ok(total.confidence > 0.5 && total.confidence <= 1);
  }

  public get notRead() {
    return /^page 3: its picture could not be read, so it has no words: unsupported format: /;
  }
}

const ocrTest = new TiffOcrTest();

describe("tiffLayout", () => {
  it("should find a drawn word where it lies on the page", {}, () => {
    const [page, ...rest] = tiffLayout(ocrTest.total42);
    assert.deepEqual(rest, []);
    assert.ok(page);
    assert.deepEqual(ocrTest.linesOf(page), ["TOTAL 42"]);
    assert.deepEqual(
      [page.page, page.width, page.height, page.contentRotation],
      [1, 612, 216, 0]
    );
    assert.deepEqual(page.warnings, []);
    ocrTest.totalIsInItsPlace(page);
  });

  it("should read a page kept on its side as it is to be looked at", {}, () => {
    // Orientation 8: the file's rows run up the left side of the page.
    const kept = ocrTest.tiff([
      {
        width: 600,
        height: 1700,
        gray: ocrTest.turned(ocrTest.drawn(1700, 600), 1700, 600),
        dpi: { x: 200, y: 200 },
        orientation: 8
      }
    ]);
    const [page] = tiffLayout(kept);
    assert.ok(page);
    assert.deepEqual(ocrTest.linesOf(page), ["TOTAL 42"]);
    assert.deepEqual(
      [page.width, page.height, page.contentRotation],
      [612, 216, 0]
    );
    ocrTest.totalIsInItsPlace(page);
  });

  it("should turn a page whose text lies sideways, and say so", {}, () => {
    // The same picture with nothing said of its orientation: the text itself has to be found.
    const sideways = ocrTest.tiff([
      {
        width: 600,
        height: 1700,
        gray: ocrTest.turned(ocrTest.drawn(1700, 600), 1700, 600),
        dpi: { x: 200, y: 200 }
      }
    ]);
    const [page] = tiffLayout(sideways);
    assert.ok(page);
    assert.deepEqual(ocrTest.linesOf(page), ["TOTAL 42"]);
    assert.deepEqual(
      [page.width, page.height, page.contentRotation],
      [612, 216, 270]
    );
    assert.equal(page.warnings.length, 1);
    assert.match(
      page.warnings[0] ?? "",
      /^page 1: text reads upright after a 270° turn \(by (OSD|retry)\)$/
    );
    ocrTest.totalIsInItsPlace(page);
  });

  it(
    "should list a page it cannot decode, with no words and the reason",
    {},
    () => {
      const [first, second, third, ...rest] = tiffLayout(ocrTest.threePages, {
        maxThreads: 2
      });
      assert.deepEqual(rest, []);
      assert.ok(first && second && third);
      assert.deepEqual(ocrTest.linesOf(first), ["TOTAL 42"]);
      assert.deepEqual(
        [second.page, second.words, second.warnings],
        [2, [], []]
      );
      assert.deepEqual([third.page, third.words, third.lines], [3, [], []]);
      assert.deepEqual([third.width, third.height], [144, 72]);
      assert.equal(third.warnings.length, 1);
      assert.match(third.warnings[0] ?? "", ocrTest.notRead);
    }
  );

  it("should say so when Tesseract has no data for the language", {}, () => {
    const [page] = tiffLayout(ocrTest.total42, { lang: "no-such-language" });
    assert.ok(page);
    assert.deepEqual(page.words, []);
    assert.deepEqual(page.warnings, [
      "page 1: not read: Tesseract has no data here for the language `no-such-language`"
    ]);
  });

  it("should give the same answer from its promise", {}, async () => {
    assert.deepEqual(
      await tiffLayoutAsync(ocrTest.total42),
      tiffLayout(ocrTest.total42)
    );
    await assert.rejects(
      tiffLayoutAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });
});

describe("tiffTextPerPage", () => {
  it(
    "should give each page's lines, and nothing for a page with none",
    {},
    async () => {
      const text = tiffTextPerPage(ocrTest.threePages);
      assert.deepEqual(text, [
        { page: 1, text: "TOTAL 42" },
        { page: 2, text: "" },
        { page: 3, text: "" }
      ]);
      assert.deepEqual(await tiffTextPerPageAsync(ocrTest.threePages), text);
    }
  );
});

describe("tiffInvoice", () => {
  it(
    "should read from the recognized pages and name a page that was not read",
    {},
    async () => {
      const invoice = tiffInvoice(ocrTest.threePages);
      assert.equal(invoice.source, "Ocr");
      assert.ok(
        invoice.warnings.some((warning) => ocrTest.notRead.test(warning))
      );
      assert.deepEqual(await tiffInvoiceAsync(ocrTest.threePages), invoice);
    }
  );

  it("should take the invoice's options and the recognition's", {}, () => {
    const invoice = tiffInvoice(
      ocrTest.total42,
      { languages: ["en"], includeCandidates: true },
      { lang: "eng", maxThreads: 1 }
    );
    assert.equal(invoice.source, "Ocr");
  });
});

describe("tiffDocumentOcr", () => {
  it(
    "should give the facts, the layout, the text, and the invoice from one reading",
    {},
    async () => {
      const file = ocrTest.threePages;
      const found = tiffDocumentOcr(file);
      assert.equal(found.byteSize, file.length);
      assert.equal(found.pageCount, 3);
      assert.deepEqual(found.pages, tiffPages(file));
      assert.deepEqual(found.warnings, []);
      assert.deepEqual(found.layout, tiffLayout(file));
      assert.deepEqual(found.text, tiffTextPerPage(file));
      assert.deepEqual(found.invoice, tiffInvoice(file));
      assert.deepEqual(await tiffDocumentOcrAsync(file), found);
    }
  );
});

describe("TiffDown", () => {
  it(
    "should read words as the functions do, and keep what the base package has",
    {},
    async () => {
      const file = ocrTest.threePages;
      const tiff = new TiffDown(file);
      assert.equal(tiff.pageCount(), 3);
      assert.deepEqual(tiff.pages(), tiffPages(file));
      const layout = tiffLayout(file);
      assert.deepEqual(tiff.layout(), layout);
      assert.deepEqual(await tiff.layoutAsync({ maxThreads: 2 }), layout);
      assert.deepEqual(tiff.textPerPage(), tiffTextPerPage(file));
      assert.deepEqual(await tiff.textPerPageAsync(), tiffTextPerPage(file));
      assert.deepEqual(tiff.invoice(), tiffInvoice(file));
      assert.deepEqual(await tiff.invoiceAsync(), tiffInvoice(file));
      assert.deepEqual(tiff.documentOcr(), tiffDocumentOcr(file));
      assert.deepEqual(await tiff.documentOcrAsync(), tiffDocumentOcr(file));
    }
  );
});
