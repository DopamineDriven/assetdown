import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import {
  MimeDown,
  __napiBindingTarget,
  builtWith,
  sniff,
  sniffAsync,
  specs,
  specsAsync,
  specsFromRanges
} from "@/index.js";
import type { Specs } from "@/index.js";

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
const t = new WorkupService();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  /** An Illustrator document, which is a PDF under another name. */
  const illustrator = t.getFixtureBuff("Asset-1ldpi.ai");
  /** The start of a PNG of one pixel: its signature and its header chunk, checksum and all. */
  const picture = Buffer.from(
    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489",
    "hex"
  );

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });
  it("should name a type from its bytes as the native build does", {}, () => {
    assert.deepEqual(sniff(illustrator), {
      mime: "application/pdf",
      topLevel: "application",
      ext: "pdf",
      parser: "pdfdown",
      evidence: "signature",
      retry: null,
      needs: null
    });
    assert.partialDeepStrictEqual(sniff(picture), {
      mime: "image/png",
      topLevel: "image",
      ext: "png",
      parser: null
    });
    assert.equal(sniff(Buffer.alloc(64)).mime, "application/octet-stream");
    // Behind leading bytes a file is found only when that is asked for.
    const behind = Buffer.concat([Buffer.alloc(100), picture]);
    assert.equal(sniff(behind).evidence, null);
    assert.deepEqual(sniff(behind, { retryOnUnknown: true }).retry, {
      type: "leadingBytes",
      skipped: 100
    });
  });
  it("should describe a picture and a document as the native build does", {}, () => {
    const found = specs(picture);
    assert.equal(found.facts.type, "image");
    if (found.facts.type !== "image") return;
    const { image } = found.facts;
    assert.deepEqual(
      [image.width, image.height, image.hasAlpha, image.colorModel],
      [1, 1, true, "rgba"]
    );

    const { pdf } = builtWith();
    const described = specs(illustrator);
    assert.equal(described.mime, "application/pdf");
    assert.deepEqual(described.warnings, []);
    assert.equal(described.facts.type, "document");
    if (described.facts.type !== "document") return;
    const { document } = described.facts;
    assert.equal(document.detail.type, "pdf");
    if (document.detail.type !== "pdf") return;
    assert.equal(document.detail.pdf.version, "1.6");
    // The rest is the PDF parser's, when the build has it.
    assert.equal(document.pageCount, pdf ? 1 : null);
  });
  it(
    "should give the whole file's answer from its head and what that asks for",
    {},
    () => {
      const size = illustrator.byteLength;
      const pieces = [{ offset: 0, bytes: illustrator.subarray(0, 1024) }];
      let answer = specsFromRanges(pieces, size);
      let fetches = 1;
      while (answer.needs !== null && fetches < 8) {
        const { offset, length } = answer.needs;
        pieces.push({
          offset,
          bytes: illustrator.subarray(offset, offset + length)
        });
        answer = specsFromRanges(pieces, size);
        fetches += 1;
      }
      assert.equal(answer.needs, null);
      assert.ok(fetches <= 2, `took ${fetches} fetches`);
      assert.deepEqual(answer, specs(illustrator));
    }
  );
  it(
    "should resolve each async form to what its sync twin returns",
    {},
    async () => {
      const held = new MimeDown(illustrator);
      assert.equal(held instanceof MimeDown, true);
      assert.equal(held.byteSize, illustrator.byteLength);
      assert.deepEqual(await sniffAsync(illustrator), sniff(illustrator));
      assert.deepEqual(await specsAsync(illustrator), specs(illustrator));
      assert.deepEqual(await sniffAsync(picture), sniff(picture));
      assert.deepEqual(held.sniff(), sniff(illustrator));
      assert.deepEqual(await held.sniffAsync(), held.sniff());
      assert.deepEqual(held.specs(), specs(illustrator));
      assert.deepEqual(await held.specsAsync(), held.specs());
      assert.deepEqual(await held.keyframesAsync(), held.keyframes());
    }
  );
  it(
    "should return the same answer to 32 readers running at once",
    {},
    async () => {
      const held = new MimeDown(illustrator);
      const expected = held.specs();
      const readers = Array.of<Promise<Specs>>();
      for (let i = 0; i < 16; i++) {
        readers.push(held.specsAsync());
        readers.push(specsAsync(illustrator));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should refuse a piece that says where it is with something other than a whole number",
    {},
    () => {
      assert.throws(() => specsFromRanges([{ offset: -1, bytes: picture }]), {
        code: "InvalidArg"
      });
      assert.throws(
        () => specsFromRanges([{ offset: 0, bytes: picture }], 1.5),
        { code: "InvalidArg" }
      );
    }
  );
});
