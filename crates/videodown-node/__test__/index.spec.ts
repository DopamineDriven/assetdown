import assert from "node:assert/strict";
import { describe, it } from "node:test";

import info from "@/__test__/fixtures/fixture-info.json" with { type: "json" };
import { WorkupService } from "@/__test__/workup.ts";
import {
  VideoDown,
  VideoService,
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
  posterAsync,
  sniffVideo
} from "@/index.js";

/** What ffprobe says about one fixture, as recorded in fixture-info.json. */
interface FfprobeFacts {
  size: number;
  duration: number | null;
  streams: number;
  attachedPics: number;
  title: string | null;
  video: {
    codec: string;
    width: number;
    height: number;
    rotate: string | null;
  };
  audio: { codec: string; channels: number; sampleRate: number }[];
  keyframes: number[];
  videoPackets: number;
}

// WorkupService extends Fs
class VideodownTest extends WorkupService {
  /** Every fixture ffprobe was run on, by file name, with what it reported. */
  public get probed() {
    return Object.entries<FfprobeFacts>(info);
  }

  /** Exports from a video model, with the size and counts each was made at. */
  public get exports() {
    return [
      ["text-to-video-16x9-1080p.mp4", 1920, 1080, "1080p", "landscape", 240],
      [
        "video-gemini-omni-1.1-flash-1080p-16x9-006-delta_data.mp4",
        1920,
        1080,
        "1080p",
        "landscape",
        240
      ],
      [
        "video-gemini-omni-1.1-flash-4k-16x9-009-delta_data.mp4",
        3840,
        2160,
        "4K",
        "landscape",
        240
      ],
      [
        "video-gemini-omni-1.1-flash-720p-16x9-008-delta_data.mp4",
        1280,
        720,
        "720p",
        "landscape",
        240
      ],
      [
        "video-gemini-omni-1.1-flash-720p-9x16-008-delta_data.mp4",
        720,
        1280,
        "720p",
        "portrait",
        720
      ]
    ] as const;
  }

  /** `<name>.reference.f32`: FFmpeg's decode of the audio track, as little-endian 32-bit floats. */
  public reference(name: "h264-aac-faststart.mp4" | "vp9-vorbis.webm") {
    const bytes = this.getFixtureBuff(`${name}.reference.f32`);
    return Float32Array.from({ length: bytes.length / 4 }, (_, i) =>
      bytes.readFloatLE(i * 4)
    );
  }

  /**
   * The Vorbis fixture with its codec id changed, at the same length, to TrueHD: a codec the
   * reader knows and nothing here decodes. No fixture carries such a track.
   */
  public get undecodable() {
    const bytes = Buffer.from(this.getFixtureBuff("vp9-vorbis.webm"));
    bytes.write("A_TRUEHD", bytes.indexOf("A_VORBIS"), "latin1");
    return bytes;
  }

  /** The largest difference between two runs of samples of the same length. */
  public largestError(samples: Float32Array, reference: Float32Array) {
    let max = 0;
    for (const [i, sample] of samples.entries()) {
      max = Math.max(max, Math.abs(sample - (reference[i] ?? Number.NaN)));
    }
    return max;
  }
}

const t = new VideodownTest();
const faststart = t.getFixtureBuff("h264-aac-faststart.mp4");
const mdatFirst = t.getFixtureBuff("h264-aac-mdat-first.mp4");
const coverMp4 = t.getFixtureBuff("h264-aac-cover.mp4");
const coverMkv = t.getFixtureBuff("h264-aac-cover.mkv");
const silent = t.getFixtureBuff("silent.mp4");
const vorbis = t.getFixtureBuff("vp9-vorbis.webm");
const opus = t.getFixtureBuff("vp9-opus.webm");
const live = t.getFixtureBuff("vp9-opus-live.webm");
const undecodable = t.undecodable;
const cover = t.getFixtureBuff("cover.jpg");
const notVideo = Buffer.from("not a video at all");
/** The start of a file whose index has not arrived yet. */
const prefix = faststart.subarray(0, 4096);
const prefixMessage =
  "buffer is a prefix of the file; fetch the whole object before decoding its audio";

