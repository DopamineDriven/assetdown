import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import {
  PdfDown,
  __napiBindingTarget,
  capabilities,
  extractInvoiceAsync,
  extractTextPerPage,
  extractTextPerPageAsync,
  layoutPerPageAsync,
  pdfDocumentAsync
} from "@/index.js";
import type { PageLayout } from "@/index.js";

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
class WasiPdfTest extends WorkupService {
  /** The PNG signature every extracted image starts with. */
  public get pngMagic() {
    return [0x89, 0x50, 0x4e, 0x47] as const;
  }

  public isPng(data: Buffer) {
    return this.pngMagic.every((byte, i) => data[i] === byte);
  }

  /** The item at `index`, failing the test when there is none. */
  public at<const T>(items: readonly T[], index: number) {
    const item = items[index];
    if (item === undefined) {
      assert.fail(`no item at index ${index} of ${items.length}`);
    }
    return item;
  }
}

const t = new WasiPdfTest();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const invoice = t.getFixtureBuff(
    "invoices/wordpress-pdf-invoice-plugin-sample"
  );
  const layoutSample = t.getFixtureBuff("layout/layout-sample");
  const dctScan = t.getFixtureBuff("ascii85-dct-scan");
  const form = t.getFixtureBuff("forms/acroform-basic");

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });
  it("should report neither OCR nor rendering", {}, () => {
    assert.deepEqual(capabilities(), { ocr: false, render: false });
  });
  it("should return a PdfDown that reads metadata and text", {}, () => {
    const doc = new PdfDown(invoice);
    assert.equal(doc instanceof PdfDown, true);
    const meta = doc.metadata();
    assert.equal(meta.version, "1.3");
    assert.equal(meta.pageCount, 1);
    const box = t.at(meta.pageBoxes, 0);
    assert.equal(Math.round(box.width), 595);
    assert.equal(Math.round(box.height), 842);
    assert.ok(t.at(doc.textPerPage(), 0).text.includes("INV-3337"));
  });
  it("should make each layout line address a slice of the words", {}, () => {
    const pages = new PdfDown(layoutSample).layout();
    assert.equal(pages.length, 2);
    for (const page of pages) {
      assert.ok(page.words.length > 0);
      for (const [index, line] of page.lines.entries()) {
        const words = page.words.slice(
          line.firstWord,
          line.firstWord + line.wordCount
        );
        assert.equal(words.map((w) => w.text).join(" "), line.text);
        for (const word of words) {
          assert.equal(word.line, index);
        }
      }
    }
  });
  it("should decode a JPEG scan into a PNG", {}, () => {
    const image = t.at(new PdfDown(dctScan).imagesPerPage(), 0);
    assert.equal(image.page, 1);
    assert.ok(image.width > 0 && image.height > 0);
    assert.equal(t.isPng(image.data), true);
  });
  it("should read form fields and the invoice number", {}, () => {
    assert.ok(new PdfDown(form).formFieldsPerPage().length > 0);
    const result = new PdfDown(invoice).invoice();
    assert.equal(result.source, "Native");
    assert.equal(result.fields.invoiceId?.value, "INV-3337");
  });
  it(
    "should resolve openAsync on a worker thread to the same text as the constructor",
    {},
    async () => {
      const opened = await PdfDown.openAsync(invoice);
      assert.equal(opened instanceof PdfDown, true);
      assert.deepEqual(opened.textPerPage(), new PdfDown(invoice).textPerPage());
    }
  );
  it("should resolve each async reader to what its sync twin returns", {}, async () => {
    const doc = new PdfDown(layoutSample);
    assert.deepEqual(await doc.textPerPageAsync(), doc.textPerPage());
    assert.deepEqual(await doc.layoutAsync(), doc.layout());
    assert.deepEqual(await doc.metadataAsync(), doc.metadata());
    assert.deepEqual(await doc.documentAsync(), doc.document());
    assert.deepEqual(
      await extractTextPerPageAsync(layoutSample),
      extractTextPerPage(layoutSample)
    );
    assert.deepEqual(await pdfDocumentAsync(layoutSample), doc.document());
    assert.deepEqual(
      await extractInvoiceAsync(invoice),
      new PdfDown(invoice).invoice()
    );
  });
  it(
    "should return the same layout to 32 readers running at once",
    {},
    async () => {
      const doc = new PdfDown(layoutSample);
      const expected = doc.layout();
      const readers = Array.of<Promise<PageLayout[]>>();
      for (let i = 0; i < 16; i++) {
        readers.push(doc.layoutAsync());
        readers.push(layoutPerPageAsync(layoutSample));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should throw from the constructor and reject from openAsync for bytes that are not a PDF",
    {},
    async () => {
      const bytes = Buffer.from("not a pdf");
      assert.throws(() => new PdfDown(bytes), {
        message: /^Failed to load PDF/
      });
      await assert.rejects(PdfDown.openAsync(bytes), {
        message: /^Failed to load PDF/
      });
    }
  );
});
