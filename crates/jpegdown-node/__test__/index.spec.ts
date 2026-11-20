import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { JpegTest } from "@/__test__/jpeg-test.ts";
import {
  JpegDown,
  jpegDocument,
  jpegDocumentAsync,
  jpegPage,
  jpegPageAsync,
  jpegPng,
  jpegPngAsync
} from "@/index.js";

const jpegTest = new JpegTest();

describe("jpegPage", () => {
  it("should say what the file states of its page", {}, () => {
    const page = jpegPage(jpegTest.tiny);
    assert.deepEqual([page.width, page.height], [16, 8]);
    assert.deepEqual(page.resolution, { x: 200, y: 200 });
    assert.deepEqual([page.bitsPerSample, page.components], [8, 1]);
    assert.deepEqual(page.color, { type: "gray" });
    assert.deepEqual(page.coding, { type: "baseline" });
    assert.equal(page.orientation, 1);
    assert.equal(page.software, undefined);
    assert.equal(page.dateTime, undefined);
    assert.ok(page.sizeInPoints);
    assert.ok(Math.abs(page.sizeInPoints.width - 5.76) < 1e-9);
    assert.ok(Math.abs(page.sizeInPoints.height - 2.88) < 1e-9);
    assert.equal(page.paperSize.type, "UNKNOWN");
  });

  it(
    "should take a size no sheet has for the sheet its shape is nearest to",
    {},
    () => {
      // Sixteen pixels at 200 to the inch is a sheet a twelfth of an inch wide. Twice as wide
      // as it is tall, it is nearest to a legal sheet on its side.
      const { sizeAsRead } = jpegPage(jpegTest.tiny);
      assert.deepEqual([sizeAsRead.width, sizeAsRead.height], [1008, 612]);
      assert.deepEqual(sizeAsRead.assumed, {
        type: "LEGAL",
        width: 612,
        height: 1008
      });
    }
  );

  it("should state no resolution for a density that is in no unit", {}, () => {
    const page = jpegPage(jpegTest.withUnit(0));
    assert.equal(page.resolution, undefined);
    assert.equal(page.sizeInPoints, undefined);
    // 200 to the centimeter is 508 to the inch.
    assert.deepEqual(jpegPage(jpegTest.withUnit(2)).resolution, {
      x: 508,
      y: 508
    });
  });

  it("should read how the picture is to be turned from its Exif", {}, () => {
    const turned = jpegTest.withSegment(jpegTest.tiny, jpegTest.orientation(6));
    const page = jpegPage(turned);
    assert.equal(page.orientation, 6);
    // As it lies in the file it is wide; turned, it is tall.
    assert.deepEqual([page.width, page.height], [16, 8]);
    assert.deepEqual(
      [page.sizeAsRead.width, page.sizeAsRead.height],
      [612, 1008]
    );
  });

  it(
    "should name a coding it does not read and still give the page",
    {},
    () => {
      assert.deepEqual(jpegPage(jpegTest.lossless).coding, {
        type: "lossless"
      });
    }
  );

  it(
    "should throw for bytes that are not a JPEG, and for a header cut short",
    {},
    () => {
      assert.throws(
        () => jpegPage(Buffer.from("%PDF-1.7\n")),
        /^Error: unsupported format: the file does not open as a JPEG does$/
      );
      assert.throws(
        () => jpegPage(jpegTest.tiny.subarray(0, 12)),
        /^Error: malformed file: /
      );
    }
  );

  it("should give the same answer from its promise", {}, async () => {
    assert.deepEqual(
      await jpegPageAsync(jpegTest.tiny),
      jpegPage(jpegTest.tiny)
    );
    await assert.rejects(
      jpegPageAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });
});

describe("jpegPng", () => {
  it("should give the picture as a PNG of its size", {}, () => {
    assert.deepEqual(jpegTest.pngHeader(jpegPng(jpegTest.tiny)), {
      width: 16,
      height: 8,
      bitDepth: 8,
      colorType: 0
    });
  });

  it("should turn the picture the way its orientation says", {}, () => {
    const turned = jpegTest.withSegment(jpegTest.tiny, jpegTest.orientation(6));
    const { width, height } = jpegTest.pngHeader(jpegPng(turned));
    assert.deepEqual([width, height], [8, 16]);
  });

  it("should refuse a picture kept without loss, by name", {}, () => {
    assert.throws(
      () => jpegPng(jpegTest.lossless),
      /^Error: unsupported format: the picture is kept without loss, a coding of JPEG that is not read$/
    );
  });

  it("should throw for a picture whose data is cut short", {}, () => {
    const cut = jpegTest.tiny.subarray(0, jpegTest.tiny.length - 40);
    assert.throws(() => jpegPng(cut), /^Error: malformed file: /);
  });

  it("should give the same PNG from its promise", {}, async () => {
    assert.deepEqual(await jpegPngAsync(jpegTest.tiny), jpegPng(jpegTest.tiny));
    await assert.rejects(
      jpegPngAsync(jpegTest.lossless),
      /^Error: unsupported format: /
    );
  });
});

describe("jpegDocument", () => {
  it("should give the page with the file's size in bytes", {}, async () => {
    const file = jpegTest.tiny;
    const found = jpegDocument(file);
    assert.equal(found.byteSize, file.length);
    assert.deepEqual(found.page, jpegPage(file));
    assert.deepEqual(await jpegDocumentAsync(file), found);
  });
});

describe("JpegDown", () => {
  it(
    "should answer as the functions do, from one reading of the header",
    {},
    async () => {
      const file = jpegTest.tiny;
      const jpeg = new JpegDown(file);
      assert.deepEqual(jpeg.page(), jpegPage(file));
      assert.deepEqual(jpeg.document(), jpegDocument(file));
      assert.deepEqual(jpeg.png(), jpegPng(file));
      assert.deepEqual(await jpeg.pngAsync(), jpegPng(file));
    }
  );

  it("should open from a promise", {}, async () => {
    const jpeg = await JpegDown.openAsync(jpegTest.tiny);
    assert.ok(jpeg instanceof JpegDown);
    assert.equal(jpeg.page().width, 16);
    await assert.rejects(
      JpegDown.openAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });

  it("should keep its own copy of the bytes it was given", {}, () => {
    const file = jpegTest.tiny;
    const before = jpegPng(file);
    const jpeg = new JpegDown(file);
    file.fill(0);
    assert.deepEqual(jpeg.png(), before);
  });

  it("should throw from its constructor for what is not a JPEG", {}, () => {
    assert.throws(
      () => new JpegDown(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });
});