describe("sniffVideo", () => {
  it("should name the container from the first bytes", {}, () => {
    assert.equal(sniffVideo(faststart), "mp4");
    assert.equal(sniffVideo(t.getFixtureBuff("h264-aac.mov")), "mov");
    assert.equal(sniffVideo(t.getFixtureBuff("h264-aac.mkv")), "mkv");
    assert.equal(sniffVideo(vorbis), "webm");
  });
  it("should return null for bytes that are not a video", {}, () => {
    assert.equal(sniffVideo(notVideo), null);
    assert.equal(sniffVideo(cover), null);
    assert.equal(sniffVideo(Buffer.alloc(0)), null);
  });
});

describe("parseVideo", () => {
  it("should agree with ffprobe on every fixture it was run on", {}, () => {
    for (const [name, facts] of t.probed) {
      const bytes = t.getFixtureBuff(name);
      const specs = parseVideo(bytes);
      assert.partialDeepStrictEqual(
        specs,
        {
          byteSize: facts.size,
          width: facts.video.width,
          height: facts.video.height,
          videoCodec: facts.video.codec,
          // ffprobe reports the rotate tag as text, and nothing at all when upright.
          rotation:
            facts.video.rotate === null
              ? 0
              : Number.parseInt(facts.video.rotate, 10),
          frameCount: facts.videoPackets,
          trackCount: facts.streams - facts.attachedPics,
          hasAudio: facts.audio.length > 0,
          title: facts.title
        },
        name
      );
      assert.deepEqual(
        specs.audioTracks.map((a) => [a.codec, a.channels, a.sampleRate]),
        facts.audio.map((a) => [a.codec, a.channels, a.sampleRate]),
        name
      );
      assert.equal(specs.coverArt !== null, facts.attachedPics > 0, name);
      if (facts.duration !== null) {
        assert.ok(
          Math.abs((specs.durationSec ?? 0) - facts.duration) < 1e-9,
          `${name}: ${specs.durationSec} against ${facts.duration}`
        );
      }
      assert.deepEqual(
        [...keyframes(bytes).timesMs],
        facts.keyframes.map((seconds) => Math.round(seconds * 1000)),
        name
      );
    }
  });
  it("should describe an MP4 in full", {}, () => {
    assert.partialDeepStrictEqual(parseVideo(faststart), {
      type: "VIDEO",
      kind: "mp4",
      format: "mp4",
      ext: "mp4",
      mime: "video/mp4",
      mimeType: "video/mp4",
      contentType: "video/mp4",
      container: "isobmff",
      byteSize: 191556,
      complete: true,
      truncated: false,
      fragmented: false,
      boxes: ["ftyp", "moov", "free", "mdat"],
      moovBeforeMdat: true,
      brands: {
        major: "isom",
        minorVersion: 512,
        compatible: ["isom", "iso2", "avc1", "mp41"]
      },
      creationTime: "2026-10-14T12:00:00Z",
      writingApp: "Lavf58.76.100",
      coverArt: null,
      timescale: 1000,
      durationSec: 4,
      durationMs: 4000,
      bitrateKbps: 383,
      trackCount: 2,
      videoTrackCount: 1,
      audioTrackCount: 1,
      displayWidth: 320,
      displayHeight: 180,
      aspectRatioLabel: "16:9",
      orientation: "landscape",
      resolution: "180p",
      frameRate: 30,
      constantFrameRate: true,
      videoCodecTag: "avc1",
      audioCodec: "aac",
      audioCodecTag: "mp4a",
      audioChannels: 2,
      audioSampleRate: 48000,
      keyframeCount: 4,
      keyframeSource: "stss",
      video: {
        trackId: 1,
        codec: "h264",
        codecTag: "avc1",
        timescale: 15360,
        sampleCount: 120,
        bitrate: 278236
      }
    });
  });
  it("should give a QuickTime file its own kind, brand and mime", {}, () => {
    assert.partialDeepStrictEqual(
      parseVideo(t.getFixtureBuff("h264-aac.mov")),
      {
        kind: "mov",
        ext: "mov",
        mime: "video/quicktime",
        container: "isobmff",
        brands: { major: "qt  ", compatible: ["qt  "] },
        boxes: ["ftyp", "moov", "wide", "mdat"]
      }
    );
  });
  it("should say whether the index comes before the media", {}, () => {
    assert.equal(parseVideo(faststart).moovBeforeMdat, true);
    assert.partialDeepStrictEqual(parseVideo(mdatFirst), {
      moovBeforeMdat: false,
      boxes: ["ftyp", "free", "mdat", "moov"]
    });
  });
  it("should turn the display size with the rotation", {}, () => {
    for (const [name, rotation] of [
      ["rotated-90.mp4", 90],
      ["rotated-270.mp4", 270]
    ] as const) {
      assert.partialDeepStrictEqual(
        parseVideo(t.getFixtureBuff(name)),
        {
          width: 320,
          height: 180,
          rotation,
          displayWidth: 180,
          displayHeight: 320,
          orientation: "portrait",
          aspectRatioLabel: "9:16"
        },
        name
      );
    }
  });
  it(
    "should list every audio track and summarize the first, or report none",
    {},
    () => {
      const two = parseVideo(t.getFixtureBuff("two-audio.mp4"));
      assert.partialDeepStrictEqual(two, {
        audioTrackCount: 2,
        audioChannels: 2,
        audioSampleRate: 48000,
        audioTracks: [
          { trackId: 2, channels: 2, sampleRate: 48000, bitrate: 93254 },
          { trackId: 3, channels: 1, sampleRate: 44100, bitrate: 47950 }
        ]
      });
      assert.partialDeepStrictEqual(parseVideo(silent), {
        hasAudio: false,
        audioTrackCount: 0,
        audioCodec: null,
        audioChannels: null,
        audioTracks: []
      });
    }
  );
  it("should read Matroska and WebM", {}, () => {
    assert.partialDeepStrictEqual(
      parseVideo(t.getFixtureBuff("h264-aac.mkv")),
      {
        kind: "mkv",
        mime: "video/x-matroska",
        container: "matroska",
        videoCodecTag: "V_MPEG4/ISO/AVC",
        audioCodecTag: "A_AAC",
        keyframeSource: "cues",
        moovBeforeMdat: null,
        brands: null,
        boxes: []
      }
    );
    assert.partialDeepStrictEqual(parseVideo(vorbis), {
      kind: "webm",
      mime: "video/webm",
      container: "matroska",
      videoCodec: "vp9",
      audioCodec: "vorbis",
      writingApp: "Lavf58.76.100"
    });
  });
  it("should read exports at every size they come in", {}, () => {
    for (const [
      name,
      width,
      height,
      resolution,
      orientation,
      frames
    ] of t.exports) {
      assert.partialDeepStrictEqual(
        parseVideo(t.getFixtureBuff(name)),
        {
          kind: "mp4",
          complete: true,
          width,
          height,
          resolution,
          orientation,
          frameRate: 24,
          frameCount: frames,
          videoCodec: "h264",
          audioCodec: "aac",
          writingApp: "Google"
        },
        name
      );
    }
  });
  it("should flag the start of a file as truncated", {}, () => {
    assert.partialDeepStrictEqual(parseVideo(prefix), {
      kind: "mp4",
      byteSize: 4096,
      complete: false,
      truncated: true,
      width: null,
      frameCount: null,
      keyframeCount: 0
    });
  });
  it(
    "should carry the source it was given and leave it out otherwise",
    {},
    () => {
      assert.equal(
        parseVideo(faststart, "s3://bucket/a.mp4").source,
        "s3://bucket/a.mp4"
      );
      assert.equal("source" in parseVideo(faststart), false);
    }
  );
  it("should throw unrecognized video magic for anything else", {}, () => {
    for (const bytes of [notVideo, cover, Buffer.alloc(0)]) {
      assert.throws(() => parseVideo(bytes), {
        message: "unrecognized video magic"
      });
    }
  });
  it("the async form should agree", {}, async () => {
    assert.deepEqual(
      await parseVideoAsync(faststart, "a.mp4"),
      parseVideo(faststart, "a.mp4")
    );
    assert.deepEqual(await parseVideoAsync(vorbis), parseVideo(vorbis));
    await assert.rejects(parseVideoAsync(notVideo), {
      message: "unrecognized video magic"
    });
  });
});

