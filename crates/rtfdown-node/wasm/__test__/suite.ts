import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Rm } from "@d0paminedriven/type-utils/utils";

import { RtfTest } from "@/__test__/rtf-test.ts";

/** What the package exports, as a caller of it sees. */
type Root = typeof import("@d0paminedriven/rtfdown-wasm");

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
    RtfDown,
    rtfDocument,
    rtfDocumentAsync,
    rtfEmbedded,
    rtfEmbeddedAsync,
    rtfInvoice,
    rtfInvoiceAsync,
    rtfLayout,
    rtfLayoutAsync,
    rtfPictures,
    rtfPicturesAsync,
    rtfText,
    rtfTextAsync
  } = binding;
  const rtfTest = new RtfTest();
  const notRtf = Buffer.from("%PDF-1.7\n");

  describe(`the WebAssembly package (${flavor})`, () => {
    const file = rtfTest.invoice;

    it(
      "should be the build it is said to be, and never a native addon",
      {},
      () => {
        assert.notEqual(binding.__napiBindingTarget, "native");
        assert.equal(binding.__napiBindingTarget, flavor);
      }
    );

    it("should read a document's text and set it on pages", {}, () => {
      assert.equal(rtfText(file).split("\n")[1], "Invoice Number: INV-1001");
      const [page] = rtfLayout(file);
      assert.ok(page);
      assert.deepEqual([page.width, page.height], [612, 792]);
      assert.equal(page.lines[0]?.text, "INVOICE");
    });

    it("should read the invoice's fields", {}, () => {
      const invoice = rtfInvoice(file);
      assert.equal(invoice.fields.invoiceId?.value, "INV-1001");
      assert.equal(invoice.fields.invoiceDate?.value, "2024-03-14");
      assert.equal(invoice.fields.invoiceTotal?.value, 1234.56);
    });

    it("should give the files and the pictures a document carries", {}, () => {
      const held = Buffer.from("%PDF-1.4 the file itself");
      const carrying = rtfTest.document(
        `{\\pict\\pngblip ${rtfTest.hex(rtfTest.png)}}${rtfTest.object("Paint.Picture", held)}`
      );
      const [embedded] = rtfEmbedded(carrying);
      assert.ok(embedded);
      assert.deepEqual(embedded.data, held);
      const [picture] = rtfPictures(carrying);
      assert.ok(picture);
      assert.equal(picture.format, "png");
      assert.deepEqual(picture.data, rtfTest.png);
    });

    it("should give the same answers from its promises", {}, async () => {
      const [text, layout, invoice, embedded, pictures, document] =
        await Promise.all([
          rtfTextAsync(file),
          rtfLayoutAsync(file),
          rtfInvoiceAsync(file),
          rtfEmbeddedAsync(file),
          rtfPicturesAsync(file),
          rtfDocumentAsync(file)
        ]);
      assert.equal(text, rtfText(file));
      assert.deepEqual(layout, rtfLayout(file));
      assert.deepEqual(invoice, rtfInvoice(file));
      assert.deepEqual(embedded, rtfEmbedded(file));
      assert.deepEqual(pictures, rtfPictures(file));
      assert.deepEqual(document, rtfDocument(file));
    });

    it("should reject from a promise for what is not RTF", {}, async () => {
      await assert.rejects(
        rtfTextAsync(notRtf),
        /^Error: unsupported format: /
      );
      assert.throws(() => rtfText(notRtf), /^Error: unsupported format: /);
    });

    it(
      "should parse once in the class, in its constructor or from a promise",
      {},
      async () => {
        const rtf = await RtfDown.openAsync(file);
        assert.ok(rtf instanceof RtfDown);
        assert.deepEqual(rtf.document(), new RtfDown(file).document());
        assert.deepEqual(await rtf.documentAsync(), rtfDocument(file));
        assert.deepEqual(await rtf.layoutAsync(), rtfLayout(file));
      }
    );

    it("should take many calls at once", {}, async () => {
      // More waiting than any pool of workers has hands, and with no pool at all.
      const texts = await Promise.all(
        Array.from({ length: 64 }, () => rtfTextAsync(file))
      );
      assert.deepEqual(new Set(texts), new Set([rtfText(file)]));
    });
  });
}
