import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Rm } from "@d0paminedriven/type-utils/utils";

import { WorkupService } from "@/__test__/workup.ts";

/** What the package exports, as a caller of it sees. */
type Root = typeof import("@d0paminedriven/docxdown-wasm");

/** Either build: the same verbs, and its own name for itself. */
type Binding = Rm<Root, "__napiBindingTarget"> & {
  readonly __napiBindingTarget: "wasm32-wasi" | "wasm32-wasip1";
};

/**
 * The WebAssembly package: the verbs of the main one, with no native addon
 * behind them and no entry that looks for one.
 *
 * It holds two builds, and this asks the same of both. The one with threads
 * is what a caller in Node gets from the package's name. The one without is
 * what a page or a worker with no shared memory can run, and is the
 * package's `threadless` entry. Each is imported by the name a caller would
 * use, so the package's own map of entries is what finds it.
 */
export function wasmSuite(
  binding: Binding,
  flavor: Binding["__napiBindingTarget"]
) {
  const {
    DocxDown,
    docxAnnotations,
    docxAnnotationsAsync,
    docxBlocks,
    docxBlocksAsync,
    docxDocument,
    docxDocumentAsync,
    docxFormFields,
    docxFormFieldsAsync,
    docxImages,
    docxImagesAsync,
    docxInvoice,
    docxInvoiceAsync,
    docxLayout,
    docxLayoutAsync,
    docxMarkdown,
    docxMarkdownAsync,
    docxMetadata,
    docxMetadataAsync,
    docxNotes,
    docxNotesAsync,
    docxTables,
    docxTablesAsync,
    docxText,
    docxTextAsync,
    docxTextPerPage,
    docxTextPerPageAsync
  } = binding;
  const fixtures = new WorkupService();
  const notDocument = Buffer.from("not a zip at all");

  describe(`the WebAssembly package (${flavor})`, () => {
    /** A short invoice, with a table, a link, and a footnote. */
    const sample = fixtures.getFixtureBuff("lo-pandoc.docx");
    /** A paper of five pages with seven pictures in it. */
    const pictures = fixtures.getFixtureBuff("Muscles-of-mastication.docx");

    it(
      "should be the build it is said to be, and never a native addon",
      {},
      () => {
        assert.notEqual(binding.__napiBindingTarget, "native");
        assert.equal(binding.__napiBindingTarget, flavor);
      }
    );

    it("should read a document's text and what it says of itself", {}, () => {
      assert.equal(docxText(sample).split("\n").at(-1), "Total due: 25.50 USD");
      assert.partialDeepStrictEqual(docxMetadata(sample), {
        title: "Invoice 4711",
        language: "en-US",
        pageCount: 1,
        pagesCounted: 1,
        paragraphCount: 20,
        tableCount: 1,
        linkCount: 1,
        footnoteCount: 1,
        pageSizeUniform: true
      });
      assert.partialDeepStrictEqual(docxMetadata(pictures), {
        pageCount: 5,
        pagesCounted: 5,
        tableCount: 2,
        imageCount: 7,
        linkCount: 2
      });
    });

    it("should write its Markdown and set its lines on a page", {}, () => {
      assert.equal(
        docxMarkdown(sample).split("\n").slice(8, 12).join("\n"),
        [
          "| Item | Qty | Unit price | Total |",
          "| --- | --- | --- | --- |",
          "| Widget | 2 | 10.00 | 20.00 |",
          "| Gadget | 1 | 5.50 | 5.50 |"
        ].join("\n")
      );
      const [page] = docxLayout(sample);
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
      assert.equal(docxInvoice(sample).fields.invoiceId?.value, "4711");
    });

    it("should give each picture with its bytes", {}, () => {
      const images = docxImages(pictures);
      assert.equal(images.length, 7);
      for (const image of images) {
        assert.ok(image.data.length > 0);
        assert.notEqual(image.mime, undefined);
      }
    });

    it("should give the same answer from each promise", {}, async () => {
      assert.equal(await docxTextAsync(sample), docxText(sample));
      assert.equal(await docxMarkdownAsync(sample), docxMarkdown(sample));
      assert.deepEqual(await docxMetadataAsync(sample), docxMetadata(sample));
      assert.deepEqual(
        await docxTextPerPageAsync(sample),
        docxTextPerPage(sample)
      );
      assert.deepEqual(await docxBlocksAsync(sample), docxBlocks(sample));
      assert.deepEqual(await docxTablesAsync(sample), docxTables(sample));
      assert.deepEqual(
        await docxAnnotationsAsync(sample),
        docxAnnotations(sample)
      );
      assert.deepEqual(
        await docxFormFieldsAsync(sample),
        docxFormFields(sample)
      );
      assert.deepEqual(await docxNotesAsync(sample), docxNotes(sample));
      assert.deepEqual(await docxLayoutAsync(sample), docxLayout(sample));
      assert.deepEqual(await docxInvoiceAsync(sample), docxInvoice(sample));
      assert.deepEqual(await docxImagesAsync(pictures), docxImages(pictures));
      assert.deepEqual(
        await docxDocumentAsync(pictures),
        docxDocument(pictures)
      );
    });

    it(
      "should reject from a promise for what is not a Word document",
      {},
      async () => {
        await assert.rejects(
          docxTextAsync(notDocument),
          /^Error: malformed document: /
        );
        assert.throws(
          () => docxText(notDocument),
          /^Error: malformed document: /
        );
      }
    );

    it(
      "should parse once in the class, in its constructor or from a promise",
      {},
      async () => {
        const doc = await DocxDown.openAsync(sample);
        assert.ok(doc instanceof DocxDown);
        assert.deepEqual(doc.document(), new DocxDown(sample).document());
        assert.deepEqual(await doc.documentAsync(), docxDocument(sample));
        assert.deepEqual(await doc.layoutAsync(), docxLayout(sample));
        assert.equal(await doc.textAsync(), docxText(sample));
      }
    );

    it("should take many calls at once", {}, async () => {
      // More waiting than any pool of workers has hands, and with no pool at all.
      const texts = await Promise.all(
        Array.from({ length: 64 }, () => docxTextAsync(pictures))
      );
      assert.deepEqual(new Set(texts), new Set([docxText(pictures)]));
    });
  });
}
