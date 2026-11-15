import assert from "node:assert/strict";
import { describe, it } from "node:test";

import info from "@/__test__/fixtures/fixture-info.json" with { type: "json" };
import { WorkupService } from "@/__test__/workup.ts";
import {
  AudioDown,
  __napiBindingTarget,
  analyzeAudio,
  analyzeAudioAsync,
  decodeAudio,
  decodeAudioAsync,
  parseAudio,
  parseAudioAsync,
  waveformPeaks,
  waveformPeaksAsync
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
// WorkupService extends Fs
class WasiAudioTest extends WorkupService {
  /** The item at `index`, failing the test when there is none. */
  public at<const T>(items: readonly T[], index: number) {
    const item = items[index];
    if (item === undefined) {
      assert.fail(`no item at index ${index} of ${items.length}`);
    }
    return item;
  }
}

const t = new WasiAudioTest();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const cbr = t.getFixtureBuff("cbr.mp3");
  const stereo = t.getFixtureBuff("stereo.wav");
  const exactFloat = t.getFixtureBuff("exact-float.wav");
  const notAudio = Buffer.from("not audio at all");

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });
  it("should read the same specs as the native build", {}, () => {
    assert.partialDeepStrictEqual(parseAudio(cbr, "cbr.mp3"), {
      kind: "mp3",
      source: "cbr.mp3",
      byteSize: 17230,
      sampleRate: 44100,
      channels: 2,
      bitrateKbps: 128,
      frames: 40,
      sampleFrames: 46080,
      title: "Precision fixture",
      id3v2: { version: "2.4.0", size: 95 },
      xing: { kind: "Info", frames: 40, bytes: 17135 }
    });
    assert.partialDeepStrictEqual(parseAudio(stereo), {
      kind: "wav",
      codec: "pcm",
      container: "RIFF",
      durationSec: 1,
      sampleFrames: info.frames,
      byteRate: 176400,
      dataOffset: 44
    });
  });
  it(
    "should decode float samples exactly and trim an MP3 to its original length",
    {},
    () => {
      assert.deepEqual([...decodeAudio(exactFloat).samples], info.floatSamples);
      assert.partialDeepStrictEqual(decodeAudio(cbr), {
        sampleRate: info.sampleRate,
        channels: info.channels,
        frameCount: info.frames,
        gaplessEnabled: true
      });
    }
  );
  it(
    "should reduce a waveform to the same peaks on one thread and on many",
    {},
    () => {
      const peaks = waveformPeaks(stereo, { peakCount: 4 });
      assert.deepEqual([...peaks.boundaries], [0, 11025, 22050, 33075, 44100]);
      assert.deepEqual(
        [...peaks.envelope],
        [0.54998779296875, 0.54998779296875, 0.9765625, 0.54998779296875]
      );
      assert.equal(t.at(peaks.channels, 0).rms instanceof Float64Array, true);
      assert.deepEqual(
        waveformPeaks(cbr, { peakCount: 64, parallel: true }),
        waveformPeaks(cbr, { peakCount: 64, parallel: false })
      );
    }
  );
  it(
    "should resolve each async form to what its sync twin returns",
    {},
    async () => {
      assert.deepEqual(await parseAudioAsync(cbr), parseAudio(cbr));
      assert.deepEqual(await decodeAudioAsync(cbr), decodeAudio(cbr));
      assert.deepEqual(
        await waveformPeaksAsync(stereo, { peakCount: 16 }),
        waveformPeaks(stereo, { peakCount: 16 })
      );
      assert.deepEqual(
        await analyzeAudioAsync(cbr, { peakCount: 16 }),
        analyzeAudio(cbr, { peakCount: 16 })
      );
      const audio = new AudioDown(cbr);
      assert.deepEqual(await audio.metadataAsync(), audio.metadata());
      assert.equal((await audio.pcmAsync()).frameCount, info.frames);
    }
  );
  it(
    "should return the same peaks to 32 readers running at once",
    {},
    async () => {
      const audio = new AudioDown(cbr);
      const expected = audio.waveform({ peakCount: 32 });
      const readers = Array.of<Promise<WaveformPeaks>>();
      for (let i = 0; i < 16; i++) {
        readers.push(audio.waveformAsync({ peakCount: 32 }));
        readers.push(waveformPeaksAsync(cbr, { peakCount: 32 }));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should throw from the sync forms and reject from the async ones for bytes that are not audio",
    {},
    async () => {
      const message = "unrecognized audio magic";
      assert.throws(() => parseAudio(notAudio), { message });
      assert.throws(() => new AudioDown(notAudio), { message });
      await assert.rejects(parseAudioAsync(notAudio), { message });
      await assert.rejects(decodeAudioAsync(notAudio), { message });
    }
  );
});