describe("keyframes", () => {
  it("should index an MP4 from its sync-sample table", {}, () => {
    const index = keyframes(faststart);
    assert.partialDeepStrictEqual(index, {
      source: "stss",
      complete: true,
      count: 4,
      sampleCount: 120,
      durationSec: 4,
      meanIntervalSec: 1
    });
    assert.deepEqual(index.timesMs, Uint32Array.of(0, 1000, 2000, 3000));
    assert.deepEqual(index.timesSec, Float64Array.of(0, 1, 2, 3));
  });
  it("should index a live WebM, which has no cues, from its blocks", {}, () => {
    const index = keyframes(live);
    assert.partialDeepStrictEqual(index, {
      source: "blocks",
      complete: true,
      count: 4
    });
    assert.deepEqual([...index.timesMs], [7, 1007, 2007, 3007]);
  });
  it("should mark the index of a WebM prefix as partial", {}, () => {
    assert.partialDeepStrictEqual(keyframes(vorbis.subarray(0, 20000)), {
      source: "blocks",
      complete: false,
      count: 1
    });
  });
  it("the async form should agree", {}, async () => {
    assert.deepEqual(await keyframesAsync(faststart), keyframes(faststart));
    assert.deepEqual(await keyframesAsync(live), keyframes(live));
  });
});

