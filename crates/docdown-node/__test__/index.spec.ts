import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { DocTest } from "@/__test__/doc-test.ts";
import {
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
} from "@/index.js";

const t = new DocTest();
const invoice = t.invoice;
const text =
  "Acme Tooling Ltd\nInvoice Number: INV-2026-0042\nInvoice Date: 2026-11-23\nTotal Due: 1,250.00";
const estimated =
  "word positions are estimated from the flow of the text; a Word file of this kind records no layout";

describe("docText", () => {
  it("should give a line to a paragraph", {}, () => {
    assert.equal(docText(invoice), text);
  });
  it("should resolve to the same text off the thread", {}, async () => {
    assert.equal(await docTextAsync(invoice), text);
  });
  it(
    "should throw unsupported format for a document saved with a password",
    {},
    async () => {
      const message = "unsupported format: an encrypted document";
      assert.throws(() => docText(t.locked), { message });
      await assert.rejects(docTextAsync(t.locked), { message });
    }
  );
  it("should throw unsupported format for a document of Word 95", {}, () => {
    assert.throws(() => docText(t.older), {
      message: "unsupported format: a Word 6 or 95 document"
    });
  });
  it("should throw malformed document for bytes that are not one", {}, () => {
    assert.throws(() => docText(Buffer.from("{\\rtf1 not this}")), {
      message: "malformed document: not a compound file"
    });
  });
});

describe("docLayout", () => {
  const pages = docLayout(invoice);
  it("should set the document on one page, a line to a paragraph", {}, () => {
    assert.equal(pages.length, 1);
    const [page] = pages;
    assert.ok(page, "the document has no page");
    assert.deepEqual(
      page.lines.map((line) => line.text),
      text.split("\n")
    );
    assert.equal(page.width, 612);
    assert.equal(page.height, 792);
  });
  it("should say that the positions are estimated", {}, () => {
    assert.deepEqual(pages[0]?.warnings, [estimated]);
  });
  it("should resolve to the same pages off the thread", {}, async () => {
    assert.deepEqual(await docLayoutAsync(invoice), pages);
  });
});

describe("docInvoice", () => {
  it("should read the invoice's fields from the pages", {}, async () => {
    const found = docInvoice(invoice);
    assert.equal(found.fields.invoiceId?.value, "INV-2026-0042");
    assert.deepEqual(await docInvoiceAsync(invoice), found);
  });
  it("should say first that the document carries a file", {}, () => {
    assert.equal(
      docInvoice(t.carrying).warnings[0],
      "the document carries 1 embedded file, which is not read here: the invoice may be in it"
    );
  });
});

describe("docEmbedded and docPictures", () => {
  it("should give a carried file as it was before it was embedded", {}, () => {
    const [file, ...rest] = docEmbedded(t.carrying);
    assert.ok(file, "the document carries no file");
    assert.equal(rest.length, 0);
    assert.equal(file.data.subarray(0, 13).toString("latin1"), "%PDF-1.4 held");
    assert.equal(file.name, undefined);
  });
  it("should give none for a document that carries none", {}, async () => {
    assert.deepEqual(docEmbedded(invoice), []);
    assert.deepEqual(await docEmbeddedAsync(invoice), []);
    assert.deepEqual(docPictures(invoice), []);
    assert.deepEqual(await docPicturesAsync(invoice), []);
  });
});

describe("docDocument", () => {
  const whole = docDocument(t.carrying);
  it("should give everything in one object", {}, () => {
    assert.equal(whole.text, text);
    assert.equal(whole.pages.length, 1);
    assert.equal(whole.embedded.length, 1);
    assert.deepEqual(whole.pictures, []);
    assert.deepEqual(whole.warnings, []);
    assert.equal(whole.isTemplate, false);
    assert.equal(whole.hasMacros, false);
    assert.equal(whole.invoice.fields.invoiceId?.value, "INV-2026-0042");
  });
  it("should resolve to the same object off the thread", {}, async () => {
    assert.deepEqual(await docDocumentAsync(t.carrying), whole);
  });
});

describe("DocDown", () => {
  const doc = new DocDown(t.carrying);
  it("should read from a document parsed once", {}, () => {
    assert.equal(doc.text(), text);
    assert.deepEqual(doc.layout(), docLayout(t.carrying));
    assert.deepEqual(doc.invoice(), docInvoice(t.carrying));
    assert.equal(doc.embedded().length, 1);
    assert.deepEqual(doc.pictures(), []);
    assert.deepEqual(doc.warnings(), []);
    assert.deepEqual(doc.document(), docDocument(t.carrying));
  });
  it("should do the same off the thread", {}, async () => {
    const opened = await DocDown.openAsync(t.carrying);
    assert.equal(opened instanceof DocDown, true);
    assert.deepEqual(await opened.layoutAsync(), doc.layout());
    assert.deepEqual(await opened.invoiceAsync(), doc.invoice());
    assert.deepEqual(await opened.documentAsync(), doc.document());
  });
  it(
    "should throw from the constructor and reject from openAsync for a locked document",
    {},
    async () => {
      const message = "unsupported format: an encrypted document";
      assert.throws(() => new DocDown(t.locked), { message });
      await assert.rejects(DocDown.openAsync(t.locked), { message });
    }
  );
});
