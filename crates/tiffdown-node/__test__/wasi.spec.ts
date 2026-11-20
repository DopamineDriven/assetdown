import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { TiffTest } from "@/__test__/tiff-test.ts";
import {
  TiffDown,
  __napiBindingTarget,
  tiffDocument,
  tiffDocumentAsync,
  tiffPagePng,
  tiffPagePngAsync,
  tiffPages,
  tiffPagesAsync
} from "@/index.js";

/**
 * The WebAssembly build.
 *
 * `yarn test:wasi` sets `NAPI_RS_FORCE_WASI=error`, which makes the generated
 * loader require the wasm binding and throw when there is none. Under plain
 * `yarn test` the native addon loads and this suite skips itself.
 *
 * Async work is the part that differs most from native: on wasm it runs on
 * worker threads over shared memory, so the promise forms get the attention.
 */
const tiffTest = new TiffTest();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const file = tiffTest.twoPages;

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });

  it("should list a file's pages with what the file says of them", {}, () => {
    const [first, second, ...rest] = tiffPages(file);
    assert.deepEqual(rest, []);
    assert.ok(first && second);
    assert.deepEqual([first.number, first.width, first.height], [1, 170, 220]);
    assert.deepEqual(first.resolution, { x: 20, y: 20 });
    assert.deepEqual(first.compression, { type: "none" });
    assert.equal(first.dateTime, "2015-03-04T10:22:33");
    assert.deepEqual(first.paperSize, {
      type: "LETTER",
      width: 612,
      height: 792
    });
    assert.deepEqual([second.number, second.width, second.height], [2, 4, 2]);
  });

  it("should write a page as a PNG of its size", {}, () => {
    assert.deepEqual(tiffTest.pngHeader(tiffPagePng(file, 2)), {
      width: 4,
      height: 2,
      bitDepth: 8,
      colorType: 0
    });
    assert.throws(
      () => tiffPagePng(file, 3),
      /^Error: limit exceeded: page 3 was asked for, and the file has 2 pages$/
    );
  });

  it("should give the same answers from its promises", {}, async () => {
    const [pages, png, document] = await Promise.all([
      tiffPagesAsync(file),
      tiffPagePngAsync(file, 1),
      tiffDocumentAsync(file)
    ]);
    assert.deepEqual(pages, tiffPages(file));
    assert.deepEqual(png, tiffPagePng(file, 1));
    assert.deepEqual(document, tiffDocument(file));
  });

  it("should reject from a promise for what is not a TIFF", {}, async () => {
    await assert.rejects(
      tiffPagesAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });

  it(
    "should parse once in the class, in its constructor or from a promise",
    {},
    async () => {
      const tiff = await TiffDown.openAsync(file);
      assert.ok(tiff instanceof TiffDown);
      assert.deepEqual(tiff.document(), new TiffDown(file).document());
      assert.deepEqual(await tiff.pagePngAsync(2), tiffPagePng(file, 2));
      assert.deepEqual(await tiff.documentAsync(), tiffDocument(file));
    }
  );
});
