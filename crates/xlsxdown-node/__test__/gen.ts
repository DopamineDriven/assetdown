import { Fs } from "@d0paminedriven/fs";
const fs = new Fs(process.cwd());

const paths = fs
  .readDir("__test__/fixtures", { recursive: true }).filter((t) =>!t.endsWith(".ts") && t.lastIndexOf(".") !==-1)
  .map((t) => `__test__/fixtures/${t}`);

function filenameHelper(path: string) {
  const filenameWithExt = path.slice(path.lastIndexOf("/") + 1),
    filename = filenameWithExt.slice(0, filenameWithExt.lastIndexOf("."));
  return { sansExt: filename, withExt: filenameWithExt };
}

function genObj(paths: string[]) {
  const arr = Array.of<[string, string]>();
  for (const path of paths) {
    const { sansExt } = filenameHelper(path);
    arr.push([sansExt, path]);
  }
  return Object.fromEntries(arr);
}
const templateClass = `import { Fs } from "@d0paminedriven/fs";
import { resolve } from "node:path";

/**
 *
 * For \`Fs\` derived methods
 *
 * to access any path, use the *Tmp styled methods
 *
 * to leverage the cwd, use the standard methods
 *
 * eg
 *
 * \`readTmp\` &rarr; Absolute, uses \`resolve(...paths)\` where \`typeof paths = string[]\`
 *
 * \`fileToBuffer\`&rarr; Standard, uses \`relative(process.cwd(), path_arg_is_what_falls_in_here)\`
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
      return \`\${err}\`;
    } else return String(err);
  }

  public readAbsPath(
    paths: string[]
  ){
    return resolve(...paths);
  }

  public get workspaceRoot() {
    return this.readAbsPath([process.cwd(), "../.."])
  }

  public get fixturePaths() {
    return ${JSON.stringify(genObj(paths), null, 2)} as const;
  }

  public narrowFixture = (s: string) => {
    return (
      s === "date1904" ||
      s === "form" ||
      s === "macro" ||
      s === "minimal" ||
      s === "not-a-workbook" ||
      s === "ole" ||
      s === "strict"
    );
  };

  public getFixtureBuff<const T extends keyof typeof this.fixturePaths | (string & {}) = "form">(
    t: T
  ) {
    if (this.narrowFixture(t)) {
      return this.fileToBuffer(
        this.fixturePaths[t]
      ) satisfies Buffer<ArrayBuffer>;
    } else throw new Error(\`invalid fixture name passed into getFixutreBuff \${t}\`);
  }
}

if (process.argv[2] === "inspect" && process.argv[3]) {
  const workup = new WorkupService();
  if (workup.narrowFixture(process.argv[3])) {
    const fixtureBuff = workup.getFixtureBuff(process.argv[3]);
    console.log(fixtureBuff.toString("utf-8"));
  } else {
    console.warn(
      \`\${process.argv[3]} is an invalid argument.\n\nValid Options: "date1904" | "form" | "macro" | "minimal" | "not-a-workbook" | "ole" | "strict"\`
    );
  }
}
`;

if (process.argv[2] === "--gen" && process.argv[3] === "workup") {
  fs.withWs("__test__/workup.ts", templateClass);
}