describe("coverArt", () => {
  it(
    "should return the picture from an MP4's covr atom and from an MKV's attachment",
    {},
    () => {
      for (const bytes of [coverMp4, coverMkv]) {
        const art = coverArt(bytes);
        assert.ok(art);
        assert.partialDeepStrictEqual(art, {
          mime: "image/jpeg",
          byteLength: cover.length,
          width: 320,
          height: 180
        });
        assert.equal(art.data instanceof Uint8Array, true);
        assert.equal(Buffer.compare(art.data, cover), 0);
      }
      assert.deepEqual(parseVideo(coverMp4).coverArt, {
        mime: "image/jpeg",
        byteLength: cover.length,
        width: 320,
        height: 180
      });
    }
  );
  it("should return null when there is none", {}, async () => {
    assert.equal(coverArt(faststart), null);
    assert.equal(await coverArtAsync(faststart), null);
  });
  it("the async form should agree", {}, async () => {
    assert.deepEqual(await coverArtAsync(coverMkv), coverArt(coverMkv));
  });
});

describe("poster", () => {
  /** The first bytes of an image, as text. */
  const head = (data: Uint8Array, from: number, to: number) =>
    Buffer.from(data).toString("latin1", from, to);

  it(
    "should return the first keyframe as a JPEG the size the video is shown at",
    {},
    () => {
      const made = poster(faststart);
      assert.ok(made);
      assert.partialDeepStrictEqual(made, {
        mime: "image/jpeg",
        width: 320,
        height: 180,
        timeSec: 0,
        byteLength: made.data.length
      });
      assert.equal(made.data instanceof Uint8Array, true);
      assert.deepEqual([...made.data.subarray(0, 3)], [0xff, 0xd8, 0xff]);
    }
  );
  it("should write PNG and lossless WebP when asked", {}, () => {
    const png = poster(faststart, { format: "png" });
    const webp = poster(faststart, { format: "webp" });
    assert.ok(png && webp);
    assert.equal(png.mime, "image/png");
    assert.equal(head(png.data, 1, 4), "PNG");
    assert.equal(webp.mime, "image/webp");
    assert.equal(head(webp.data, 0, 4) + head(webp.data, 8, 12), "RIFFWEBP");
    // A lower JPEG quality is a smaller file.
    const fine = poster(faststart, { quality: 90 });
    const rough = poster(faststart, { quality: 30 });
    assert.ok(fine && rough && rough.byteLength < fine.byteLength);
  });
  it("should make the same poster whatever the container", {}, () => {
    // The MOV and MKV fixtures carry the stream the MP4 does.
    const expected = poster(faststart, { format: "png" });
    assert.ok(expected);
    for (const name of [
      "h264-aac.mov",
      "h264-aac.mkv",
      "h264-aac-mdat-first.mp4"
    ] as const) {
      assert.deepEqual(
        poster(t.getFixtureBuff(name), { format: "png" }),
        expected,
        name
      );
    }
  });
  it("should give a rotated video an upright poster", {}, () => {
    for (const name of ["rotated-90.mp4", "rotated-270.mp4"] as const) {
      assert.partialDeepStrictEqual(
        poster(t.getFixtureBuff(name)),
        { width: 180, height: 320 },
        name
      );
    }
  });
  it("should take the keyframe at or before timeSec", {}, () => {
    // Keyframes at 0, 1, 2 and 3 seconds.
    const first = poster(faststart, { format: "png" });
    const later = poster(faststart, { format: "png", timeSec: 2.5 });
    assert.ok(first && later);
    assert.equal(later.timeSec, 2);
    assert.notDeepEqual(later.data, first.data);
    assert.equal(poster(faststart, { timeSec: 0.4 })?.timeSec, 0);
    assert.equal(poster(faststart, { timeSec: 99 })?.timeSec, 3);
  });
  it("should decode HEVC, and exports at every size they come in", {}, () => {
    assert.partialDeepStrictEqual(poster(t.getFixtureBuff("hevc-aac.mp4")), {
      mime: "image/jpeg",
      width: 320,
      height: 180
    });
    for (const [name, width, height] of t.exports) {
      assert.partialDeepStrictEqual(
        poster(t.getFixtureBuff(name)),
        { mime: "image/jpeg", width, height, timeSec: 0 },
        name
      );
    }
  });
  it("should return null for a codec it has no decoder for", {}, async () => {
    assert.equal(poster(t.getFixtureBuff("av1-aac.mp4")), null);
    assert.equal(poster(vorbis), null);
    assert.equal(await posterAsync(vorbis), null);
  });
  it(
    "should work from a prefix that holds the first keyframe, and throw for a shorter one",
    {},
    () => {
      assert.deepEqual(poster(faststart.subarray(0, 12000)), poster(faststart));
      assert.throws(() => poster(prefix), {
        message: /^buffer ends before the keyframe/
      });
      assert.throws(() => poster(notVideo), {
        message: "unrecognized video magic"
      });
    }
  );
  it("should reject a time or a quality out of range", {}, () => {
    assert.throws(() => poster(faststart, { timeSec: -1 }), {
      code: "InvalidArg",
      message: "timeSec must be a finite number of seconds, zero or more"
    });
    for (const quality of [0, 1.5, 101]) {
      assert.throws(() => poster(faststart, { quality }), {
        code: "InvalidArg",
        message: "quality must be an integer in 1..=100"
      });
    }
  });
  it("the async form should agree", {}, async () => {
    assert.deepEqual(
      await posterAsync(faststart, { format: "png", timeSec: 1 }),
      poster(faststart, { format: "png", timeSec: 1 })
    );
  });
});

