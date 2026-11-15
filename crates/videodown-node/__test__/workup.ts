import { resolve } from "node:path";

import { Fs } from "@d0paminedriven/fs";
// const fs = new Fs(process.cwd());

// const dir = fs
//   .readDir("__test__/fixtures", { recursive: true })
//   .filter((v) => v.endsWith(".pdf") && v.lastIndexOf(".") !== -1)
//   .map((o) => `__test__/fixtures/${o}`);

// const getKeys = dir
//   .map((o) => o.replace("__test__/fixtures/", ""))
//   .map((t) => t.slice(0, t.lastIndexOf(".")));
// console.log(getKeys)
/**
 *
 * For `Fs` derived methods
 *
 * to access any path, use the *Tmp styled methods
 *
 * to leverage the cwd, use the standard methods
 *
 * eg
 *
 * `readTmp` &rarr; Absolute, uses `resolve(...paths)` where `typeof paths = string[]`
 *
 * `fileToBuffer`&rarr; Standard, uses `relative(process.cwd(), path_arg_is_what_falls_in_here)`
 */
export class WorkupService extends Fs {
  constructor() {
    super(process.cwd());
  }
  public safeErrMsg(err: unknown) {
    if (err instanceof Error) {
      return err.message;
    } else if (typeof err === "object" && err != null) {
      return JSON.stringify(err, Object.getOwnPropertyNames(err), 2);
    } else if (typeof err === "string") {
      return err;
    } else if (typeof err === "number") {
      return err.toPrecision(5);
    } else if (typeof err === "boolean") {
      return `${err}`;
    } else return String(err);
  }

  public readAbsPath(paths: string[]) {
    return resolve(...paths);
  }

  public get workspaceRoot() {
    return this.readAbsPath([process.cwd(), "../.."]);
  }
  public get fixturePaths() {
    return {
      "av1-aac.mp4": "__test__/fixtures/av1-aac.mp4",
      "cover.jpg": "__test__/fixtures/cover.jpg",
      "fixture-info.json": "__test__/fixtures/fixture-info.json",
      "h264-aac-cover.mkv": "__test__/fixtures/h264-aac-cover.mkv",
      "h264-aac-cover.mp4": "__test__/fixtures/h264-aac-cover.mp4",
      "h264-aac-faststart.mp4": "__test__/fixtures/h264-aac-faststart.mp4",
      "h264-aac-faststart.mp4.reference.f32":
        "__test__/fixtures/h264-aac-faststart.mp4.reference.f32",
      "h264-aac-mdat-first.mp4": "__test__/fixtures/h264-aac-mdat-first.mp4",
      "h264-aac.mkv": "__test__/fixtures/h264-aac.mkv",
      "h264-aac.mov": "__test__/fixtures/h264-aac.mov",
      "hevc-aac.mp4": "__test__/fixtures/hevc-aac.mp4",
      "rotated-270.mp4": "__test__/fixtures/rotated-270.mp4",
      "rotated-90.mp4": "__test__/fixtures/rotated-90.mp4",
      "silent.mp4": "__test__/fixtures/silent.mp4",
      "text-to-video-16x9-1080p.mp4":
        "__test__/fixtures/text-to-video-16x9-1080p.mp4",
      "two-audio.mp4": "__test__/fixtures/two-audio.mp4",
      "video-gemini-omni-1.1-flash-1080p-16x9-006-delta_data.mp4":
        "__test__/fixtures/video-gemini-omni-1.1-flash-1080p-16x9-006-delta_data.mp4",
      "video-gemini-omni-1.1-flash-1080p-16x9-009-delta_data.mp4":
        "__test__/fixtures/video-gemini-omni-1.1-flash-1080p-16x9-009-delta_data.mp4",
      "video-gemini-omni-1.1-flash-4k-16x9-009-delta_data.mp4":
        "__test__/fixtures/video-gemini-omni-1.1-flash-4k-16x9-009-delta_data.mp4",
      "video-gemini-omni-1.1-flash-720p-16x9-008-delta_data.mp4":
        "__test__/fixtures/video-gemini-omni-1.1-flash-720p-16x9-008-delta_data.mp4",
      "video-gemini-omni-1.1-flash-720p-9x16-008-delta_data.mp4":
        "__test__/fixtures/video-gemini-omni-1.1-flash-720p-9x16-008-delta_data.mp4",
      "video-gemini-omni-1.1-flash-720p-9x16-009-delta_data.mp4":
        "__test__/fixtures/video-gemini-omni-1.1-flash-720p-9x16-009-delta_data.mp4",
      "vp9-opus-live.webm": "__test__/fixtures/vp9-opus-live.webm",
      "vp9-opus.webm": "__test__/fixtures/vp9-opus.webm",
      "vp9-vorbis.webm": "__test__/fixtures/vp9-vorbis.webm",
      "vp9-vorbis.webm.reference.f32":
        "__test__/fixtures/vp9-vorbis.webm.reference.f32"
    } as const;
  }

