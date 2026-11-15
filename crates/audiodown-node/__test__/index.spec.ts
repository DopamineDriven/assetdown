import assert from "node:assert/strict";
import { describe, it } from "node:test";

import info from "@/__test__/fixtures/fixture-info.json" with { type: "json" };
import { WorkupService } from "@/__test__/workup.ts";
import {
  AudioDown,
  AudioPcm,
  AudioService,
  analyzeAudio,
  analyzeAudioAsync,
  decodeAudio,
  decodeAudioAsync,
  parseAudio,
  parseAudioAsync,
  parseMp3,
  parseMp3Async,
  parseWav,
  parseWavAsync,
  sniffAudio,
  waveformFromPcm,
  waveformFromPcmAsync,
  waveformPeaks,
  waveformPeaksAsync
} from "@/index.js";

// WorkupService extends Fs
class AudiodownTestService extends WorkupService {
  /** The MP3 fixtures that come with FFmpeg's decode of the same file. */
  public get referenced() {
    return ["cbr.mp3", "vbr.mp3", "no-xing.mp3"] as const;
  }

  /** The WAV fixtures that hold one second of 44.1 kHz stereo, one per sample format. */
  public get stereoSeconds() {
    return ["stereo.wav", "pcm24.wav", "float.wav", "rf64.wav"] as const;
  }

  /** `<name>.reference.f32`: FFmpeg's decode, as little-endian 32-bit floats. */
  public reference(name: (typeof this.referenced)[number]) {
    const bytes = this.getFixtureBuff(`${name}.reference.f32`);
    return Float32Array.from({ length: bytes.length / 4 }, (_, i) =>
      bytes.readFloatLE(i * 4)
    );
  }