describe("decodeAudio", () => {
  it("should decode the audio track to interleaved floats", {}, () => {
    const decoded = decodeAudio(faststart);
    assert.ok(decoded);
    assert.partialDeepStrictEqual(decoded, {
      sampleRate: 48000,
      channels: 2,
      frameCount: 193536,
      gaplessEnabled: true,
      skippedPackets: 0
    });
    assert.equal(decoded.samples instanceof Float32Array, true);
    assert.equal(decoded.samples.length, 193536 * 2);
  });
  it("should return null for a file with no audio track", {}, async () => {
    assert.equal(decodeAudio(silent), null);
    assert.equal(await decodeAudioAsync(silent), null);
  });
  it("should decode Vorbis to FFmpeg's samples", {}, () => {
    const decoded = decodeAudio(vorbis);
    assert.ok(decoded);
    const reference = t.reference("vp9-vorbis.webm");
    assert.equal(decoded.samples.length, reference.length);
    assert.ok(t.largestError(decoded.samples, reference) < 1e-5);
  });
  it(
    "should decode AAC to FFmpeg's samples, which start one priming frame later",
    {},
    () => {
      const decoded = decodeAudio(faststart);
      assert.ok(decoded);
      const reference = t.reference("h264-aac-faststart.mp4");
      // An AAC frame is 1024 samples per channel; FFmpeg drops the first, this decode keeps it.
      const aligned = decoded.samples.subarray(1024 * decoded.channels);
      assert.equal(aligned.length, reference.length);
      assert.ok(t.largestError(aligned, reference) < 1e-5);
    }
  );
  it(
    "should decode the audio beside a video track too large for the audio reader",
    {},
    () => {
      // x265 writes a video configuration larger than Symphonia accepts. The audio beside it is
      // the track the H.264 fixture carries.
      const hevc = t.getFixtureBuff("hevc-aac.mp4");
      assert.deepEqual(decodeAudio(hevc), decodeAudio(faststart));
      assert.equal(audioPeaks(hevc, { peakCount: 4 })?.frameCount, 193536);
      assert.equal(analyzeVideo(hevc).waveformStatus, "ok");
    }
  );
  it(
    "should decode Opus, with the lead-in and padding of the stream kept",
    {},
    () => {
      const decoded = decodeAudio(opus);
      assert.ok(decoded);
      // 192,000 frames of source, the 312 Opus asks a player to drop, and 648 of final padding.
      assert.partialDeepStrictEqual(decoded, {
        sampleRate: 48000,
        channels: 2,
        frameCount: 192960,
        skippedPackets: 0
      });
      // The live file carries the same packets, without cues or a duration.
      assert.deepEqual(decodeAudio(live), decoded);
      assert.equal(audioPeaks(opus, { peakCount: 4 })?.frameCount, 192960);
      assert.equal(analyzeVideo(opus).waveformStatus, "ok");
    }
  );
  it(
    "should throw for a codec it cannot decode, for a prefix, and past maxDecodedSamples",
    {},
    () => {
      assert.throws(() => decodeAudio(undecodable), {
        message: /^unsupported audio codec/
      });
      assert.throws(() => decodeAudio(prefix), { message: prefixMessage });
      assert.throws(() => decodeAudio(faststart, { maxDecodedSamples: 10 }), {
        message: "decoded audio exceeds maxDecodedSamples"
      });
    }
  );
  it(
    "the async form should agree, and reject where the sync one throws",
    {},
    async () => {
      assert.deepEqual(await decodeAudioAsync(vorbis), decodeAudio(vorbis));
      await assert.rejects(decodeAudioAsync(undecodable), {
        message: /^unsupported audio codec/
      });
    }
  );
});

