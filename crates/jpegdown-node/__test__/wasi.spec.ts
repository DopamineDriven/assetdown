import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { JpegTest } from "@/__test__/jpeg-test.ts";
import {
  JpegDown,
  __napiBindingTarget,
  jpegDocument,
  jpegDocumentAsync,
  jpegPage,
  jpegPageAsync,
  jpegPng,
  jpegPngAsync
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
const jpegTest = new JpegTest();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const file = jpegTest.tiny;

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });

  it("should say what the file states of its page", {}, () => {
    const page = jpegPage(file);
    assert.deepEqual([page.width, page.height], [16, 8]);
    assert.deepEqual(page.resolution, { x: 200, y: 200 });
    assert.deepEqual(page.color, { type: "gray" });
    assert.deepEqual(page.coding, { type: "baseline" });
  });

  it("should decode the picture and write it as a PNG", {}, () => {
    assert.deepEqual(jpegTest.pngHeader(jpegPng(file)), {
      width: 16,
      height: 8,
      bitDepth: 8,
      colorType: 0
    });
    assert.throws(
      () => jpegPng(jpegTest.lossless),
      /^Error: unsupported format: /
    );
  });

  it("should give the same answers from its promises", {}, async () => {
    const [page, png, document] = await Promise.all([
      jpegPageAsync(file),
      jpegPngAsync(file),
      jpegDocumentAsync(file)
    ]);
    assert.deepEqual(page, jpegPage(file));
    assert.deepEqual(png, jpegPng(file));
    assert.deepEqual(document, jpegDocument(file));
  });

  it("should reject from a promise for what is not a JPEG", {}, async () => {
    await assert.rejects(
      jpegPageAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });

  it(
    "should read the header once in the class, in its constructor or from a promise",
    {},
    async () => {
      const jpeg = await JpegDown.openAsync(file);
      assert.ok(jpeg instanceof JpegDown);
      assert.deepEqual(jpeg.document(), new JpegDown(file).document());
      assert.deepEqual(await jpeg.pngAsync(), jpegPng(file));
    }
  );
});
