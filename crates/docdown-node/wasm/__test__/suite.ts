import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Rm } from "@d0paminedriven/type-utils/utils";

import { DocTest } from "@/__test__/doc-test.ts";

/** What the package exports, as a caller of it sees. */
type Root = typeof import("@d0paminedriven/docdown-wasm");

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
    DocDown,
    docDocument,
    docDocumentAsync,
    docEmbedded,
    docEmbeddedAsync,
    docInvoice,
    docInvoiceAsync,
    docLayout,
    docLayoutAsync,
    docPictures,
    docPicturesAsync,
    docText,
    docTextAsync
  } = binding;
  const docTest = new DocTest();
  const notDoc = Buffer.from("%PDF-1.7\n");

  describe(`the WebAssembly package (${flavor})`, () => {
    const file = docTest.invoice;

    it(
      "should be the build it is said to be, and never a native addon",
      {},
      () => {
        assert.notEqual(binding.__napiBindingTarget, "native");
        assert.equal(binding.__napiBindingTarget, flavor);
      }
    );

    it("should read a document's text and set it on pages", {}, () => {
      assert.equal(
        docText(file).split("\n")[1],
        "Invoice Number: INV-2026-0042"
      );
      const [page] = docLayout(file);
      assert.ok(page);
      assert.deepEqual([page.width, page.height], [612, 792]);
      assert.equal(page.lines[0]?.text, "Acme Tooling Ltd");
    });

    it("should read the invoice's fields", {}, () => {
      const invoice = docInvoice(file);
      assert.equal(invoice.fields.invoiceId?.value, "INV-2026-0042");
      assert.equal(invoice.fields.invoiceDate?.value, "2026-11-23");
      assert.equal(invoice.fields.invoiceTotal?.value, 1250);
    });

    it("should give the files a document carries, and its pictures", {}, () => {
      const [embedded, ...rest] = docEmbedded(docTest.carrying);
      assert.ok(embedded);
      assert.equal(rest.length, 0);
      assert.equal(
        embedded.data.subarray(0, 13).toString("latin1"),
        "%PDF-1.4 held"
      );
      assert.deepEqual(docPictures(docTest.carrying), []);
    });

    it("should give the same answers from its promises", {}, async () => {
      const carrying = docTest.carrying;
      const [text, layout, invoice, embedded, pictures, document] =
        await Promise.all([
          docTextAsync(carrying),
          docLayoutAsync(carrying),
          docInvoiceAsync(carrying),
          docEmbeddedAsync(carrying),
          docPicturesAsync(carrying),
          docDocumentAsync(carrying)
        ]);
      assert.equal(text, docText(carrying));
      assert.deepEqual(layout, docLayout(carrying));
      assert.deepEqual(invoice, docInvoice(carrying));
      assert.deepEqual(embedded, docEmbedded(carrying));
      assert.deepEqual(pictures, docPictures(carrying));
      assert.deepEqual(document, docDocument(carrying));
    });

    it(
      "should reject from a promise for what is not a Word document",
      {},
      async () => {
        await assert.rejects(
          docTextAsync(notDoc),
          /^Error: malformed document: /
        );
        assert.throws(() => docText(notDoc), /^Error: malformed document: /);
        await assert.rejects(
          docTextAsync(docTest.locked),
          /^Error: unsupported format: /
        );
      }
    );

    it(
      "should parse once in the class, in its constructor or from a promise",
      {},
      async () => {
        const doc = await DocDown.openAsync(file);
        assert.ok(doc instanceof DocDown);
        assert.deepEqual(doc.document(), new DocDown(file).document());
        assert.deepEqual(await doc.documentAsync(), docDocument(file));
        assert.deepEqual(await doc.layoutAsync(), docLayout(file));
      }
    );

    it("should take many calls at once", {}, async () => {
      // More waiting than any pool of workers has hands, and with no pool at all.
      const texts = await Promise.all(
        Array.from({ length: 64 }, () => docTextAsync(file))
      );
      assert.deepEqual(new Set(texts), new Set([docText(file)]));
    });
  });
}