describe("audioPeaks", () => {
  it("should reduce the audio track to peaks per bucket", {}, () => {
    const peaks = audioPeaks(faststart, { peakCount: 4 });
    assert.ok(peaks);
    assert.partialDeepStrictEqual(peaks, {
      sampleRate: 48000,
      channelCount: 2,
      frameCount: 193536,
      peakCount: 4,
      gaplessEnabled: true
    });
    assert.deepEqual([...peaks.boundaries], [0, 48384, 96768, 145152, 193536]);
    assert.equal(peaks.channels.length, 2);
    assert.equal(peaks.envelope.length, 4);
    // The fixtures carry a sine at 0.55 of full scale.
    assert.ok(Math.abs(Math.max(...peaks.envelope) - 0.55) < 0.03);
    for (const channel of peaks.channels) {
      assert.equal(channel.peak instanceof Float32Array, true);
      assert.equal(channel.rms instanceof Float64Array, true);
    }
  });
  it("should default to 1024 peaks and take samplesPerPeak instead", {}, () => {
    assert.equal(audioPeaks(faststart)?.peakCount, 1024);
    assert.deepEqual(
      audioPeaks(faststart, { samplesPerPeak: 48384 }),
      audioPeaks(faststart, { peakCount: 4 })
    );
  });
  it("should give the same peaks serial and parallel", {}, () => {
    assert.deepEqual(
      audioPeaks(vorbis, { peakCount: 64, parallel: false }),
      audioPeaks(vorbis, { peakCount: 64, parallel: true })
    );
  });
  it(
    "should return null without an audio track, and throw for an undecodable codec and for a prefix",
    {},
    () => {
      assert.equal(audioPeaks(silent), null);
      assert.throws(() => audioPeaks(undecodable), {
        message: /^unsupported audio codec/
      });
      assert.throws(() => audioPeaks(prefix), { message: prefixMessage });
    }
  );
  it("should reject two resolutions at once, and a fractional one", {}, () => {
    assert.throws(
      () => audioPeaks(faststart, { peakCount: 4, samplesPerPeak: 10 }),
      {
        code: "InvalidArg",
        message: "pass either peakCount or samplesPerPeak, not both"
      }
    );
    assert.throws(() => audioPeaks(faststart, { peakCount: 1.5 }), {
      code: "InvalidArg",
      message: "peakCount must be an integer in 1..=1000000"
    });
  });
  it("the async form should agree", {}, async () => {
    assert.deepEqual(
      await audioPeaksAsync(faststart, { peakCount: 32 }),
      audioPeaks(faststart, { peakCount: 32 })
    );
    assert.equal(await audioPeaksAsync(silent), null);
  });
});

