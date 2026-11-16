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
  'Catullus-and-Lucan-on-Pompey-and-Caesar.docx': '__test__/fixtures/Catullus-and-Lucan-on-Pompey-and-Caesar.docx',
  'Harvati-2003.docx': '__test__/fixtures/Harvati-2003.docx',
  'Lucans-Pharsalia-1.129-157.docx': '__test__/fixtures/Lucans-Pharsalia-1.129-157.docx',
  'Muscles-of-mastication.docx': '__test__/fixtures/Muscles-of-mastication.docx',
  'Muscles-of-the-Skull.docx': '__test__/fixtures/Muscles-of-the-Skull.docx',
  'Ongoing-Research-Notes.docx': '__test__/fixtures/Ongoing-Research-Notes.docx',
  'Poem-29.docx': '__test__/fixtures/Poem-29.docx',
  'Ramal-Morphology-Articles-Summary.docx': '__test__/fixtures/Ramal-Morphology-Articles-Summary.docx',
  'Summary-of-Mandibular-Ontogenetic-Patterns.docx': '__test__/fixtures/Summary-of-Mandibular-Ontogenetic-Patterns.docx',
  'andrew-ross-oct-2026.docx': '__test__/fixtures/andrew-ross-oct-2026.docx',
  'fonts-sample-dos-word.docx': '__test__/fixtures/fonts-sample-dos-word.docx',
  'lo-pandoc.docx': '__test__/fixtures/lo-pandoc.docx',
  'pandoc.docx': '__test__/fixtures/pandoc.docx'
} as const;
  }

  public narrowFixture = (s: string) => {
    return (
      s === "Catullus-and-Lucan-on-Pompey-and-Caesar.docx" ||
      s === "Harvati-2003.docx" ||
      s === "Lucans-Pharsalia-1.129-157.docx" ||
      s === "Muscles-of-mastication.docx" ||
      s === "Muscles-of-the-Skull.docx" ||
      s === "Ongoing-Research-Notes.docx" ||
      s === "Poem-29.docx" ||
      s === "Ramal-Morphology-Articles-Summary.docx" ||
      s === "Summary-of-Mandibular-Ontogenetic-Patterns.docx" ||
      s === "andrew-ross-oct-2026.docx" ||
      s === "fonts-sample-dos-word.docx" ||
      s === "lo-pandoc.docx" ||
      s === "pandoc.docx"
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

Valid Options: "Catullus-and-Lucan-on-Pompey-and-Caesar.docx" | "Harvati-2003.docx" | "Lucans-Pharsalia-1.129-157.docx" | "Muscles-of-mastication.docx" | "Muscles-of-the-Skull.docx" | "Ongoing-Research-Notes.docx" | "Poem-29.docx" | "Ramal-Morphology-Articles-Summary.docx" | "Summary-of-Mandibular-Ontogenetic-Patterns.docx" | "andrew-ross-oct-2026.docx" | "fonts-sample-dos-word.docx" | "lo-pandoc.docx" | "pandoc.docx"`
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

