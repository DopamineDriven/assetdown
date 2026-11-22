import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import {
  MimeDown,
  builtWith,
  extensionsOf,
  keyframes,
  keyframesAsync,
  mediaTypeOfPath,
  mediaTypesOf,
  poster,
  sniff,
  sniffAsync,
  sniffFromRanges,
  specs,
  specsAsync,
  specsFromRanges,
  waveform,
  waveformAsync
} from "@/index.js";

export class MimeTest extends WorkupService {
  /** The four Illustrator documents among the fixtures. Each is a PDF under another name. */
  public get illustrator() {
    return Object.keys(this.fixturePaths);
  }

  /** The start of a PNG of one pixel: its signature and its header chunk, checksum and all. */
  public get png() {
    return Buffer.from(
      "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489",
      "hex"
    );
  }

  /** A WAVE file of half a second of silence: one channel, 16 bits, 8,000 samples a second. */
  public get wav() {
    const sound = 8000;
    const file = Buffer.alloc(44 + sound);
    file.write("RIFF", 0, "latin1");
    file.writeUInt32LE(36 + sound, 4);
    file.write("WAVEfmt ", 8, "latin1");
    file.writeUInt32LE(16, 16);
    file.writeUInt16LE(1, 20);
    file.writeUInt16LE(1, 22);
    file.writeUInt32LE(8000, 24);
    file.writeUInt32LE(16_000, 28);
    file.writeUInt16LE(2, 32);
    file.writeUInt16LE(16, 34);
    file.write("data", 36, "latin1");
    file.writeUInt32LE(sound, 40);
    return file;
  }

  /**
   * What a fetcher ends with: the head of `file`, then each range the answer asks for, until
   * it asks for nothing. With it, how many ranges that took.
   */
  public settle(file: Buffer, head: number) {
    const pieces = [{ offset: 0, bytes: file.subarray(0, head) }];
    let answer = specsFromRanges(pieces, file.byteLength);
    let fetches = 1;
    while (answer.needs !== null && fetches < 8) {
      const { offset, length } = answer.needs;
      pieces.push({ offset, bytes: file.subarray(offset, offset + length) });
      answer = specsFromRanges(pieces, file.byteLength);
      fetches += 1;
    }
    return { answer, fetches } as const;
  }
}

const mimeTest = new MimeTest();

