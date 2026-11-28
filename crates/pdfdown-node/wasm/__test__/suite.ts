import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Rm } from "@d0paminedriven/type-utils/utils";

import { WorkupService } from "@/__test__/workup.ts";

/** What the package exports, as a caller of it sees. */
type Root = typeof import("@d0paminedriven/pdfdown-wasm");

/** Either build: the same verbs, and its own name for itself. */
type Binding = Rm<Root, "__napiBindingTarget"> & {
  readonly __napiBindingTarget: "wasm32-wasi" | "wasm32-wasip1";
};

/**
 * The WebAssembly package: the verbs of the main one, with no native addon
 * behind them and no entry that looks for one. It reads the text a PDF holds;
 * it neither recognizes scanned pages nor renders them.
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
    PdfDown,
    attachments,
    attachmentsAsync,
    capabilities,
    capabilitiesAsync,
    extractAnnotationsPerPage,
    extractAnnotationsPerPageAsync,
    extractFormFieldsPerPage,
    extractFormFieldsPerPageAsync,
    extractImagesPerPage,
    extractImagesPerPageAsync,
    extractInvoice,
    extractInvoiceAsync,
    extractStructuredTextPerPage,
    extractStructuredTextPerPageAsync,
    extractTextPerPage,
    extractTextPerPageAsync,
    invoiceFromLayout,
    invoiceFromLayoutAsync,
    layoutPerPage,
    layoutPerPageAsync,
    pdfDocument,
    pdfDocumentAsync,
    pdfImageCount,
    pdfImageCountAsync,
    pdfMetadata,
    pdfMetadataAsync
  } = binding;
  const fixtures = new WorkupService();
  const notPdf = Buffer.from("not a pdf at all");

  describe(`the WebAssembly package (${flavor})`, () => {
    const invoice = fixtures.getFixtureBuff(
      "invoices/wordpress-pdf-invoice-plugin-sample"
    );
    const layoutSample = fixtures.getFixtureBuff("layout/layout-sample");
    const dctScan = fixtures.getFixtureBuff("ascii85-dct-scan");
    const form = fixtures.getFixtureBuff("forms/acroform-basic");

    it(
      "should be the build it is said to be, and never a native addon",
      {},
      () => {
        assert.notEqual(binding.__napiBindingTarget, "native");
        assert.equal(binding.__napiBindingTarget, flavor);
      }
    );

    it("should say that it neither recognizes nor renders", {}, async () => {
      assert.deepEqual(capabilities(), { ocr: false, render: false });
      assert.deepEqual(await capabilitiesAsync(), capabilities());
    });

    it("should read a document's metadata and its text", {}, () => {
      const meta = pdfMetadata(invoice);
      assert.equal(meta.version, "1.3");
      assert.equal(meta.pageCount, 1);
      const [box] = meta.pageBoxes;
      assert.ok(box);
      assert.deepEqual(
        [Math.round(box.width), Math.round(box.height)],
        [595, 842]
      );
      const [page] = extractTextPerPage(invoice);
      assert.ok(page?.text.includes("INV-3337"));
    });

    it("should make each layout line address a slice of the words", {}, () => {
      const pages = layoutPerPage(layoutSample);
      assert.equal(pages.length, 2);
      for (const page of pages) {
        assert.ok(page.words.length > 0);
        for (const [index, line] of page.lines.entries()) {
          const words = page.words.slice(
            line.firstWord,
            line.firstWord + line.wordCount
          );
          assert.equal(words.map((word) => word.text).join(" "), line.text);
          for (const word of words) assert.equal(word.line, index);
        }
      }
    });

    it("should decode a JPEG scan into a PNG", {}, () => {
      const [image] = extractImagesPerPage(dctScan);
      assert.ok(image);
      assert.equal(image.page, 1);
      assert.ok(image.width > 0 && image.height > 0);
      assert.deepEqual(
        [...image.data.subarray(0, 4)],
        [0x89, 0x50, 0x4e, 0x47]
      );
      assert.equal(pdfImageCount(dctScan), 1);
    });

    it("should read form fields and the invoice's number", {}, () => {
      assert.ok(extractFormFieldsPerPage(form).length > 0);
      const result = extractInvoice(invoice);
      assert.equal(result.source, "Native");
      assert.equal(result.fields.invoiceId?.value, "INV-3337");
      // The same fields from pages a caller already has.
      const fromPages = invoiceFromLayout(layoutPerPage(invoice));
      assert.equal(fromPages.fields.invoiceId?.value, "INV-3337");
    });

    it("should give the same answer from each promise", {}, async () => {
      assert.deepEqual(
        await extractTextPerPageAsync(layoutSample),
        extractTextPerPage(layoutSample)
      );
      assert.deepEqual(
        await extractStructuredTextPerPageAsync(layoutSample),
        extractStructuredTextPerPage(layoutSample)
      );
      assert.deepEqual(
        await layoutPerPageAsync(layoutSample),
        layoutPerPage(layoutSample)
      );
      assert.deepEqual(
        await extractAnnotationsPerPageAsync(layoutSample),
        extractAnnotationsPerPage(layoutSample)
      );
      assert.deepEqual(await pdfMetadataAsync(invoice), pdfMetadata(invoice));
      assert.deepEqual(await attachmentsAsync(invoice), attachments(invoice));
      assert.deepEqual(
        await extractInvoiceAsync(invoice),
        extractInvoice(invoice)
      );
      const pages = layoutPerPage(invoice);
      assert.deepEqual(
        await invoiceFromLayoutAsync(pages),
        invoiceFromLayout(pages)
      );
      assert.deepEqual(
        await extractFormFieldsPerPageAsync(form),
        extractFormFieldsPerPage(form)
      );
      assert.deepEqual(
        await extractImagesPerPageAsync(dctScan),
        extractImagesPerPage(dctScan)
      );
      assert.equal(await pdfImageCountAsync(dctScan), pdfImageCount(dctScan));
      assert.deepEqual(await pdfDocumentAsync(invoice), pdfDocument(invoice));
    });

    it("should reject from a promise for what is not a PDF", {}, async () => {
      await assert.rejects(
        extractTextPerPageAsync(notPdf),
        /^Error: Failed to load PDF: /
      );
      assert.throws(
        () => extractTextPerPage(notPdf),
        /^Error: Failed to load PDF: /
      );
    });

    it(
      "should parse once in the class, in its constructor or from a promise",
      {},
      async () => {
        const doc = await PdfDown.openAsync(invoice);
        assert.ok(doc instanceof PdfDown);
        assert.deepEqual(doc.textPerPage(), new PdfDown(invoice).textPerPage());
        assert.deepEqual(await doc.documentAsync(), pdfDocument(invoice));
        assert.deepEqual(await doc.layoutAsync(), layoutPerPage(invoice));
      }
    );

    it("should take many calls at once", {}, async () => {
      // More waiting than any pool of workers has hands, and with no pool at all.
      const expected = layoutPerPage(layoutSample);
      const answers = await Promise.all(
        Array.from({ length: 64 }, () => layoutPerPageAsync(layoutSample))
      );
      for (const answer of answers) assert.deepEqual(answer, expected);
    });
  });
}
