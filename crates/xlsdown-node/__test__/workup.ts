import { resolve } from "node:path";

import { Fs } from "@d0paminedriven/fs";

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
}

if (process.argv[2] === "--analyze") {
  const fs = new Fs(process.cwd());
  if (process.argv[3] === "paths") {
    const o = fs
      .readDir("__test__/fixtures", { recursive: false })
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
