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
      "cbr.mp3": "__test__/fixtures/cbr.mp3",
      "cbr.mp3.reference.f32": "__test__/fixtures/cbr.mp3.reference.f32",
      "cherokee-dirt-interlunar-space.mp3":
        "__test__/fixtures/cherokee-dirt-interlunar-space.mp3",
      "exact-float.wav": "__test__/fixtures/exact-float.wav",
      "fixture-info.json": "__test__/fixtures/fixture-info.json",
      "float.wav": "__test__/fixtures/float.wav",
      "grokina-meteorologist.mp3":
        "__test__/fixtures/grokina-meteorologist.mp3",
      "lock-me-in-the-fauxcket.mp3":
        "__test__/fixtures/lock-me-in-the-fauxcket.mp3",
      "mpeg2-mono.mp3": "__test__/fixtures/mpeg2-mono.mp3",
      "no-xing.mp3": "__test__/fixtures/no-xing.mp3",
      "no-xing.mp3.reference.f32":
        "__test__/fixtures/no-xing.mp3.reference.f32",
      "pcm24.wav": "__test__/fixtures/pcm24.wav",
      "pcm_alaw.wav": "__test__/fixtures/pcm_alaw.wav",
      "pcm_mulaw.wav": "__test__/fixtures/pcm_mulaw.wav",
      "rf64.wav": "__test__/fixtures/rf64.wav",
      "stereo.wav": "__test__/fixtures/stereo.wav",
      "vbr.mp3": "__test__/fixtures/vbr.mp3",
      "vbr.mp3.reference.f32": "__test__/fixtures/vbr.mp3.reference.f32"
    } as const;
  }

  public narrowFixture = (s: string) => {
    return (
      s === "cbr.mp3" ||
      s === "cbr.mp3.reference.f32" ||
      s === "cherokee-dirt-interlunar-space.mp3" ||
      s === "exact-float.wav" ||
      s === "fixture-info.json" ||
      s === "float.wav" ||
      s === "grokina-meteorologist.mp3" ||
      s === "lock-me-in-the-fauxcket.mp3" ||
      s === "mpeg2-mono.mp3" ||
      s === "no-xing.mp3" ||
      s === "no-xing.mp3.reference.f32" ||
      s === "pcm24.wav" ||
      s === "pcm_alaw.wav" ||
      s === "pcm_mulaw.wav" ||
      s === "rf64.wav" ||
      s === "stereo.wav" ||
      s === "vbr.mp3" ||
      s === "vbr.mp3.reference.f32"
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
