import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Rm } from "@d0paminedriven/type-utils/utils";

import { MimeTest } from "@/__test__/mime-test.ts";

/** What the package exports, as a caller of it sees. */
type Root = typeof import("@d0paminedriven/mimedown-wasm");

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
    posterAsync,
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
  } = binding;
  const mimeTest = new MimeTest();

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

  describe(`the WebAssembly package (${flavor})`, () => {
    const [name] = mimeTest.illustrator;
    assert.ok(name !== undefined);
    /** An Illustrator document, which is a PDF under another name. */
    const illustrator = mimeTest.getFixtureBuff(name);

    it(
      "should be the build it is said to be, and never a native addon",
      {},
      () => {
        assert.notEqual(binding.__napiBindingTarget, "native");
        assert.equal(binding.__napiBindingTarget, flavor);
      }
    );

    it("should be built with every parser the main package has", {}, () => {
      assert.deepEqual(builtWith(), {
        pdf: true,
        office: true,
        audio: true,
        video: true
      });
    });

    it("should name a type from its bytes", {}, () => {
      assert.deepEqual(sniff(illustrator), {
        mime: "application/pdf",
        topLevel: "application",
        ext: "pdf",
        parser: "pdfdown",
        evidence: "signature",
        retry: null,
        needs: null
      });
      assert.partialDeepStrictEqual(sniff(mimeTest.png), {
        mime: "image/png",
        topLevel: "image",
        ext: "png",
        parser: null
      });
      assert.equal(sniff(Buffer.alloc(0)).mime, "application/x-empty");
      assert.equal(sniff(Buffer.alloc(64)).mime, "application/octet-stream");
      // Behind leading bytes a file is found only when that is asked for.
      const behind = Buffer.concat([Buffer.alloc(100), mimeTest.png]);
      assert.equal(sniff(behind).evidence, null);
      assert.deepEqual(sniff(behind, { retryOnUnknown: true }).retry, {
        type: "leadingBytes",
        skipped: 100
      });
      // A file's name is a hint for text, and for nothing else.
      const source = Buffer.from("fn main() {\n  let words = 3;\n}\n");
      assert.equal(sniff(source, { fileName: "main.rs" }).mime, "text/rust");
    });

    it("should describe a document with the parser that reads it", {}, () => {
      const described = specs(illustrator);
      assert.equal(described.mime, "application/pdf");
      assert.deepEqual(described.warnings, []);
      assert.ok(described.facts.type === "document");
      const { document } = described.facts;
      assert.ok(document.detail.type === "pdf");
      assert.equal(document.detail.pdf.version, "1.6");
      assert.equal(document.pageCount, 1);
      assert.equal(document.detail.pdf.isEncrypted, false);
    });

    it("should tell a still picture from one that moves", {}, () => {
      assert.deepEqual(moves(mimeTest.pngOf()), ["image/png", 1, false]);
      assert.deepEqual(moves(mimeTest.pngOf(3)), ["image/apng", 3, true]);
      assert.deepEqual(moves(mimeTest.gifOf(1)), ["image/gif", 1, false]);
      assert.deepEqual(moves(mimeTest.gifOf(3)), ["image/gif", 3, true]);
      assert.deepEqual(moves(mimeTest.webpOf()), ["image/webp", 1, false]);
      assert.deepEqual(moves(mimeTest.webpOf(3)), ["image/webp", 3, true]);
    });

    it(
      "should not count an icon's sizes or a TIFF's pages as frames",
      {},
      () => {
        const icon = picture(mimeTest.icoOf([16, 32, 256]));
        assert.deepEqual([icon.frames, icon.animated], [1, false]);
        assert.ok(icon.detail.type === "ico");
        assert.equal(icon.detail.ico.entries.length, 3);
        const scan = picture(mimeTest.tiffOf(3));
        assert.deepEqual([scan.frames, scan.animated], [1, false]);
        assert.ok(scan.detail.type === "tiff");
        assert.equal(scan.detail.tiff.pages, 3);
      }
    );

    it(
      "should give the whole file's answer from its head and what that asks for",
      {},
      () => {
        const { answer, fetches } = mimeTest.settle(
          specsFromRanges,
          illustrator,
          1024
        );
        assert.equal(answer.needs, null);
        assert.ok(fetches <= 2, `took ${fetches} fetches`);
        assert.deepEqual(answer, specs(illustrator));
        // A head that stops short names the type, and says what would settle the rest.
        const head = [{ offset: 0, bytes: mimeTest.png.subarray(0, 8) }];
        assert.equal(sniffFromRanges(head).mime, "image/png");
        assert.notEqual(sniffFromRanges(head).needs, null);
      }
    );

    it("should go between a type and the extensions in use for it", {}, () => {
      assert.deepEqual(mediaTypesOf("jpg"), ["image/jpeg"]);
      assert.deepEqual(mediaTypesOf(".PDF"), ["application/pdf"]);
      assert.equal(extensionsOf("image/jpeg").at(0), "jpg");
      assert.equal(mediaTypeOfPath("a/b/report.final.pdf"), "application/pdf");
      assert.equal(mediaTypeOfPath("a/b/no-extension"), null);
    });

    it("should give the tables' answers from their promises", {}, async () => {
      assert.deepEqual(await mediaTypesOfAsync(".PDF"), mediaTypesOf(".PDF"));
      assert.deepEqual(await mediaTypesOfAsync("no-such-extension"), []);
      assert.deepEqual(
        await extensionsOfAsync("image/jpeg"),
        extensionsOf("image/jpeg")
      );
      assert.equal(
        await mediaTypeOfPathAsync("a/b/report.final.pdf"),
        "application/pdf"
      );
      assert.equal(await mediaTypeOfPathAsync("a/b/no-extension"), null);
      assert.deepEqual(await builtWithAsync(), builtWith());
    });

    it("should give the same answer from each promise", {}, async () => {
      const head = [{ offset: 0, bytes: illustrator.subarray(0, 1024) }];
      const size = illustrator.byteLength;
      assert.deepEqual(await sniffAsync(illustrator), sniff(illustrator));
      assert.deepEqual(await specsAsync(illustrator), specs(illustrator));
      assert.deepEqual(
        await sniffFromRangesAsync(head, size),
        sniffFromRanges(head, size)
      );
      assert.deepEqual(
        await specsFromRangesAsync(head, size),
        specsFromRanges(head, size)
      );
      assert.deepEqual(
        await keyframesAsync(illustrator),
        keyframes(illustrator)
      );
      assert.deepEqual(await posterAsync(illustrator), poster(illustrator));
    });

    it(
      "should hold a file in the class for more than one question",
      {},
      async () => {
        const held = new MimeDown(illustrator);
        assert.ok(held instanceof MimeDown);
        assert.equal(held.byteSize, illustrator.byteLength);
        assert.deepEqual(held.sniff(), sniff(illustrator));
        assert.deepEqual(await held.sniffAsync(), sniff(illustrator));
        assert.deepEqual(held.specs(), specs(illustrator));
        assert.deepEqual(await held.specsAsync(), specs(illustrator));
        assert.deepEqual(await held.keyframesAsync(), held.keyframes());
      }
    );

    it("should give the shape of a sound", {}, async () => {
      const shape = waveform(mimeTest.wav, { peakCount: 100 });
      assert.ok(shape.type === "made");
      const { waveform: made } = shape;
      assert.deepEqual(
        [made.sampleRate, made.channelCount, made.frameCount, made.durationMs],
        [8000, 1, 4000, 500]
      );
      assert.equal(made.peakCount, 100);
      assert.equal(made.boundaries.length, 101);
      assert.equal(made.channels.at(0)?.peak.length, 100);
      assert.deepEqual(
        await waveformAsync(mimeTest.wav, { peakCount: 100 }),
        shape
      );
    });

    it(
      "should say that a document has no waveform, poster, or keyframes",
      {},
      () => {
        const none = {
          type: "nothing",
          why: { type: "noReader", mime: "application/pdf" }
        };
        assert.deepEqual(waveform(illustrator), none);
        assert.deepEqual(poster(illustrator), none);
        assert.deepEqual(keyframes(illustrator), none);
      }
    );

    it("should refuse what a caller got wrong, where it was passed", {}, () => {
      const wrong = { code: "InvalidArg" };
      const { png, wav } = mimeTest;
      assert.throws(() => specsFromRanges([{ offset: -1, bytes: png }]), wrong);
      assert.throws(
        () => specsFromRanges([{ offset: 0, bytes: png }], 1.5),
        wrong
      );
      // The promise verbs throw it at the call too, and not from the promise.
      assert.throws(
        () => specsFromRangesAsync([{ offset: -1, bytes: png }]),
        wrong
      );
      assert.throws(
        () => sniffFromRangesAsync([{ offset: 0, bytes: png }], 4),
        wrong
      );
      assert.throws(
        () => waveform(wav, { peakCount: 10, samplesPerPeak: 10 }),
        wrong
      );
      assert.throws(() => poster(wav, { quality: 0 }), wrong);
    });

    it("should take many calls at once", {}, async () => {
      // More waiting than any pool of workers has hands, and with no pool at all.
      const answers = await Promise.all(
        Array.from({ length: 64 }, () => specsAsync(illustrator))
      );
      const expected = specs(illustrator);
      for (const answer of answers) assert.deepEqual(answer, expected);
    });
  });
}