describe("analyzeVideo", () => {
  it(
    "should return the specs, keyframes, peaks and cover of one read",
    {},
    async () => {
      const analysis = analyzeVideo(coverMp4, { peakCount: 16 }, "cover.mp4");
      assert.deepEqual(analysis.specs, parseVideo(coverMp4, "cover.mp4"));
      assert.deepEqual(analysis.keyframes, keyframes(coverMp4));
      assert.deepEqual(
        analysis.waveform,
        audioPeaks(coverMp4, { peakCount: 16 })
      );
      assert.deepEqual(analysis.coverArt, coverArt(coverMp4));
      assert.equal(analysis.waveformStatus, "ok");
      assert.equal(analysis.waveformReason, null);
      assert.deepEqual(
        await analyzeVideoAsync(coverMp4, { peakCount: 16 }, "cover.mp4"),
        analysis
      );
    }
  );
  it(
    "should say why there are no peaks where the readers return null or throw",
    {},
    () => {
      assert.partialDeepStrictEqual(analyzeVideo(silent), {
        waveform: null,
        waveformStatus: "no-audio",
        waveformReason: null
      });
      assert.partialDeepStrictEqual(analyzeVideo(prefix), {
        waveform: null,
        waveformStatus: "truncated",
        waveformReason: null
      });
      const unsupported = analyzeVideo(undecodable);
      assert.equal(unsupported.waveform, null);
      assert.equal(unsupported.waveformStatus, "unsupported-codec");
      assert.match(
        unsupported.waveformReason ?? "",
        /^unsupported audio codec/
      );
      assert.equal(unsupported.specs.audioCodec, "truehd");
    }
  );
});

describe("async input", () => {
  it(
    "should be read from a snapshot, whatever happens to the input afterward",
    {},
    async () => {
      const bytes = Buffer.from(faststart);
      const analysis = analyzeVideoAsync(bytes, { peakCount: 8 });
      bytes.fill(0);
      const { specs, waveform } = await analysis;
      assert.equal(specs.width, 320);
      assert.equal(waveform?.frameCount, 193536);
    }
  );
});

