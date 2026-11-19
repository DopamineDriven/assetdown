import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { UTR } from "@d0paminedriven/type-utils";

import { WorkupService } from "@/__test__/workup.ts";
import {
  PdfDown,
  attachments,
  attachmentsAsync,
  capabilities,
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
} from "@/index.js";
import type { PaperSize } from "@/index.js";

// WorkupService extends Fs
class PdfTest extends WorkupService {
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

const t = new PdfTest();
const invoice = t.getFixtureBuff(
  "invoices/wordpress-pdf-invoice-plugin-sample"
);
const layoutSample = t.getFixtureBuff("layout/layout-sample");
const dctScan = t.getFixtureBuff("ascii85-dct-scan");
const ccitt = t.getFixtureBuff("ccitt-g4-tiny");
const form = t.getFixtureBuff("forms/acroform-basic");
const doc = new PdfDown(invoice);

describe("PdfDown constructor", () => {
  it("should return a PdfDown for a PDF", {}, () => {
    assert.equal(doc instanceof PdfDown, true);
  });
  it("should throw Failed to load PDF for bytes that are not a PDF", {}, () => {
    assert.throws(() => new PdfDown(Buffer.from("not a pdf")), {
      message: /^Failed to load PDF/
    });
  });
  it(
    "openAsync should resolve to a PdfDown that reads the same",
    {},
    async () => {
      const opened = await PdfDown.openAsync(invoice);
      assert.equal(opened instanceof PdfDown, true);
      assert.deepEqual(opened.textPerPage(), doc.textPerPage());
    }
  );
  it("openAsync should reject for bytes that are not a PDF", {}, async () => {
    await assert.rejects(PdfDown.openAsync(Buffer.from("not a pdf")), {
      message: /^Failed to load PDF/
    });
  });
});

describe("capabilities", () => {
  it("should report no OCR in the base package", {}, () => {
    const caps = capabilities();
    assert.equal(caps.ocr, false);
    assert.equal(typeof caps.render, "boolean");
  });
});

describe("metadata", () => {
  const meta = doc.metadata();

  it("should report the version, page count and one page box", {}, () => {
    assert.equal(meta.version, "1.3");
    assert.equal(meta.pageCount, 1);
    assert.equal(meta.pageBoxes.length, 1);
    const [box] = meta.pageBoxes;
    assert.ok(box);
    assert.equal(box.boxType, "MediaBox");
    assert.equal(box.pageCount, 1);
    assert.equal(Math.round(box.width), 595);
    assert.equal(Math.round(box.height), 842);
  });
  it("should list a second box for pages of another size", {}, () => {
    const boxes = new PdfDown(layoutSample).metadata().pageBoxes;
    assert.equal(boxes.length, 2);
    assert.equal(
      boxes.reduce((sum, b) => sum + b.pageCount, 0),
      2
    );
  });
  it(
    "should name the standard paper size each page box is nearest to",
    {},
    () => {
      // The union turns into a record keyed by `type`, each member still tied to its key.
      const a4 = {
        type: "A4",
        width: 595,
        height: 842
      } satisfies UTR<PaperSize, "type">["A4"];
      const letter = {
        type: "LETTER",
        width: 612,
        height: 792
      } satisfies UTR<PaperSize, "type">["LETTER"];
      assert.deepEqual(t.at(meta.pageBoxes, 0).paperSize, a4);
      assert.deepEqual(
        t.at(new PdfDown(form).metadata().pageBoxes, 0).paperSize,
        letter
      );
      // The second page of this one is 300 by 500 points, which is no standard size. Only an
      // unknown size says which two it lies between.
      const unknown = {
        type: "UNKNOWN",
        closestTo: [
          { type: "A6", width: 298, height: 420 },
          { type: "B6", width: 354, height: 499 }
        ]
      } satisfies UTR<PaperSize, "type">["UNKNOWN"];
      assert.deepEqual(
        new PdfDown(layoutSample).metadata().pageBoxes.map((b) => b.paperSize),
        [letter, unknown]
      );
      assert.equal("closestTo" in a4, false);
    }
  );
  it("should carry AcroForm facts when the document has a form", {}, () => {
    assert.ok(new PdfDown(form).metadata().form);
    assert.equal(meta.form, undefined);
  });
  it("should say what the document calls itself, as the file writes it", {}, () => {
    assert.equal(meta.title, "PDF Invoice Example");
    assert.equal(meta.author, "Sliced Invoices");
    assert.equal(
      meta.subject,
      "Example PDF invoice that was created using the Sli"
    );
    const scan = new PdfDown(dctScan).metadata();
    assert.equal(scan.title, "Sample PDF - scanned, image only");
    assert.equal(scan.author, "ConvertICO.com");
    assert.equal(
      scan.subject,
      "Free sample PDF from convertico.com/samples/pdf/"
    );
    // What a file does not say is left out, not made up: neither states keywords or a language,
    // and the form states nothing of itself at all.
    assert.equal(meta.keywords, undefined);
    assert.equal(meta.language, undefined);
    const plain = new PdfDown(form).metadata();
    assert.deepEqual(
      [plain.title, plain.author, plain.subject, plain.keywords],
      [undefined, undefined, undefined, undefined]
    );
  });
  it("should say whether the file was encrypted on disk", {}, () => {
    assert.equal(meta.isEncrypted, false);
    assert.equal(new PdfDown(form).metadata().isEncrypted, false);
    // Encrypted with an empty password: it opens, and still says that it was.
    const locked = new PdfDown(t.getFixtureBuff("forms/acroform-basic-aes256"));
    assert.equal(locked.metadata().isEncrypted, true);
  });
  it("the async and free-function forms should agree", {}, async () => {
    assert.deepEqual(await doc.metadataAsync(), meta);
    assert.deepEqual(pdfMetadata(invoice), meta);
    assert.deepEqual(await pdfMetadataAsync(invoice), meta);
  });
});

describe("text", () => {
  const pages = doc.textPerPage();

  it("should return one entry per page, numbered from 1", {}, () => {
    assert.deepEqual(
      pages.map((p) => p.page),
      [1]
    );
    const [first] = pages;
    assert.ok(first);
    assert.equal(first.text.length, 681);
  });
  it("should split each page into header, body and footer", {}, () => {
    const [structured] = doc.structuredText();
    assert.ok(structured);
    assert.equal(structured.page, 1);
    assert.equal(typeof structured.header, "string");
    assert.equal(typeof structured.footer, "string");
    assert.ok(structured.body.length > 0);
  });
  it("the async and free-function forms should agree", {}, async () => {
    assert.deepEqual(await doc.textPerPageAsync(), pages);
    assert.deepEqual(extractTextPerPage(invoice), pages);
    assert.deepEqual(await extractTextPerPageAsync(invoice), pages);
    const structured = doc.structuredText();
    assert.deepEqual(await doc.structuredTextAsync(), structured);
    assert.deepEqual(extractStructuredTextPerPage(invoice), structured);
    assert.deepEqual(
      await extractStructuredTextPerPageAsync(invoice),
      structured
    );
  });
});

describe("images", () => {
  const images = doc.imagesPerPage();

  it("should return each painted image as a PNG with its facts", {}, () => {
    assert.equal(images.length, 1);
    const [image] = images;
    assert.ok(image);
    assert.equal(image.page, 1);
    assert.equal(image.imageIndex, 0);
    assert.equal(image.width, 300);
    assert.equal(image.height, 45);
    assert.equal(image.filter, "FlateDecode");
    assert.equal(image.colorSpace, "ICCBased3");
    assert.equal(t.isPng(image.data), true);
    assert.match(image.objectId, /^[0-9]+ [0-9]+ obj$/);
  });
  it(
    "should decode a JPEG stored behind an ASCII85 transport filter",
    {},
    () => {
      const [scan] = new PdfDown(dctScan).imagesPerPage();
      assert.ok(scan);
      assert.equal(scan.filter, "DCTDecode");
      assert.equal(scan.width, 1240);
      assert.equal(scan.height, 1754);
      assert.equal(t.isPng(scan.data), true);
    }
  );
  it("should count the images the pages paint without decoding them", {}, async () => {
    assert.equal(doc.imageCount(), 1);
    assert.equal(doc.imageCount(), images.length);
    // The same picture painted on two pages is two.
    assert.equal(new PdfDown(layoutSample).imageCount(), 2);
    assert.equal(new PdfDown(form).imageCount(), 0);
    assert.equal(await doc.imageCountAsync(), 1);
    assert.equal(pdfImageCount(invoice), 1);
    assert.equal(await pdfImageCountAsync(invoice), 1);
  });
  it("should decode a CCITT Group 4 fax image", {}, () => {
    const [fax] = new PdfDown(ccitt).imagesPerPage();
    assert.ok(fax);
    assert.equal(fax.filter, "CCITTFaxDecode");
    assert.equal(fax.width, 8);
    assert.equal(fax.height, 2);
    assert.equal(t.isPng(fax.data), true);
  });
  it("the async and free-function forms should agree", {}, async () => {
    assert.deepEqual(await doc.imagesPerPageAsync(), images);
    assert.deepEqual(extractImagesPerPage(invoice), images);
    assert.deepEqual(await extractImagesPerPageAsync(invoice), images);
  });
});

describe("annotations", () => {
  const annotations = doc.annotationsPerPage();

  it("should return the page's links with their targets", {}, () => {
    assert.equal(annotations.length, 4);
    const [first] = annotations;
    assert.ok(first);
    assert.equal(first.page, 1);
    assert.equal(first.subtype, "Link");
    assert.equal(first.uri, "http://slicedinvoices.com/demo");
    assert.equal(first.rect.length, 4);
  });
  it("should list form widgets as annotations", {}, () => {
    const widgets = new PdfDown(form).annotationsPerPage();
    assert.equal(widgets.length, 11);
    assert.ok(widgets.every((w) => w.subtype === "Widget"));
  });
  it("the async and free-function forms should agree", {}, async () => {
    assert.deepEqual(await doc.annotationsPerPageAsync(), annotations);
    assert.deepEqual(extractAnnotationsPerPage(invoice), annotations);
    assert.deepEqual(
      await extractAnnotationsPerPageAsync(invoice),
      annotations
    );
  });
});

describe("document", () => {
  const whole = doc.document();

  it("should gather what the separate readers return", {}, () => {
    assert.deepEqual(whole.text, doc.textPerPage());
    assert.deepEqual(whole.structuredText, doc.structuredText());
    assert.deepEqual(whole.images, doc.imagesPerPage());
    assert.deepEqual(whole.annotations, doc.annotationsPerPage());
    assert.deepEqual(whole.formFields, doc.formFieldsPerPage());
    const meta = doc.metadata();
    assert.equal(whole.pageCount, meta.pageCount);
    assert.equal(whole.version, meta.version);
    assert.deepEqual(whole.pageBoxes, meta.pageBoxes);
    // What the document says of itself comes with it, as `metadata()` gives it.
    assert.deepEqual(
      [whole.title, whole.author, whole.subject, whole.keywords],
      [meta.title, meta.author, meta.subject, meta.keywords]
    );
    assert.equal(whole.title, "PDF Invoice Example");
    assert.equal(whole.language, meta.language);
    assert.equal(whole.isEncrypted, false);
  });
  it(
    "should count images, annotations and form fields and name their pages",
    {},
    () => {
      assert.equal(whole.totalImages, 1);
      assert.equal(whole.totalAnnotations, 4);
      assert.equal(whole.totalFormFields, 0);
      assert.deepEqual(whole.imagePages, [1]);
      assert.deepEqual(whole.annotationPages, [1]);
      const withForm = new PdfDown(form).document();
      assert.equal(withForm.totalFormFields, 11);
      assert.deepEqual(withForm.formFieldPages, [1]);
    }
  );
  it("the async and free-function forms should agree", {}, async () => {
    assert.deepEqual(await doc.documentAsync(), whole);
    assert.deepEqual(pdfDocument(invoice), whole);
    assert.deepEqual(await pdfDocumentAsync(invoice), whole);
  });
});

describe("async readers", () => {
  it(
    "should not be affected when the caller overwrites its buffer after the call",
    {},
    async () => {
      const scratch = Buffer.from(invoice);
      const pending = extractTextPerPageAsync(scratch);
      scratch.fill(0);
      assert.deepEqual(await pending, doc.textPerPage());
    }
  );
  it(
    "should give the same results when many run at once on one document",
    {},
    async () => {
      const expected = doc.document();
      const readers = Array.of<Promise<typeof expected>>();
      for (let i = 0; i < 12; i++) {
        readers.push(doc.documentAsync());
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
});

describe("form fields", () => {
  const fields = new PdfDown(form).formFieldsPerPage();

  it(
    "should list every placed widget in reading order with its kind",
    {},
    () => {
      assert.deepEqual(
        fields.map((f) => [f.name, f.type]),
        [
          ["name", "Text"],
          ["notes", "Text"],
          ["zip", "Text"],
          ["agree", "Checkbox"],
          ["color", "Radio"],
          ["color", "Radio"],
          ["color", "Radio"],
          ["country", "ComboBox"],
          ["tags", "ListBox"],
          ["sig", "Signature"],
          ["submit", "PushButton"]
        ]
      );
    }
  );
  it("should narrow on type to the fields that kind carries", {}, () => {
    const name = t.at(fields, 0);
    if (name.type !== "Text") {
      assert.fail(`expected a Text field, got ${name.type}`);
    }
    assert.equal(name.value, "Jane Doe");
    assert.equal(name.multiline, false);
    assert.equal(name.page, 1);
    assert.equal(name.rect.length, 4);
    assert.equal(name.widgetCount, 1);
  });
  it("should report the form's facts in the metadata", {}, () => {
    const info = new PdfDown(form).metadata().form;
    assert.ok(info);
    assert.equal(info.fieldCount, 9);
    assert.equal(info.widgetCount, 11);
    assert.equal(info.hasXfa, false);
    assert.equal(info.needAppearances, true);
    assert.deepEqual(info.unplacedFields, []);
    assert.equal(info.truncated, false);
  });
  it("should read an encrypted form the same as the plain one", {}, () => {
    const plain = fields.map((f) => [f.name, f.type, f.value]);
    for (const name of [
      "forms/acroform-basic-aes256",
      "forms/acroform-basic-rc4"
    ] as const) {
      const encrypted = new PdfDown(t.getFixtureBuff(name)).formFieldsPerPage();
      assert.deepEqual(
        encrypted.map((f) => [f.name, f.type, f.value]),
        plain,
        name
      );
    }
  });
  it("should keep page order across a large form", {}, () => {
    const many = new PdfDown(t.getFixtureBuff("forms/acroform-many"));
    const all = many.formFieldsPerPage();
    assert.equal(all.length, 1500);
    for (const [index, field] of all.entries()) {
      const previous = all[index - 1];
      assert.ok(previous === undefined || previous.page <= field.page);
    }
    assert.equal(many.document().formFieldPages.length, 30);
  });
  it("the async and free-function forms should agree", {}, async () => {
    const doc = new PdfDown(form);
    assert.deepEqual(await doc.formFieldsPerPageAsync(), fields);
    assert.deepEqual(extractFormFieldsPerPage(form), fields);
    assert.deepEqual(await extractFormFieldsPerPageAsync(form), fields);
  });
});

describe("layout", () => {
  const pages = new PdfDown(layoutSample).layout();
  const first = t.at(pages, 0);

  it(
    "should describe each page: its size, rotation and where its text came from",
    {},
    () => {
      assert.deepEqual(
        pages.map((p) => [
          p.page,
          p.width,
          p.height,
          p.rotate,
          p.contentRotation,
          p.source
        ]),
        [
          [1, 612, 792, 0, 0, "Native"],
          [2, 300, 500, 90, 0, "Native"]
        ]
      );
    }
  );
  it(
    "should read the lines top to bottom, whatever order the content stream paints them",
    {},
    () => {
      assert.deepEqual(
        first.lines.map((l) => l.text),
        [
          "Layout Sample",
          "Item Qty Amount",
          "Widget 2 10,00",
          "Gadget 5 25,00",
          "Word spacing applies here",
          "Inside a form XObject",
          "Page 1 of 2"
        ]
      );
    }
  );
  it("should give every word its box, its line and its font", {}, () => {
    assert.deepEqual(t.at(first.words, 0), {
      text: "Layout",
      x0: 50,
      y0: 736.274,
      x1: 109.004,
      y1: 752.924,
      line: 0,
      fontSize: 18,
      fontName: "Helvetica-Bold",
      bold: true,
      italic: false,
      symbolic: false
    });
  });
  it("should make each line address a slice of the words", {}, () => {
    for (const page of pages) {
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
  it("should say where each image was painted", {}, () => {
    assert.deepEqual(first.images, [
      { xobjectName: "Im1", x0: 400, y0: 650, x1: 500, y1: 700 }
    ]);
  });
  it(
    "should report weight only when the font gives evidence, and flag barcode fonts",
    {},
    () => {
      const words = t.at(
        new PdfDown(t.getFixtureBuff("layout/fonts-sample")).layout(),
        0
      ).words;
      const barcode = t.at(words, 0);
      assert.equal(barcode.symbolic, true);
      const anonymous = words.find((w) => w.text === "Anonymous");
      assert.ok(anonymous);
      assert.equal(anonymous.bold, undefined);
      assert.equal(anonymous.italic, undefined);
      const bold = words.find((w) => w.text === "Bold");
      assert.ok(bold);
      assert.equal(bold.bold, true);
    }
  );
  it("the async and free-function forms should agree", {}, async () => {
    const doc = new PdfDown(layoutSample);
    assert.deepEqual(await doc.layoutAsync(), pages);
    assert.deepEqual(layoutPerPage(layoutSample), pages);
    assert.deepEqual(await layoutPerPageAsync(layoutSample), pages);
  });
});

describe("attachments", () => {
  const ubl = t.getFixtureBuff("einvoice/ubl-attachments");
  const files = new PdfDown(ubl).attachments();

  it(
    "should list embedded files with their type, size, relationship and page",
    {},
    () => {
      assert.deepEqual(
        files.map((f) => [f.name, f.mimeType, f.size, f.relationship, f.page]),
        [
          ["invoice.xml", "application/xml", 3352, "Data", undefined],
          ["readme.txt", "text/plain", 33, undefined, undefined],
          ["notes.csv", "text/csv", 14, undefined, 2]
        ]
      );
      for (const file of files) {
        assert.equal(file.data.length, file.size);
      }
    }
  );
  it("should return none for a document without any", {}, () => {
    assert.deepEqual(doc.attachments(), []);
  });
  it("the async and free-function forms should agree", {}, async () => {
    assert.deepEqual(await new PdfDown(ubl).attachmentsAsync(), files);
    assert.deepEqual(attachments(ubl), files);
    assert.deepEqual(await attachmentsAsync(ubl), files);
  });
});

describe("invoice", () => {
  const facturx = t.getFixtureBuff("einvoice/facturx-minimal");

  it(
    "should read embedded Factur-X XML when the document carries it",
    {},
    () => {
      const result = extractInvoice(facturx);
      assert.equal(result.source, "EmbeddedXml");
      assert.equal(result.documentType, "Invoice");
      assert.equal(result.confidence, 1);
      assert.equal(result.fields.invoiceId?.value, "INV-2026-0042");
      assert.equal(result.fields.invoiceDate?.value, "2026-11-01");
      assert.equal(result.fields.invoiceTotal?.value, 833);
      assert.equal(result.fields.invoiceTotal?.currencyCode, "EUR");
      assert.equal(result.fields.vendorName?.value, "Acme Aviation Parts GmbH");
      assert.equal(result.fields.items?.length, 2);
      assert.equal(result.embeddedXml?.filename, "factur-x.xml");
      assert.equal(result.embeddedXml?.standard, "FacturX");
      assert.deepEqual(result.warnings, []);
    }
  );
  it("should recognize XRechnung in a UBL attachment", {}, () => {
    const result = extractInvoice(t.getFixtureBuff("einvoice/ubl-attachments"));
    assert.equal(result.source, "EmbeddedXml");
    assert.equal(result.embeddedXml?.standard, "XRechnung");
    assert.equal(result.fields.invoiceId?.value, "UBL-77");
    assert.equal(result.fields.invoiceTotal?.value, 108);
    assert.equal(result.fields.invoiceTotal?.currencyCode, "USD");
  });
  it(
    "should say so, and find nothing, when the embedded XML is broken and the pages are empty",
    {},
    () => {
      const result = extractInvoice(
        t.getFixtureBuff("einvoice/einvoice-broken")
      );
      assert.equal(result.source, "None");
      assert.equal(result.documentType, "Unknown");
      assert.equal(result.confidence, 0);
      assert.equal(result.warnings.length, 3);
    }
  );
  it("should read the pages when there is no embedded XML", {}, () => {
    const result = extractInvoice(t.getFixtureBuff("invoices/cluster-a"));
    assert.equal(result.source, "Native");
    assert.equal(result.fields.invoiceId?.value, "A-10045");
    assert.equal(result.fields.invoiceDate?.value, "2024-03-14");
    assert.equal(result.fields.invoiceTotal?.value, 13321.5);
    assert.equal(result.fields.vendorName?.value, "ACME AVIATION SERVICES");
    assert.ok(
      result.warnings.some((w) => w.includes("no embedded e-invoice XML"))
    );
  });
  it(
    "should recognize a credit note and report its total as a positive amount",
    {},
    () => {
      const result = extractInvoice(t.getFixtureBuff("invoices/credit-memo"));
      assert.equal(result.documentType, "CreditNote");
      assert.equal(result.fields.invoiceId?.value, "CM-2201");
      assert.equal(result.fields.invoiceTotal?.value, 1200);
      assert.ok(result.creditSignals.length > 0);
    }
  );
  it("should leave the XML alone when preferEmbeddedXml is false", {}, () => {
    const result = extractInvoice(facturx, { preferEmbeddedXml: false });
    assert.notEqual(result.source, "EmbeddedXml");
    assert.equal(result.embeddedXml, undefined);
  });
  it("invoiceFromLayout should read pages a caller supplies", {}, async () => {
    const pages = layoutPerPage(invoice);
    const fromPages = invoiceFromLayout(pages);
    assert.equal(fromPages.source, "Native");
    assert.equal(fromPages.fields.invoiceId?.value, "INV-3337");
    assert.deepEqual(
      fromPages.fields,
      extractInvoice(invoice, { preferEmbeddedXml: false }).fields
    );
    assert.deepEqual(await invoiceFromLayoutAsync(pages), fromPages);
  });
  it("the class, async and free-function forms should agree", {}, async () => {
    const expected = extractInvoice(facturx);
    const held = new PdfDown(facturx);
    assert.deepEqual(held.invoice(), expected);
    assert.deepEqual(await held.invoiceAsync(), expected);
    assert.deepEqual(await extractInvoiceAsync(facturx), expected);
  });
});
