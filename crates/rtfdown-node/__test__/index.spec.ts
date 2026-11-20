import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { RtfTest } from "@/__test__/rtf-test.ts";
import {
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
} from "@/index.js";

const rtfTest = new RtfTest();
const notRtf = Buffer.from("%PDF-1.7\n");
const estimated =
  "word positions are estimated from the flow of the text; an RTF file records no layout";

describe("rtfText", () => {
  it("should give a line to a paragraph and to a table's row", {}, () => {
    assert.equal(
      rtfText(rtfTest.invoice),
      [
        "INVOICE",
        "Invoice Number: INV-1001",
        "Invoice Date: 03/14/2024",
        "Description\tAmount",
        "Engine inspection\t$1,234.56",
        "Total: $1,234.56"
      ].join("\n")
    );
  });

  it("should read escapes as the characters they stand for", {}, () => {
    const escaped = rtfTest.document("caf\\'e9 \\u8364? 1\\emdash 2\\par");
    assert.equal(rtfText(escaped), "café € 1—2");
  });

  it("should leave out what is not the document's text", {}, () => {
    const file = rtfTest.document(
      "{\\info{\\title A title}}{\\header In the header\\par}Kept{\\*\\generator Some program;}\\par"
    );
    assert.equal(rtfText(file), "Kept");
  });

  it("should throw for bytes that are not RTF", {}, () => {
    assert.throws(
      () => rtfText(notRtf),
      /^Error: unsupported format: the file does not open as RTF does$/
    );
  });

  it("should give the same answer from its promise", {}, async () => {
    assert.equal(await rtfTextAsync(rtfTest.invoice), rtfText(rtfTest.invoice));
    await assert.rejects(rtfTextAsync(notRtf), /^Error: unsupported format: /);
  });
});

describe("rtfLayout", () => {
  it(
    "should set a paragraph's words from the margins of a letter page",
    {},
    () => {
      const [page, ...rest] = rtfLayout(rtfTest.document("Invoice 42\\par"));
      assert.deepEqual(rest, []);
      assert.ok(page);
      assert.deepEqual([page.page, page.width, page.height], [1, 612, 792]);
      assert.deepEqual(
        page.lines.map((line) => line.text),
        ["Invoice 42"]
      );
      assert.deepEqual(page.warnings, [estimated]);
      const [first] = page.words;
      assert.ok(first);
      // Margins of 90 points at the sides and 72 at the top; text of 12 points.
      assert.deepEqual(
        [first.text, first.x0, first.y1, first.line],
        ["Invoice", 90, 720, 0]
      );
      assert.equal(first.fontSize, 12);
    }
  );

  it(
    "should set a paragraph the file places where the file puts it",
    {},
    () => {
      const placed = rtfTest.document(
        "\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw2000 Total\\par\\pard\\pvpg\\phpg\\posx8000\\posy2880\\absw2000 5.00\\par"
      );
      const [page] = rtfLayout(placed);
      assert.ok(page);
      assert.deepEqual(
        page.lines.map((line) => line.text),
        ["Total 5.00"]
      );
      // 1,440 twentieths of a point from the left, and 2,880 down from the top.
      const [total, amount] = page.words;
      assert.ok(total && amount);
      assert.deepEqual([total.x0, total.y1], [72, 648]);
      assert.deepEqual([amount.x0, amount.y1], [400, 648]);
    }
  );

  it("should set the file's header and footer on every page", {}, () => {
    const pages = rtfLayout(
      rtfTest.document(
        "{\\header Letterhead\\par}{\\footer Foot\\par}One\\par\\page Two\\par"
      )
    );
    assert.deepEqual(
      pages.map((page) => page.lines.map((line) => line.text)),
      [
        ["Letterhead", "One", "Foot"],
        ["Letterhead", "Two", "Foot"]
      ]
    );
    // They are on the pages, and not in the text.
    assert.equal(
      rtfText(rtfTest.document("{\\header Letterhead\\par}One\\par")),
      "One"
    );
  });

  it("should set paragraphs that share a frame one under the other", {}, () => {
    const [page] = rtfLayout(
      rtfTest.document(
        "\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw4000 First\\par Second\\par"
      )
    );
    assert.ok(page);
    assert.deepEqual(
      page.lines.map((line) => line.text),
      ["First", "Second"]
    );
  });

  it("should turn a page at a page break", {}, () => {
    const pages = rtfLayout(rtfTest.document("One\\par\\page Two\\par"));
    assert.deepEqual(
      pages.map((page) => [page.page, page.lines[0]?.text]),
      [
        [1, "One"],
        [2, "Two"]
      ]
    );
  });

  it("should give the same answer from its promise", {}, async () => {
    assert.deepEqual(
      await rtfLayoutAsync(rtfTest.invoice),
      rtfLayout(rtfTest.invoice)
    );
  });
});

