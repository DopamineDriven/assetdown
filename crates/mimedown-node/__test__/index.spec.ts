import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { MimeTest } from "@/__test__/mime-test.ts";
import {
  MimeDown,
  builtWith,
  builtWithAsync,
  extensionsOf,
  extensionsOfAsync,
  keyframes,
  keyframesAsync,
  mediaTypeOfPath,
  mediaTypeOfPathAsync,
  mediaTypesOf,
  mediaTypesOfAsync,
  poster,
  sniff,
  sniffAsync,
  sniffFromRanges,
  sniffFromRangesAsync,
  specs,
  specsAsync,
  specsFromRanges,
  specsFromRangesAsync,
  waveform,
  waveformAsync
} from "@/index.js";

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

  it(
    "should give the same answer for a head from its promise",
    {},
    async () => {
      const head = [{ offset: 0, bytes: mimeTest.png.subarray(0, 8) }];
      assert.deepEqual(await sniffFromRangesAsync(head), sniffFromRanges(head));
    }
  );
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
        const { answer, fetches } = mimeTest.settle(
          specsFromRanges,
          file,
          1024
        );
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

describe("the promise of an answer from ranges", () => {
  const [name] = mimeTest.illustrator;
  assert.ok(name !== undefined);
  const file = mimeTest.getFixtureBuff(name);

  it("should be what the answer at once is, at each fetch", {}, async () => {
    const pieces = [{ offset: 0, bytes: file.subarray(0, 1024) }];
    const first = specsFromRanges(pieces, file.byteLength);
    assert.deepEqual(
      await specsFromRangesAsync(pieces, file.byteLength),
      first
    );
    assert.ok(first.needs !== null);
    const { offset, length } = first.needs;
    pieces.push({ offset, bytes: file.subarray(offset, offset + length) });
    assert.deepEqual(
      await specsFromRangesAsync(pieces, file.byteLength),
      specs(file)
    );
  });

  it(
    "should read the pieces as they were when it was asked",
    {},
    async () => {
      // The work runs on another thread, and a caller may reuse its buffer meanwhile.
      const bytes = Buffer.from(mimeTest.png);
      const asked = specsFromRangesAsync([{ offset: 0, bytes }]);
      bytes.fill(0);
      assert.equal((await asked).mime, "image/png");
    }
  );

  it(
    "should refuse what is wrong with the pieces where they are passed",
    {},
    () => {
      // Thrown at the call, as the answer at once throws it, and not from the promise.
      assert.throws(
        () => specsFromRangesAsync([{ offset: -1, bytes: mimeTest.png }]),
        { code: "InvalidArg" }
      );
      assert.throws(
        () => sniffFromRangesAsync([{ offset: 0, bytes: mimeTest.png }], 1.5),
        { code: "InvalidArg" }
      );
      // A piece that ends past the end of the file it is said to be a piece of.
      assert.throws(
        () => sniffFromRangesAsync([{ offset: 0, bytes: mimeTest.png }], 4),
        { code: "InvalidArg" }
      );
    }
  );
});

describe("the frames of a picture", () => {
  /** The type and the facts of a file the test expects to be a picture. */
  const picture = (file: Buffer) => {
    const found = specs(file);
    assert.ok(found.facts.type === "image", `${found.mime} is no picture`);
    return { mime: found.mime, ...found.facts.image };
  };
  /** What tells a still picture from one that moves. */
  const moves = (file: Buffer) => {
    const { mime, frames, animated } = picture(file);
    return [mime, frames, animated];
  };

  it("should tell a still picture from one that moves", {}, () => {
    assert.deepEqual(moves(mimeTest.pngOf()), ["image/png", 1, false]);
    assert.deepEqual(moves(mimeTest.pngOf(3)), ["image/apng", 3, true]);
    assert.deepEqual(moves(mimeTest.gifOf(1)), ["image/gif", 1, false]);
    assert.deepEqual(moves(mimeTest.gifOf(3)), ["image/gif", 3, true]);
    assert.deepEqual(moves(mimeTest.webpOf()), ["image/webp", 1, false]);
    assert.deepEqual(moves(mimeTest.webpOf(3)), ["image/webp", 3, true]);
  });

  it("should call an animation of one frame a still picture", {}, () => {
    assert.deepEqual(moves(mimeTest.pngOf(1)).slice(1), [1, false]);
    assert.deepEqual(moves(mimeTest.webpOf(1)).slice(1), [1, false]);
  });

  it(
    "should not count an icon's sizes or a TIFF's pages as frames",
    {},
    () => {
      // A set of sizes of one picture, and pages: neither moves. Each is counted in its
      // format's own detail.
      const icon = picture(mimeTest.icoOf([16, 32, 256]));
      assert.deepEqual([icon.frames, icon.animated], [1, false]);
      assert.ok(icon.detail.type === "ico");
      assert.deepEqual(
        icon.detail.ico.entries.map((entry) => entry.width),
        [16, 32, 256]
      );
      const scan = picture(mimeTest.tiffOf(3));
      assert.deepEqual([scan.frames, scan.animated], [1, false]);
      assert.ok(scan.detail.type === "tiff");
      assert.equal(scan.detail.tiff.pages, 3);
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

  it("should give the same answers from their promises", {}, async () => {
    for (const extension of ["jpg", ".PDF", "no-such-extension"]) {
      assert.deepEqual(
        await mediaTypesOfAsync(extension),
        mediaTypesOf(extension)
      );
    }
    for (const mime of ["image/jpeg", "no/such-type"]) {
      assert.deepEqual(await extensionsOfAsync(mime), extensionsOf(mime));
    }
    assert.equal(
      await mediaTypeOfPathAsync("a/b/report.final.pdf"),
      "application/pdf"
    );
    assert.equal(await mediaTypeOfPathAsync("a/b/no-extension"), null);
    assert.deepEqual(await builtWithAsync(), builtWith());
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
