import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import {
  DocxDown,
  __napiBindingTarget,
  docxBlocksAsync,
  docxDocumentAsync,
  docxMarkdown,
  docxMarkdownAsync,
  docxMetadata,
  docxTextAsync
} from "@/index.js";
import type { Block } from "@/index.js";

/**
 * The WebAssembly build.
 *
 * `yarn test:wasi` sets `NAPI_RS_FORCE_WASI=error`, which makes the generated
 * loader require the wasm binding and throw when there is none. Under plain
 * `yarn test` the native addon loads and this suite skips itself.
 *
 * Async work is the part that differs most from native: on wasm it runs on
 * worker threads over shared memory, so the async readers get the attention.
 */
const t = new WorkupService();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const sample = t.getFixtureBuff("lo-pandoc.docx");
  const pictures = t.getFixtureBuff("Muscles-of-mastication.docx");
  const notDocument = Buffer.from("not a zip at all");

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });
  it("should return a DocxDown that reads the sample", {}, () => {
    const doc = new DocxDown(sample);
    assert.equal(doc instanceof DocxDown, true);
    assert.equal(doc.text().split("\n").at(-1), "Total due: 25.50 USD");
  });
  it("should return the same counts and paper as the native build", {}, () => {
    assert.partialDeepStrictEqual(docxMetadata(sample), {
      title: "Invoice 4711",
      language: "en-US",
      pageCount: 1,
      pagesCounted: 1,
      paragraphCount: 20,
      tableCount: 1,
      linkCount: 1,
      footnoteCount: 1,
      pageSizeUniform: true,
      sections: [
        {
          width: 612,
          height: 792,
          paperSize: { type: "LETTER", width: 612, height: 792 }
        }
      ]
    });
    assert.partialDeepStrictEqual(docxMetadata(pictures), {
      pageCount: 5,
      pagesCounted: 5,
      tableCount: 2,
      imageCount: 7,
      linkCount: 2
    });
  });
  it("should write the same Markdown and set the same lines", {}, () => {
    const doc = new DocxDown(sample);
    assert.equal(
      docxMarkdown(sample).split("\n").slice(8, 12).join("\n"),
      [
        "| Item | Qty | Unit price | Total |",
        "| --- | --- | --- | --- |",
        "| Widget | 2 | 10.00 | 20.00 |",
        "| Gadget | 1 | 5.50 | 5.50 |"
      ].join("\n")
    );
    const [page] = doc.layout();
    assert.ok(page);
    assert.partialDeepStrictEqual(page, { page: 1, width: 612, height: 792 });
    assert.deepEqual(
      page.lines.slice(4, 7).map((line) => line.text),
      [
        "Item Qty Unit price Total",
        "Widget 2 10.00 20.00",
        "Gadget 1 5.50 5.50"
      ]
    );
    assert.equal(doc.invoice().fields.invoiceId?.value, "4711");
  });
  it("should return each picture with its bytes", {}, () => {
    const images = new DocxDown(pictures).images();
    assert.equal(images.length, 7);
    assert.equal(
      images.every(
        (image) => image.data.length > 0 && image.mime !== undefined
      ),
      true
    );
  });
  it(
    "should resolve each async form to what its sync twin returns",
    {},
    async () => {
      const doc = new DocxDown(sample);
      const opened = await DocxDown.openAsync(sample);
      assert.deepEqual(opened.metadata(), doc.metadata());
      assert.deepEqual(await doc.metadataAsync(), doc.metadata());
      assert.equal(await doc.textAsync(), doc.text());
      assert.deepEqual(await doc.textPerPageAsync(), doc.textPerPage());
      assert.deepEqual(await doc.blocksAsync(), doc.blocks());
      assert.deepEqual(await doc.tablesAsync(), doc.tables());
      assert.deepEqual(await doc.annotationsAsync(), doc.annotations());
      assert.deepEqual(await doc.notesAsync(), doc.notes());
      assert.deepEqual(await doc.layoutAsync(), doc.layout());
      assert.deepEqual(await doc.invoiceAsync(), doc.invoice());
      assert.equal(await docxMarkdownAsync(sample), doc.markdown());
      const figures = new DocxDown(pictures);
      assert.deepEqual(await figures.imagesAsync(), figures.images());
      assert.deepEqual(await docxDocumentAsync(pictures), figures.document());
    }
  );
  it(
    "should return the same blocks to 32 readers running at once",
    {},
    async () => {
      const doc = new DocxDown(pictures);
      const expected = doc.blocks();
      const readers = Array.of<Promise<Block[]>>();
      for (let i = 0; i < 16; i++) {
        readers.push(doc.blocksAsync());
        readers.push(docxBlocksAsync(pictures));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should throw from the sync forms and reject from the async ones for bytes that are not a document",
    {},
    async () => {
      const message = /^malformed document: /;
      assert.throws(() => new DocxDown(notDocument), { message });
      assert.throws(() => docxMetadata(notDocument), { message });
      await assert.rejects(DocxDown.openAsync(notDocument), { message });
      await assert.rejects(docxTextAsync(notDocument), { message });
    }
  );
});
