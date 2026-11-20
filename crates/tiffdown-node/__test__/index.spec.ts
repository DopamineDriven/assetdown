import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { TiffTest } from "@/__test__/tiff-test.ts";
import {
  TiffDown,
  tiffDocument,
  tiffDocumentAsync,
  tiffPagePng,
  tiffPagePngAsync,
  tiffPages,
  tiffPagesAsync
} from "@/index.js";

const tiffTest = new TiffTest();

describe("tiffPages", () => {
  it("should list each page with what the file says of it", {}, () => {
    const [first, second, ...rest] = tiffPages(tiffTest.twoPages);
    assert.deepEqual(rest, []);
    assert.ok(first && second);
    assert.deepEqual(
      [first.number, first.width, first.height, first.orientation],
      [1, 170, 220, 1]
    );
    assert.deepEqual(first.resolution, { x: 20, y: 20 });
    assert.deepEqual(first.bitsPerSample, [8]);
    assert.equal(first.samplesPerPixel, 1);
    assert.deepEqual(first.compression, { type: "none" });
    assert.deepEqual(first.photometric, { type: "blackIsZero" });
    assert.equal(first.software, "a scanner");
    // As the file records it, with no time zone: the format has no place for one.
    assert.equal(first.dateTime, "2015-03-04T10:22:33");
    assert.deepEqual(first.sizeInPoints, { width: 612, height: 792 });
    assert.deepEqual([second.number, second.width, second.height], [2, 4, 2]);
    assert.equal(second.resolution, undefined);
    assert.equal(second.software, undefined);
    assert.equal(second.dateTime, undefined);
    assert.equal(second.sizeInPoints, undefined);
  });

  it("should say which paper a page is nearest to", {}, () => {
    const [first, second] = tiffPages(tiffTest.twoPages);
    assert.deepEqual(first?.paperSize, {
      type: "LETTER",
      width: 612,
      height: 792
    });
    // A page that states no resolution has no size to compare.
    assert.deepEqual(second?.paperSize, {
      type: "UNKNOWN",
      closestTo: [null, null]
    });
  });

  it(
    "should name a coding it does not read and still list the page",
    {},
    () => {
      const pages = tiffPages(
        tiffTest.tiff([
          { width: 4, height: 2, compression: 7 },
          { width: 4, height: 2, compression: 34712 }
        ])
      );
      assert.deepEqual(
        pages.map((page) => page.compression),
        [{ type: "jpeg" }, { type: "other", code: 34712 }]
      );
    }
  );

  it("should throw for bytes that are not a TIFF", {}, () => {
    assert.throws(
      () => tiffPages(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });

  it("should give the same answer from its promise", {}, async () => {
    assert.deepEqual(
      await tiffPagesAsync(tiffTest.twoPages),
      tiffPages(tiffTest.twoPages)
    );
    await assert.rejects(
      tiffPagesAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });
});

describe("tiffPagePng", () => {
  it("should give a page as a PNG of its size", {}, () => {
    const png = tiffPagePng(tiffTest.twoPages, 2);
    assert.deepEqual(tiffTest.pngHeader(png), {
      width: 4,
      height: 2,
      bitDepth: 8,
      colorType: 0
    });
  });

  it("should turn a page the way its orientation says", {}, () => {
    // Orientation 6: the rows of the file run down the right side of the page.
    const kept = tiffTest.tiff([{ width: 4, height: 2, orientation: 6 }]);
    const { width, height } = tiffTest.pngHeader(tiffPagePng(kept, 1));
    assert.deepEqual([width, height], [2, 4]);
  });

  it(
    "should square the pixels of a fax with fewer rows to the inch",
    {},
    () => {
      const fax = tiffTest.tiff([
        { width: 4, height: 2, dpi: { x: 200, y: 100 } }
      ]);
      const { width, height } = tiffTest.pngHeader(tiffPagePng(fax, 1));
      assert.deepEqual([width, height], [4, 4]);
    }
  );

  it("should take only a whole page number from 1", {}, () => {
    for (const page of [0, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY]) {
      assert.throws(
        () => tiffPagePng(tiffTest.twoPages, page),
        /page must be a whole number from 1/,
        `${page}`
      );
    }
  });

  it(
    "should say how many pages there are when asked for one too many",
    {},
    () => {
      assert.throws(
        () => tiffPagePng(tiffTest.twoPages, 3),
        /^Error: limit exceeded: page 3 was asked for, and the file has 2 pages$/
      );
    }
  );

  it("should throw for a page in a coding it does not read", {}, () => {
    const coded = tiffTest.tiff([{ width: 4, height: 2, compression: 7 }]);
    assert.throws(() => tiffPagePng(coded, 1), /^Error: unsupported format: /);
  });

  it("should give the same PNG from its promise", {}, async () => {
    assert.deepEqual(
      await tiffPagePngAsync(tiffTest.twoPages, 1),
      tiffPagePng(tiffTest.twoPages, 1)
    );
    await assert.rejects(
      tiffPagePngAsync(tiffTest.twoPages, 3),
      /^Error: limit exceeded: /
    );
    // A page number is checked before any work is handed to another thread.
    assert.throws(
      () => tiffPagePngAsync(tiffTest.twoPages, 1.5),
      /page must be a whole number from 1/
    );
  });
});

describe("tiffDocument", () => {
  it(
    "should give the file's pages with its size and what was wrong with it",
    {},
    () => {
      const file = tiffTest.twoPages;
      const found = tiffDocument(file);
      assert.equal(found.byteSize, file.length);
      assert.equal(found.pageCount, 2);
      assert.deepEqual(found.pages, tiffPages(file));
      assert.deepEqual(found.warnings, []);
    }
  );

  it("should give the same answer from its promise", {}, async () => {
    assert.deepEqual(
      await tiffDocumentAsync(tiffTest.twoPages),
      tiffDocument(tiffTest.twoPages)
    );
  });
});

describe("TiffDown", () => {
  it(
    "should answer as the functions do, from one reading of the file",
    {},
    async () => {
      const file = tiffTest.twoPages;
      const tiff = new TiffDown(file);
      assert.equal(tiff.pageCount(), 2);
      assert.deepEqual(tiff.pages(), tiffPages(file));
      assert.deepEqual(tiff.warnings(), []);
      assert.deepEqual(tiff.document(), tiffDocument(file));
      assert.deepEqual(await tiff.documentAsync(), tiffDocument(file));
      assert.deepEqual(tiff.pagePng(2), tiffPagePng(file, 2));
      assert.deepEqual(await tiff.pagePngAsync(2), tiffPagePng(file, 2));
    }
  );

  it("should open from a promise", {}, async () => {
    const tiff = await TiffDown.openAsync(tiffTest.twoPages);
    assert.ok(tiff instanceof TiffDown);
    assert.equal(tiff.pageCount(), 2);
    await assert.rejects(
      TiffDown.openAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });

  it("should keep its own copy of the bytes it was given", {}, () => {
    const file = tiffTest.twoPages;
    const before = tiffPagePng(file, 1);
    const tiff = new TiffDown(file);
    file.fill(0);
    assert.deepEqual(tiff.pagePng(1), before);
  });

  it("should say what was wrong with a file it could still read", {}, () => {
    const looping = tiffTest.tiff([{ width: 4, height: 2 }], true);
    const tiff = new TiffDown(looping);
    assert.equal(tiff.pageCount(), 1);
    assert.deepEqual(tiff.warnings(), [
      "its directories lead back to one already read, where the list of its pages stops"
    ]);
  });

  it("should throw from its constructor for what is not a TIFF", {}, () => {
    assert.throws(
      () => new TiffDown(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });

  it("should take only a whole page number from 1", {}, () => {
    const tiff = new TiffDown(tiffTest.twoPages);
    assert.throws(() => tiff.pagePng(0), /page must be a whole number from 1/);
    assert.throws(
      () => tiff.pagePngAsync(2.5),
      /page must be a whole number from 1/
    );
  });
});