  /** The largest and the root-mean-square difference between two runs of samples. */
  public difference(samples: Float32Array, reference: Float32Array) {
    let max = 0;
    let energy = 0;
    for (const [i, sample] of samples.entries()) {
      const error = sample - (reference[i] ?? Number.NaN);
      max = Math.max(max, Math.abs(error));
      energy += error * error;
    }
    return { max, rms: Math.sqrt(energy / samples.length) };
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

const t = new AudiodownTestService();
const cbr = t.getFixtureBuff("cbr.mp3");
const vbr = t.getFixtureBuff("vbr.mp3");
const noXing = t.getFixtureBuff("no-xing.mp3");
const stereo = t.getFixtureBuff("stereo.wav");
const exactFloat = t.getFixtureBuff("exact-float.wav");
const notAudio = Buffer.from("not audio at all");

describe("sniffAudio", () => {
  it("should name the kind from the first bytes", {}, () => {
    assert.equal(sniffAudio(cbr), "mp3");
    assert.equal(sniffAudio(t.getFixtureBuff("mpeg2-mono.mp3")), "mp3");
    assert.equal(sniffAudio(stereo), "wav");
    assert.equal(sniffAudio(t.getFixtureBuff("rf64.wav")), "wav");
  });
  it("should return null for bytes that are neither", {}, () => {
    assert.equal(sniffAudio(notAudio), null);
    assert.equal(sniffAudio(Buffer.alloc(0)), null);
  });
});

describe("parseAudio", () => {
  it("should describe an MP3 from its frames and its tags", {}, () => {
    const specs = parseAudio(cbr);
    assert.partialDeepStrictEqual(specs, {
      type: "AUDIO",
      kind: "mp3",
      format: "mp3",
      ext: "mp3",
      mime: "audio/mpeg",
      mimeType: "audio/mpeg",
      contentType: "audio/mpeg",
      codec: "mp3",
      container: "mpeg",
      byteSize: 17230,
      durationMs: 1045,
      durationSource: "frames",
      sampleRate: 44100,
      channels: 2,
      bitrateKbps: 128,
      cbr: true,
      vbr: false,
      frames: 40,
      sampleFrames: 46080,
      truncated: false,
      title: "Precision fixture",
      artist: "audiodown",
      album: null,
      tags: { title: "Precision fixture", artist: "audiodown" },
      metadata: { title: "Precision fixture", artist: "audiodown" },
      id3v2: { version: "2.4.0", size: 95 },
      xing: { kind: "Info", frames: 40, bytes: 17135 },
      audioOffset: 95,
      frame: {
        offset: 95,
        version: "1",
        layer: "III",
        bitrateKbps: 128,
        sampleRate: 44100,
        channels: 2,
        channelMode: "stereo",
        padding: false,
        hasCrc: false,
        samplesPerFrame: 1152,
        frameSize: 417
      },
      formatTag: null,
      blockAlign: null
    });
    assert.ok(Math.abs((specs.durationSec ?? 0) - 46080 / 44100) < 1e-12);
  });
  it(
    "should take a variable bitrate from the Xing header, and count frames without one",
    {},
    () => {
      assert.partialDeepStrictEqual(parseAudio(vbr), {
        cbr: false,
        vbr: true,
        frames: 40,
        xing: { kind: "Xing", frames: 40, bytes: 8928 }
      });
      assert.partialDeepStrictEqual(parseAudio(noXing), {
        cbr: true,
        frames: 40,
        durationSource: "frames",
        xing: null
      });
    }
  );
  it("should read MPEG-2 mono", {}, () => {
    assert.partialDeepStrictEqual(
      parseAudio(t.getFixtureBuff("mpeg2-mono.mp3")),
      {
        sampleRate: 22050,
        channels: 1,
        frames: 41,
        frame: { version: "2", channelMode: "mono", samplesPerFrame: 576 }
      }
    );
  });
  it(
    "should take the duration from the Xing header when the frames are cut off",
    {},
    () => {
      const specs = parseAudio(vbr.subarray(0, 600));
      assert.equal(specs.durationSource, "xing");
      assert.equal(specs.frames, null);
      assert.ok(Math.abs((specs.durationSec ?? 0) - 46080 / 44100) < 1e-12);
    }
  );
  it("should read the format chunk of a WAV", {}, () => {
    assert.partialDeepStrictEqual(parseAudio(stereo), {
      type: "AUDIO",
      kind: "wav",
      mime: "audio/wav",
      codec: "pcm",
      container: "RIFF",
      byteSize: 176444,
      durationSec: 1,
      durationSource: "data",
      sampleRate: info.sampleRate,
      channels: info.channels,
      sampleFrames: info.frames,
      bitsPerSample: 16,
      bitrateKbps: 1411,
      formatTag: 1,
      byteRate: 176400,
      blockAlign: 4,
      dataOffset: 44,
      dataSize: 176400,
      frame: null,
      xing: null,
      id3v2: null
    });
  });
  it("should name the codec and container of each WAV flavor", {}, () => {
    assert.partialDeepStrictEqual(parseAudio(t.getFixtureBuff("float.wav")), {
      codec: "ieee-float",
      formatTag: 65534,
      bitsPerSample: 32,
      durationSource: "fact",
      tags: { software: "Lavf58.76.100" }
    });
    assert.partialDeepStrictEqual(parseAudio(t.getFixtureBuff("pcm24.wav")), {
      codec: "pcm",
      bitsPerSample: 24,
      blockAlign: 6
    });
    assert.partialDeepStrictEqual(
      parseAudio(t.getFixtureBuff("pcm_alaw.wav")),
      { codec: "alaw", formatTag: 6, sampleRate: 8000, channels: 1 }
    );
    assert.partialDeepStrictEqual(
      parseAudio(t.getFixtureBuff("pcm_mulaw.wav")),
      { codec: "mulaw", formatTag: 7, sampleRate: 8000, channels: 1 }
    );
    assert.partialDeepStrictEqual(parseAudio(t.getFixtureBuff("rf64.wav")), {
      codec: "pcm",
      container: "RF64",
      sampleFrames: info.frames
    });
  });
  it(
    "should carry the source it was given and leave it out otherwise",
    {},
    () => {
      assert.equal(
        parseAudio(cbr, "s3://bucket/a.mp3").source,
        "s3://bucket/a.mp3"
      );
      assert.equal("source" in parseAudio(cbr), false);
    }
  );
  it("parseMp3 and parseWav should each refuse the other kind", {}, () => {
    assert.equal(parseMp3(cbr).kind, "mp3");
    assert.equal(parseWav(stereo).kind, "wav");
    assert.throws(() => parseMp3(stereo), { message: "not an MP3 buffer" });
    assert.throws(() => parseWav(cbr), { message: "not a WAVE buffer" });
  });
  it("should throw unrecognized audio magic for anything else", {}, () => {
    assert.throws(() => parseAudio(notAudio), {
      message: "unrecognized audio magic"
    });
    assert.throws(() => parseAudio(Buffer.alloc(0)), {
      message: "unrecognized audio magic"
    });
  });
  it("the async forms should agree", {}, async () => {
    assert.deepEqual(
      await parseAudioAsync(cbr, "a.mp3"),
      parseAudio(cbr, "a.mp3")
    );
    assert.deepEqual(await parseMp3Async(vbr), parseMp3(vbr));
    assert.deepEqual(await parseWavAsync(stereo), parseWav(stereo));
    await assert.rejects(parseAudioAsync(notAudio), {
      message: "unrecognized audio magic"
    });
  });
});

describe("decodeAudio", () => {
  it(
    "should keep float samples exactly, those above full scale included",
    {},
    () => {
      const decoded = decodeAudio(exactFloat);
      assert.deepEqual([...decoded.samples], info.floatSamples);
      assert.equal(decoded.samples instanceof Float32Array, true);
      assert.partialDeepStrictEqual(decoded, {
        sampleRate: 8000,
        channels: 1,
        frameCount: info.floatSamples.length
      });
    }
  );
  it("should decode every WAV flavor to its one second of audio", {}, () => {
    for (const name of t.stereoSeconds) {
      const decoded = decodeAudio(t.getFixtureBuff(name));
      assert.partialDeepStrictEqual(
        decoded,
        {
          sampleRate: info.sampleRate,
          channels: info.channels,
          frameCount: info.frames,
          durationSec: 1,
          skippedPackets: 0
        },
        name
      );
      assert.equal(decoded.samples.length, info.frames * info.channels, name);
    }
    for (const name of ["pcm_alaw.wav", "pcm_mulaw.wav"] as const) {
      assert.partialDeepStrictEqual(
        decodeAudio(t.getFixtureBuff(name)),
        { sampleRate: 8000, channels: 1, frameCount: 8000 },
        name
      );
    }
  });
  it(
    "should trim the encoder's delay and padding unless gapless is off",
    {},
    () => {
      assert.partialDeepStrictEqual(decodeAudio(cbr), {
        frameCount: info.frames,
        gaplessEnabled: true
      });
      assert.partialDeepStrictEqual(decodeAudio(cbr, { gapless: false }), {
        frameCount: 46080,
        gaplessEnabled: false
      });
    }
  );
  it("should agree with FFmpeg's decode of the same MP3", {}, () => {
    for (const name of t.referenced) {
      const { samples } = decodeAudio(t.getFixtureBuff(name));
      const reference = t.reference(name);
      assert.equal(samples.length, reference.length, name);
      const { max, rms } = t.difference(samples, reference);
      assert.ok(max < 3e-5, `${name}: largest error ${max}`);
      assert.ok(rms < 3e-6, `${name}: rms error ${rms}`);
    }
  });
  it("should stop at maxDecodedSamples", {}, () => {
    assert.throws(() => decodeAudio(stereo, { maxDecodedSamples: 10 }), {
      message: "decoded audio exceeds maxDecodedSamples"
    });
  });
  it(
    "the async form should agree, and reject where the sync one throws",
    {},
    async () => {
      assert.deepEqual(await decodeAudioAsync(vbr), decodeAudio(vbr));
      await assert.rejects(
        decodeAudioAsync(stereo, { maxDecodedSamples: 10 }),
        {
          message: "decoded audio exceeds maxDecodedSamples"
        }
      );
    }
  );
});

describe("waveformPeaks", () => {
  it(
    "should reduce each channel to a min, max, peak and RMS per bucket",
    {},
    () => {
      const peaks = waveformPeaks(stereo, { peakCount: 4 });
      assert.partialDeepStrictEqual(peaks, {
        sampleRate: info.sampleRate,
        channelCount: info.channels,
        frameCount: info.frames,
        durationSec: 1,
        peakCount: 4
      });
      assert.deepEqual([...peaks.boundaries], [0, 11025, 22050, 33075, 44100]);
      assert.deepEqual(
        [...peaks.envelope],
        [0.54998779296875, 0.54998779296875, 0.9765625, 0.54998779296875]
      );
      const left = t.at(peaks.channels, 0);
      assert.deepEqual([...left.max], [...peaks.envelope]);
      assert.deepEqual(
        [...left.min],
        [
          -0.54998779296875, -0.54998779296875, -0.54998779296875,
          -0.54998779296875
        ]
      );
      assert.equal(peaks.boundaries instanceof Uint32Array, true);
      assert.equal(left.peak instanceof Float32Array, true);
      assert.equal(left.rms instanceof Float64Array, true);
    }
  );
  it("should take samplesPerPeak in place of peakCount", {}, () => {
    assert.deepEqual(
      waveformPeaks(stereo, { samplesPerPeak: 11025 }),
      waveformPeaks(stereo, { peakCount: 4 })
    );
  });
  it(
    "should default to 1024 peaks, and never to more than there are frames",
    {},
    () => {
      assert.equal(waveformPeaks(stereo).peakCount, 1024);
      const short = waveformPeaks(exactFloat, { peakCount: 1024 });
      assert.equal(short.peakCount, info.floatSamples.length);
      assert.deepEqual(
        [...short.envelope],
        info.floatSamples.map((sample) => Math.abs(sample))
      );
    }
  );
  it("should give the same peaks serial and parallel", {}, () => {
    assert.deepEqual(
      waveformPeaks(cbr, { peakCount: 64, parallel: false }),
      waveformPeaks(cbr, { peakCount: 64, parallel: true })
    );
  });
  it("should reject two resolutions at once, and a fractional one", {}, () => {
    assert.throws(
      () => waveformPeaks(stereo, { peakCount: 4, samplesPerPeak: 10 }),
      {
        code: "InvalidArg",
        message: "pass either peakCount or samplesPerPeak, not both"
      }
    );
    assert.throws(() => waveformPeaks(stereo, { peakCount: 1.5 }), {
      code: "InvalidArg",
      message: "peakCount must be an integer in 1..=1000000"
    });
  });
  it("the async form should agree", {}, async () => {
    assert.deepEqual(
      await waveformPeaksAsync(vbr, { peakCount: 32 }),
      waveformPeaks(vbr, { peakCount: 32 })
    );
  });
});

describe("waveformFromPcm", () => {
  // Two channels, four frames: left 0.5, 1, 0.25, -0.75 and right -0.5, -1, 0.25, 0.5.
  const interleaved = Float32Array.of(0.5, -0.5, 1, -1, 0.25, 0.25, -0.75, 0.5);

  it("should reduce interleaved PCM from the caller exactly", {}, () => {
    const peaks = waveformFromPcm(interleaved, 8000, 2, { peakCount: 2 });
    assert.partialDeepStrictEqual(peaks, {
      sampleRate: 8000,
      channelCount: 2,
      frameCount: 4,
      durationSec: 0.0005,
      peakCount: 2
    });
    assert.deepEqual([...peaks.boundaries], [0, 2, 4]);
    assert.deepEqual([...peaks.envelope], [1, 0.75]);
    const [left, right] = [t.at(peaks.channels, 0), t.at(peaks.channels, 1)];
    assert.deepEqual([...left.min], [0.5, -0.75]);
    assert.deepEqual([...left.max], [1, 0.25]);
    assert.deepEqual([...left.peak], [1, 0.75]);
    assert.deepEqual([...left.rms], [Math.sqrt(0.625), Math.sqrt(0.3125)]);
    assert.deepEqual([...right.min], [-1, 0.25]);
    assert.deepEqual([...right.max], [-0.5, 0.5]);
    assert.deepEqual([...right.peak], [1, 0.5]);
  });
  it("should reject incomplete frames and a sample rate of zero", {}, () => {
    assert.throws(() => waveformFromPcm(Float32Array.of(1, 2, 3), 8000, 2), {
      code: "InvalidArg",
      message: "PCM must contain complete sample frames"
    });
    assert.throws(() => waveformFromPcm(Float32Array.of(1, 2), 0, 2), {
      code: "InvalidArg",
      message: "sampleRate must be an integer in 1..=4294967295"
    });
  });
  it("the async form should agree", {}, async () => {
    assert.deepEqual(
      await waveformFromPcmAsync(interleaved, 8000, 2, { peakCount: 2 }),
      waveformFromPcm(interleaved, 8000, 2, { peakCount: 2 })
    );
  });
});

describe("analyzeAudio", () => {
  it("should return the specs and the waveform of one read", {}, async () => {
    const analysis = analyzeAudio(cbr, { peakCount: 10 }, "cbr.mp3");
    assert.deepEqual(analysis.specs, parseAudio(cbr, "cbr.mp3"));
    assert.deepEqual(analysis.waveform, waveformPeaks(cbr, { peakCount: 10 }));
    assert.deepEqual(
      await analyzeAudioAsync(cbr, { peakCount: 10 }, "cbr.mp3"),
      analysis
    );
  });
});

describe("async input", () => {
  it(
    "should be read from a snapshot, whatever happens to the input afterward",
    {},
    async () => {
      const bytes = Buffer.from(cbr);
      const analysis = analyzeAudioAsync(bytes, { peakCount: 10 });
      bytes.fill(0);
      assert.equal((await analysis).waveform.frameCount, info.frames);

      const samples = Float32Array.of(0.5, -0.25);
      const peaks = waveformFromPcmAsync(samples, 8000, 1, { peakCount: 1 });
      samples.fill(0);
      assert.deepEqual([...t.at((await peaks).channels, 0).peak], [0.5]);
    }
  );
});

describe("AudioDown", () => {
  const audio = new AudioDown(cbr, "cbr.mp3");

  it("should know its kind, size and source", {}, () => {
    assert.equal(audio instanceof AudioDown, true);
    assert.equal(audio.kind, "mp3");
    assert.equal(audio.byteSize, 17230);
    assert.equal(audio.source, "cbr.mp3");
    assert.equal(new AudioDown(stereo).source, null);
  });
  it(
    "should throw unrecognized audio magic for bytes that are not audio",
    {},
    () => {
      assert.throws(() => new AudioDown(notAudio), {
        message: "unrecognized audio magic"
      });
    }
  );
  it("should serve every reader from its one snapshot", {}, async () => {
    assert.deepEqual(audio.metadata(), parseAudio(cbr, "cbr.mp3"));
    assert.deepEqual(audio.decode(), decodeAudio(cbr));
    assert.deepEqual(
      audio.waveform({ peakCount: 16 }),
      waveformPeaks(cbr, { peakCount: 16 })
    );
    assert.deepEqual(
      audio.analyze({ peakCount: 16 }),
      analyzeAudio(cbr, { peakCount: 16 }, "cbr.mp3")
    );
    assert.deepEqual(await audio.metadataAsync(), audio.metadata());
    assert.deepEqual(await audio.decodeAsync(), audio.decode());
    assert.deepEqual(
      await audio.waveformAsync({ peakCount: 16 }),
      audio.waveform({ peakCount: 16 })
    );
    assert.deepEqual(
      await audio.analyzeAsync({ peakCount: 16 }),
      audio.analyze({ peakCount: 16 })
    );
  });
});

describe("AudioPcm", () => {
  it("should reduce one decode at any number of resolutions", {}, async () => {
    const pcm = new AudioDown(cbr).pcm();
    assert.equal(pcm instanceof AudioPcm, true);
    assert.equal(pcm.sampleRate, info.sampleRate);
    assert.equal(pcm.channels, info.channels);
    assert.equal(pcm.frameCount, info.frames);
    assert.equal(pcm.durationSec, 1);
    assert.equal(pcm.gaplessEnabled, true);
    assert.equal(pcm.skippedPackets, 0);
    assert.deepEqual(pcm.decoded(), decodeAudio(cbr));
    assert.deepEqual(pcm.samples(), decodeAudio(cbr).samples);
    for (const peakCount of [3, 100, 1024]) {
      assert.deepEqual(
        pcm.waveform({ peakCount }),
        waveformPeaks(cbr, { peakCount })
      );
    }
    assert.deepEqual(
      await pcm.waveformAsync({ peakCount: 3 }),
      pcm.waveform({ peakCount: 3 })
    );
    const viaAsync = await new AudioDown(cbr).pcmAsync({ gapless: false });
    assert.equal(viaAsync.frameCount, 46080);
  });
  it(
    "should take interleaved PCM from the caller, with defaults of its own",
    {},
    () => {
      const pcm = new AudioPcm(Float32Array.of(0, 1, 0, -1), 8000, 1, {
        peakCount: 2
      });
      assert.equal(pcm.frameCount, 4);
      assert.equal(pcm.durationSec, 0.0005);
      assert.equal(pcm.gaplessEnabled, false);
      assert.deepEqual([...pcm.waveform().envelope], [1, 1]);
      assert.equal(pcm.waveform({ peakCount: 4 }).peakCount, 4);
    }
  );
});

describe("AudioService", () => {
  const service = new AudioService({ peakCount: 8 });

  it("should mirror the standalone functions", {}, async () => {
    assert.equal(service.sniffAudio(stereo), "wav");
    assert.deepEqual(
      service.parseAudio(cbr, "a.mp3"),
      parseAudio(cbr, "a.mp3")
    );
    assert.deepEqual(service.parseMp3(cbr), parseMp3(cbr));
    assert.deepEqual(service.parseWav(stereo), parseWav(stereo));
    assert.deepEqual(service.decodeAudio(vbr), decodeAudio(vbr));
    assert.deepEqual(await service.parseAudioAsync(cbr), parseAudio(cbr));
    assert.deepEqual(await service.decodeAudioAsync(vbr), decodeAudio(vbr));
  });
  it(
    "should apply its defaults underneath the options of each call",
    {},
    async () => {
      assert.deepEqual(service.defaults, { peakCount: 8 });
      assert.deepEqual(new AudioService().defaults, {});
      assert.equal(service.waveformPeaks(stereo).peakCount, 8);
      assert.equal(
        service.waveformPeaks(stereo, { peakCount: 16 }).peakCount,
        16
      );
      // A resolution given to the call replaces the default one, in either form.
      assert.equal(
        service.waveformPeaks(stereo, { samplesPerPeak: 22050 }).peakCount,
        2
      );
      assert.equal((await service.waveformPeaksAsync(stereo)).peakCount, 8);
      assert.equal(service.analyzeAudio(stereo).waveform.peakCount, 8);
      assert.equal(
        service.waveformFromPcm(decodeAudio(stereo).samples, 44100, 2)
          .peakCount,
        8
      );
    }
  );
  it("should hand its defaults to the AudioDown it opens", {}, () => {
    const opened = service.open(stereo, "stereo.wav");
    assert.equal(opened instanceof AudioDown, true);
    assert.equal(opened.source, "stereo.wav");
    assert.equal(opened.waveform().peakCount, 8);
    assert.equal(opened.waveform({ peakCount: 2 }).peakCount, 2);
  });
});