describe("VideoDown", () => {
  const video = new VideoDown(coverMp4, "cover.mp4");

  it("should know its kind, size and source", {}, () => {
    assert.equal(video instanceof VideoDown, true);
    assert.equal(video.kind, "mp4");
    assert.equal(video.byteSize, coverMp4.length);
    assert.equal(video.source, "cover.mp4");
    assert.equal(new VideoDown(vorbis).kind, "webm");
    assert.equal(new VideoDown(vorbis).source, null);
  });
  it(
    "should throw unrecognized video magic for bytes that are not a video",
    {},
    () => {
      assert.throws(() => new VideoDown(notVideo), {
        message: "unrecognized video magic"
      });
    }
  );
  it("should serve every reader from its one snapshot", {}, async () => {
    assert.deepEqual(video.metadata(), parseVideo(coverMp4, "cover.mp4"));
    assert.deepEqual(video.keyframes(), keyframes(coverMp4));
    assert.deepEqual(video.coverArt(), coverArt(coverMp4));
    assert.deepEqual(
      video.poster({ format: "png" }),
      poster(coverMp4, { format: "png" })
    );
    assert.deepEqual(video.decodeAudio(), decodeAudio(coverMp4));
    assert.deepEqual(
      video.audioPeaks({ peakCount: 16 }),
      audioPeaks(coverMp4, { peakCount: 16 })
    );
    assert.deepEqual(
      video.analyze({ peakCount: 16 }),
      analyzeVideo(coverMp4, { peakCount: 16 }, "cover.mp4")
    );
    assert.deepEqual(await video.metadataAsync(), video.metadata());
    assert.deepEqual(await video.keyframesAsync(), video.keyframes());
    assert.deepEqual(await video.coverArtAsync(), video.coverArt());
    assert.deepEqual(await video.posterAsync(), video.poster());
    assert.deepEqual(await video.decodeAudioAsync(), video.decodeAudio());
    assert.deepEqual(
      await video.audioPeaksAsync({ peakCount: 16 }),
      video.audioPeaks({ peakCount: 16 })
    );
    assert.deepEqual(
      await video.analyzeAsync({ peakCount: 16 }),
      video.analyze({ peakCount: 16 })
    );
  });
});

describe("VideoService", () => {
  const service = new VideoService({ peakCount: 8 });

  it("should mirror the standalone functions", {}, async () => {
    assert.equal(service.sniffVideo(vorbis), "webm");
    assert.deepEqual(
      service.parseVideo(faststart, "a.mp4"),
      parseVideo(faststart, "a.mp4")
    );
    assert.deepEqual(service.keyframes(live), keyframes(live));
    assert.deepEqual(service.coverArt(coverMkv), coverArt(coverMkv));
    assert.deepEqual(service.poster(faststart), poster(faststart));
    assert.deepEqual(await service.posterAsync(faststart), poster(faststart));
    assert.deepEqual(service.decodeAudio(vorbis), decodeAudio(vorbis));
    assert.deepEqual(await service.parseVideoAsync(vorbis), parseVideo(vorbis));
    assert.deepEqual(await service.keyframesAsync(live), keyframes(live));
    assert.equal(await service.decodeAudioAsync(silent), null);
  });
  it(
    "should apply its defaults underneath the options of each call",
    {},
    async () => {
      assert.deepEqual(service.defaults, { peakCount: 8 });
      assert.deepEqual(new VideoService().defaults, {});
      assert.equal(service.audioPeaks(faststart)?.peakCount, 8);
      assert.equal(
        service.audioPeaks(faststart, { peakCount: 16 })?.peakCount,
        16
      );
      // A resolution given to the call replaces the default one, in either form.
      assert.equal(
        service.audioPeaks(faststart, { samplesPerPeak: 96768 })?.peakCount,
        2
      );
      assert.equal((await service.audioPeaksAsync(faststart))?.peakCount, 8);
      assert.equal(service.analyzeVideo(faststart).waveform?.peakCount, 8);
    }
  );
  it("should hand its defaults to the VideoDown it opens", {}, () => {
    const opened = service.open(faststart, "a.mp4");
    assert.equal(opened instanceof VideoDown, true);
    assert.equal(opened.source, "a.mp4");
    assert.equal(opened.audioPeaks()?.peakCount, 8);
    assert.equal(opened.audioPeaks({ peakCount: 2 })?.peakCount, 2);
  });
});