describe("rtfInvoice", () => {
  it("should read the invoice's fields from the pages", {}, async () => {
    const invoice = rtfInvoice(rtfTest.invoice);
    assert.equal(invoice.source, "Native");
    assert.equal(invoice.fields.invoiceId?.value, "INV-1001");
    assert.equal(invoice.fields.invoiceDate?.value, "2024-03-14");
    assert.equal(invoice.fields.invoiceTotal?.value, 1234.56);
    assert.deepEqual(await rtfInvoiceAsync(rtfTest.invoice), invoice);
  });

  it(
    "should say so when the document carries a file it does not read",
    {},
    () => {
      const carrying = rtfTest.document(
        `Please see attached.\\par ${rtfTest.object("Paint.Picture", Buffer.from("some data"))}`
      );
      assert.equal(
        rtfInvoice(carrying, { languages: ["en"] }).warnings[0],
        "the document carries 1 embedded file, which is not read here: the invoice may be in it"
      );
    }
  );
});

describe("rtfEmbedded", () => {
  it("should give a file the document carries as it was", {}, async () => {
    const held = Buffer.from("%PDF-1.4 the file itself");
    const file = rtfTest.document(rtfTest.object("Paint.Picture", held));
    const [found, ...rest] = rtfEmbedded(file);
    assert.deepEqual(rest, []);
    assert.ok(found);
    assert.equal(found.class, "Paint.Picture");
    assert.equal(found.name, undefined);
    assert.deepEqual(found.data, held);
    assert.deepEqual(await rtfEmbeddedAsync(file), rtfEmbedded(file));
  });

  it("should give nothing for a document that carries nothing", {}, () => {
    assert.deepEqual(rtfEmbedded(rtfTest.invoice), []);
  });
});

describe("rtfPictures", () => {
  it(
    "should give a picture with what it is and the size it is shown at",
    {},
    async () => {
      const file = rtfTest.document(
        `{\\pict\\pngblip\\picwgoal1440\\pichgoal2880 ${rtfTest.hex(rtfTest.png)}}`
      );
      const [picture, ...rest] = rtfPictures(file);
      assert.deepEqual(rest, []);
      assert.ok(picture);
      assert.equal(picture.format, "png");
      assert.deepEqual(picture.data, rtfTest.png);
      // Twentieths of a point, given in points.
      assert.deepEqual([picture.width, picture.height], [72, 144]);
      assert.deepEqual(await rtfPicturesAsync(file), rtfPictures(file));
    }
  );

  it(
    "should not count the picture that stands for an embedded object",
    {},
    () => {
      const file = rtfTest.document(
        rtfTest.object("Paint.Picture", Buffer.from("data"))
      );
      assert.deepEqual(rtfPictures(file), []);
    }
  );

  it(
    "should not believe a group that calls bytes a PNG when they are not one",
    {},
    () => {
      const file = rtfTest.document("{\\pict\\pngblip 9214000000}");
      assert.equal(rtfPictures(file)[0]?.format, "unknown");
    }
  );
});

describe("rtfDocument", () => {
  it("should give everything from one reading", {}, async () => {
    const file = rtfTest.document(
      `Invoice 42\\par {\\pict\\pngblip ${rtfTest.hex(rtfTest.png)}}${rtfTest.object("Paint.Picture", Buffer.from("data"))}{\\object\\objattph {\\*\\objclass Outlook.FileAttach}}`
    );
    const found = rtfDocument(file);
    assert.equal(found.text, rtfText(file));
    assert.deepEqual(found.pages, rtfLayout(file));
    assert.deepEqual(found.embedded, rtfEmbedded(file));
    assert.deepEqual(found.pictures, rtfPictures(file));
    assert.deepEqual(found.invoice, rtfInvoice(file));
    assert.deepEqual(found.warnings, [
      "an attachment has its place marked in the text, and is not in the file"
    ]);
    assert.deepEqual(await rtfDocumentAsync(file), found);
  });
});

describe("RtfDown", () => {
  it(
    "should answer as the functions do, from one reading of the file",
    {},
    async () => {
      const file = rtfTest.invoice;
      const rtf = new RtfDown(file);
      assert.equal(rtf.text(), rtfText(file));
      assert.deepEqual(rtf.layout(), rtfLayout(file));
      assert.deepEqual(await rtf.layoutAsync(), rtfLayout(file));
      assert.deepEqual(rtf.invoice(), rtfInvoice(file));
      assert.deepEqual(await rtf.invoiceAsync(), rtfInvoice(file));
      assert.deepEqual(rtf.embedded(), []);
      assert.deepEqual(rtf.pictures(), []);
      assert.deepEqual(rtf.warnings(), []);
      assert.deepEqual(rtf.document(), rtfDocument(file));
      assert.deepEqual(await rtf.documentAsync(), rtfDocument(file));
    }
  );

  it("should open from a promise", {}, async () => {
    const rtf = await RtfDown.openAsync(rtfTest.invoice);
    assert.ok(rtf instanceof RtfDown);
    assert.equal(rtf.text(), rtfText(rtfTest.invoice));
    await assert.rejects(
      RtfDown.openAsync(notRtf),
      /^Error: unsupported format: /
    );
  });

  it("should throw from its constructor for what is not RTF", {}, () => {
    assert.throws(() => new RtfDown(notRtf), /^Error: unsupported format: /);
  });
});