  public narrowFixture = (s: string) => {
    return (
      s === "av1-aac.mp4" ||
      s === "cover.jpg" ||
      s === "fixture-info.json" ||
      s === "h264-aac-cover.mkv" ||
      s === "h264-aac-cover.mp4" ||
      s === "h264-aac-faststart.mp4" ||
      s === "h264-aac-faststart.mp4.reference.f32" ||
      s === "h264-aac-mdat-first.mp4" ||
      s === "h264-aac.mkv" ||
      s === "h264-aac.mov" ||
      s === "hevc-aac.mp4" ||
      s === "rotated-270.mp4" ||
      s === "rotated-90.mp4" ||
      s === "silent.mp4" ||
      s === "text-to-video-16x9-1080p.mp4" ||
      s === "two-audio.mp4" ||
      s === "video-gemini-omni-1.1-flash-1080p-16x9-006-delta_data.mp4" ||
      s === "video-gemini-omni-1.1-flash-1080p-16x9-009-delta_data.mp4" ||
      s === "video-gemini-omni-1.1-flash-4k-16x9-009-delta_data.mp4" ||
      s === "video-gemini-omni-1.1-flash-720p-16x9-008-delta_data.mp4" ||
      s === "video-gemini-omni-1.1-flash-720p-9x16-008-delta_data.mp4" ||
      s === "video-gemini-omni-1.1-flash-720p-9x16-009-delta_data.mp4" ||
      s === "vp9-opus-live.webm" ||
      s === "vp9-opus.webm" ||
      s === "vp9-vorbis.webm" ||
      s === "vp9-vorbis.webm.reference.f32"
    );
  };

  public getFixtureBuff<
    const T extends keyof typeof this.fixturePaths | (string & {}) = "form"
  >(t: T) {
    if (this.narrowFixture(t)) {
      return this.fileToBuffer(
        this.fixturePaths[t]
      ) satisfies Buffer<ArrayBuffer>;
    } else
      throw new Error(`invalid fixture name passed into getFixutreBuff ${t}`);
  }
}

if (process.argv[2] === "inspect" && process.argv[3]) {
  const workup = new WorkupService();
  if (workup.narrowFixture(process.argv[3])) {
    const fixtureBuff = workup.getFixtureBuff(process.argv[3]);
    console.log(fixtureBuff.toString("utf-8"));
  } else {
    console.warn(
      `${process.argv[3]} is an invalid argument.

Valid Options: "cbr.mp3" | "cbr.mp3.reference.f32" | "cherokee-dirt-interlunar-space.mp3" | "exact-float.wav" | "fixture-info.json" | "float.wav" | "grokina-meteorologist.mp3" | "lock-me-in-the-fauxcket.mp3" | "mpeg2-mono.mp3" | "no-xing.mp3" | "no-xing.mp3.reference.f32" | "pcm24.wav" | "pcm_alaw.wav" | "pcm_mulaw.wav" | "rf64.wav" | "stereo.wav" | "vbr.mp3" | "vbr.mp3.reference.f32"`
    );
  }
}

if (process.argv[2] === "--analyze") {
  const fs = new Fs(process.cwd());
  if (process.argv[3] === "paths") {
    const o = fs
      .readDir("__test__/fixtures", { recursive: true })
      .filter((t) => t.lastIndexOf(".") !== -1)
      .map((t) => `${t}`);
    const t = Object.fromEntries(
      o.map((v) => {
        return [v, `__test__/fixtures/${v}`] as const;
      })
    );
    console.log(t);
  }
}