describe("sniff", () => {
  it("should name an Illustrator document as the PDF it is", {}, () => {
    for (const name of mimeTest.illustrator) {
      const found = sniff(mimeTest.getFixtureBuff(name));
      assert.deepEqual(found, {
        mime: "application/pdf",
        topLevel: "application",
        ext: "pdf",
        parser: "pdfdown",
        evidence: "signature",
        retry: null,
        needs: null
      });
    }
  });

  it("should name a picture from its first bytes", {}, () => {
    const found = sniff(mimeTest.png);
    assert.equal(found.mime, "image/png");
    assert.equal(found.topLevel, "image");
    assert.equal(found.parser, null);
  });

  it("should name the package that reads a scanned page", {}, () => {
    // A TIFF's first eight bytes, and a directory with nothing in it.
    const tiff = Buffer.from([0x49, 0x49, 42, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    const found = sniff(tiff);
    assert.equal(found.mime, "image/tiff");
    assert.equal(found.parser, "tiffdown");
  });

  it("should name the package that reads a scan kept as a JPEG", {}, () => {
    // A JPEG's opening marker and the start of its first segment.
    const jpeg = Buffer.from([
      0xff, 0xd8, 0xff, 0xe0, 0, 16, 0x4a, 0x46, 0x49, 0x46, 0
    ]);
    const found = sniff(jpeg);
    assert.equal(found.mime, "image/jpeg");
    assert.equal(found.parser, "jpegdown");
  });

  it("should tell a form's data from the form", {}, () => {
    // It opens as a PDF does, with its own name in the place of `PDF`.
    const found = sniff(Buffer.from("%FDF-1.2\r%\xE2\xE3\xCF\xD3\r\n", "latin1"));
    assert.equal(found.mime, "application/vnd.fdf");
    assert.equal(found.ext, "fdf");
    assert.equal(found.parser, null);
  });

  it("should say that a file with nothing in it is empty", {}, () => {
    const found = sniff(Buffer.alloc(0));
    assert.equal(found.mime, "application/x-empty");
    // One byte is a file with something in it, of a kind the bytes do not name.
    assert.equal(sniff(Buffer.alloc(1)).mime, "application/octet-stream");
  });

  it("should name the package that reads a rich text document", {}, () => {
    const found = sniff(Buffer.from("{\\rtf1\\ansi Invoice 42\\par}", "latin1"));
    assert.equal(found.mime, "application/rtf");
    assert.equal(found.parser, "rtfdown");
  });

  it("should answer for bytes that name no type, and not throw", {}, () => {
    const found = sniff(Buffer.alloc(64));
    assert.equal(found.mime, "application/octet-stream");
    assert.equal(found.evidence, null);
  });

  it("should look again only when asked to", {}, () => {
    const behind = Buffer.concat([Buffer.alloc(100), mimeTest.png]);
    assert.equal(sniff(behind).evidence, null);
    const found = sniff(behind, { retryOnUnknown: true });
    assert.equal(found.mime, "image/png");
    assert.deepEqual(found.retry, { type: "leadingBytes", skipped: 100 });
  });

  it(
    "should take a file's name as a hint for text and nothing else",
    {},
    () => {
      const source = Buffer.from("fn main() {\n  let words = 3;\n}\n");
      const hinted = sniff(source, { fileName: "main.rs" });
      assert.equal(hinted.mime, "text/rust");
      assert.equal(hinted.evidence, "fileName");
      // A name does not make a picture into text.
      assert.equal(
        sniff(mimeTest.png, { fileName: "notes.md" }).mime,
        "image/png"
      );
    }
  );

  it("should give the same answer from its promise", {}, async () => {
    assert.deepEqual(await sniffAsync(mimeTest.png), sniff(mimeTest.png));
  });

  it("should ask for more when a head stops short", {}, () => {
    const head = mimeTest.png.subarray(0, 8);
    const found = sniffFromRanges([{ offset: 0, bytes: head }]);
    assert.equal(found.mime, "image/png");
    assert.notEqual(found.needs, null);
  });
});

describe("specs", () => {
  it("should describe an Illustrator document as a PDF", {}, () => {
    const { pdf } = builtWith();
    for (const name of mimeTest.illustrator) {
      const found = specs(mimeTest.getFixtureBuff(name));
      assert.equal(found.mime, "application/pdf");
      assert.deepEqual(found.warnings, []);
      assert.equal(found.facts.type, "document");
      if (found.facts.type !== "document") continue;
      const { document } = found.facts;
      assert.equal(document.detail.type, "pdf");
      if (document.detail.type !== "pdf") continue;
      assert.equal(document.detail.pdf.version, "1.6");
      // The rest is the PDF parser's, when the addon was built with it.
      assert.equal(document.pageCount, pdf ? 1 : null);
      assert.equal(document.detail.pdf.isEncrypted, pdf ? false : null);
      assert.equal(typeof document.imageCount, pdf ? "number" : "object");
    }
  });

  it("should describe a picture from its header", {}, () => {
    const found = specs(mimeTest.png);
    assert.equal(found.facts.type, "image");
    if (found.facts.type !== "image") return;
    const { image } = found.facts;
    assert.deepEqual(
      [image.width, image.height, image.hasAlpha, image.colorModel],
      [1, 1, true, "rgba"]
    );
    assert.equal(image.detail.type, "png");
  });

  it(
    "should give the whole file's answer from its head and what that asks for",
    {},
    () => {
      for (const name of mimeTest.illustrator) {
        const file = mimeTest.getFixtureBuff(name);
        const { answer, fetches } = mimeTest.settle(file, 1024);
        assert.equal(answer.needs, null);
        assert.ok(fetches <= 2, `${name} took ${fetches} fetches`);
        assert.deepEqual(answer, specs(file));
      }
    }
  );

  it(
    "should give the same answer from its promise and from the class",
    {},
    async () => {
      const [name] = mimeTest.illustrator;
      assert.ok(name !== undefined);
      const file = mimeTest.getFixtureBuff(name);
      const held = new MimeDown(file);
      assert.equal(held.byteSize, file.byteLength);
      assert.deepEqual(await specsAsync(file), specs(file));
      assert.deepEqual(held.specs(), specs(file));
      assert.deepEqual(await held.specsAsync(), specs(file));
      assert.deepEqual(held.sniff(), sniff(file));
    }
  );

  it(
    "should refuse a piece that says where it is with something other than a whole number",
    {},
    () => {
      assert.throws(
        () => specsFromRanges([{ offset: -1, bytes: mimeTest.png }]),
        { code: "InvalidArg" }
      );
      assert.throws(
        () => specsFromRanges([{ offset: 0, bytes: mimeTest.png }], 1.5),
        { code: "InvalidArg" }
      );
    }
  );
});

describe("the tables", () => {
  it("should go between a type and the extensions in use for it", {}, () => {
    assert.deepEqual(mediaTypesOf("jpg"), ["image/jpeg"]);
    assert.deepEqual(mediaTypesOf(".PDF"), ["application/pdf"]);
    assert.deepEqual(mediaTypesOf("no-such-extension"), []);
    assert.equal(extensionsOf("image/jpeg").at(0), "jpg");
    assert.equal(mediaTypeOfPath("a/b/report.final.pdf"), "application/pdf");
    assert.equal(mediaTypeOfPath("a/b/no-extension"), null);
  });
});

describe("what takes more than a header", () => {
  it(
    "should say that a document has no waveform, poster or keyframes",
    {},
    async () => {
      const [name] = mimeTest.illustrator;
      assert.ok(name !== undefined);
      const file = mimeTest.getFixtureBuff(name);
      const none = {
        type: "nothing",
        why: { type: "noReader", mime: "application/pdf" }
      };
      assert.deepEqual(waveform(file), none);
      assert.deepEqual(poster(file), none);
      assert.deepEqual(keyframes(file), none);
      assert.deepEqual(await keyframesAsync(file), none);
    }
  );

  it(
    "should give the shape of a sound, or say the addon was built without it",
    {},
    async () => {
      const shape = waveform(mimeTest.wav, { peakCount: 100 });
      if (!builtWith().audio) {
        assert.deepEqual(shape, {
          type: "nothing",
          why: { type: "featureOff", feature: "audio" }
        });
        return;
      }
      assert.equal(shape.type, "made");
      if (shape.type !== "made") return;
      const { waveform: made } = shape;
      assert.deepEqual(
        [made.sampleRate, made.channelCount, made.frameCount, made.durationMs],
        [8000, 1, 4000, 500]
      );
      assert.equal(made.peakCount, 100);
      assert.equal(made.boundaries.length, 101);
      assert.equal(made.channels.at(0)?.peak.length, 100);
      const later = await waveformAsync(mimeTest.wav, { peakCount: 100 });
      assert.equal(later.type, "made");
    }
  );

  it(
    "should refuse options that contradict each other or are not whole numbers",
    {},
    () => {
      assert.throws(
        () => waveform(mimeTest.wav, { peakCount: 10, samplesPerPeak: 10 }),
        { code: "InvalidArg" }
      );
      assert.throws(() => waveform(mimeTest.wav, { peakCount: 1.5 }), {
        code: "InvalidArg"
      });
      assert.throws(() => poster(mimeTest.wav, { quality: 0 }), {
        code: "InvalidArg"
      });
    }
  );
});
