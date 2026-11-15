import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import {
  VideoDown,
  __napiBindingTarget,
  analyzeVideo,
  analyzeVideoAsync,
  audioPeaks,
  audioPeaksAsync,
  coverArt,
  coverArtAsync,
  decodeAudio,
  decodeAudioAsync,
  keyframes,
  keyframesAsync,
  parseVideo,
  parseVideoAsync,
  poster,
  posterAsync
} from "@/index.js";
import type { WaveformPeaks } from "@/index.js";

/**
 * The WebAssembly build.
 *
 * `yarn test:wasi` sets `NAPI_RS_FORCE_WASI=error`, which makes the generated
 * loader require the wasm binding and throw when there is none. Under plain
 * `yarn test` the native addon loads and this suite skips itself.
 *
 * Two things differ from native and get the attention here. The async forms
 * run their work on the runtime's own worker and not on a blocking pool, and
 * the waveform reduction fans out over rayon threads that are wasm workers.
 */
const t = new WorkupService();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const faststart = t.getFixtureBuff("h264-aac-faststart.mp4");
  const coverMkv = t.getFixtureBuff("h264-aac-cover.mkv");
  const vorbis = t.getFixtureBuff("vp9-vorbis.webm");
  const silent = t.getFixtureBuff("silent.mp4");
  const notVideo = Buffer.from("not a video at all");

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });
  it("should read the same specs as the native build", {}, () => {
    assert.partialDeepStrictEqual(parseVideo(faststart, "a.mp4"), {
      kind: "mp4",
      source: "a.mp4",
      container: "isobmff",
      byteSize: 191556,
      complete: true,
      moovBeforeMdat: true,
      creationTime: "2026-10-14T12:00:00Z",
      durationSec: 4,
      width: 320,
      height: 180,
      frameRate: 30,
      frameCount: 120,
      videoCodec: "h264",
      audioCodec: "aac",
      audioSampleRate: 48000,
      keyframeCount: 4,
      keyframeSource: "stss"
    });
    assert.partialDeepStrictEqual(parseVideo(vorbis), {
      kind: "webm",
      container: "matroska",
      videoCodec: "vp9",
      audioCodec: "vorbis",
      keyframeSource: "cues"
    });
  });
  it("should index keyframes and return cover art", {}, () => {
    assert.deepEqual(
      keyframes(faststart).timesMs,
      Uint32Array.of(0, 1000, 2000, 3000)
    );
    const art = coverArt(coverMkv);
    assert.ok(art);
    assert.partialDeepStrictEqual(art, {
      mime: "image/jpeg",
      width: 320,
      height: 180
    });
    assert.equal(Buffer.compare(art.data, t.getFixtureBuff("cover.jpg")), 0);
  });
  it(
    "should make a poster from a keyframe of H.264 and of HEVC",
    {},
    async () => {
      const made = poster(faststart);
      assert.ok(made);
      assert.partialDeepStrictEqual(made, {
        mime: "image/jpeg",
        width: 320,
        height: 180,
        timeSec: 0
      });
      assert.deepEqual([...made.data.subarray(0, 3)], [0xff, 0xd8, 0xff]);
      assert.partialDeepStrictEqual(poster(t.getFixtureBuff("hevc-aac.mp4")), {
        width: 320,
        height: 180
      });
      assert.equal(poster(vorbis), null);
      assert.deepEqual(
        await posterAsync(faststart, { format: "png", timeSec: 2 }),
        poster(faststart, { format: "png", timeSec: 2 })
      );
    }
  );
  it(
    "should decode the audio track and reduce it the same on one thread and on many",
    {},
    () => {
      assert.partialDeepStrictEqual(decodeAudio(faststart), {
        sampleRate: 48000,
        channels: 2,
        frameCount: 193536
      });
      assert.equal(decodeAudio(silent), null);
      assert.equal(
        decodeAudio(t.getFixtureBuff("vp9-opus.webm"))?.frameCount,
        192960
      );
      assert.equal(
        decodeAudio(t.getFixtureBuff("hevc-aac.mp4"))?.frameCount,
        193536
      );
      const peaks = audioPeaks(faststart, { peakCount: 4 });
      assert.deepEqual(
        [...(peaks?.boundaries ?? [])],
        [0, 48384, 96768, 145152, 193536]
      );
      assert.deepEqual(
        audioPeaks(vorbis, { peakCount: 64, parallel: true }),
        audioPeaks(vorbis, { peakCount: 64, parallel: false })
      );
    }
  );
  it(
    "should resolve each async form to what its sync twin returns",
    {},
    async () => {
      assert.deepEqual(await parseVideoAsync(faststart), parseVideo(faststart));
      assert.deepEqual(await keyframesAsync(vorbis), keyframes(vorbis));
      assert.deepEqual(await coverArtAsync(coverMkv), coverArt(coverMkv));
      assert.deepEqual(await decodeAudioAsync(vorbis), decodeAudio(vorbis));
      assert.deepEqual(
        await audioPeaksAsync(faststart, { peakCount: 16 }),
        audioPeaks(faststart, { peakCount: 16 })
      );
      assert.deepEqual(
        await analyzeVideoAsync(coverMkv, { peakCount: 16 }),
        analyzeVideo(coverMkv, { peakCount: 16 })
      );
      const video = new VideoDown(faststart);
      assert.deepEqual(await video.metadataAsync(), video.metadata());
      assert.equal(await new VideoDown(silent).audioPeaksAsync(), null);
    }
  );
  it(
    "should return the same peaks to 32 readers running at once",
    {},
    async () => {
      const video = new VideoDown(faststart);
      const expected = video.audioPeaks({ peakCount: 32 });
      const readers = Array.of<Promise<WaveformPeaks | null>>();
      for (let i = 0; i < 16; i++) {
        readers.push(video.audioPeaksAsync({ peakCount: 32 }));
        readers.push(audioPeaksAsync(faststart, { peakCount: 32 }));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should throw from the sync forms and reject from the async ones for bytes that are not a video",
    {},
    async () => {
      const message = "unrecognized video magic";
      assert.throws(() => parseVideo(notVideo), { message });
      assert.throws(() => new VideoDown(notVideo), { message });
      await assert.rejects(parseVideoAsync(notVideo), { message });
      await assert.rejects(keyframesAsync(notVideo), { message });
    }
  );
});
