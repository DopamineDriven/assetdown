import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";

import ocrPkgJson from "../ocr/package.json" with { type: "json" };
import pkgJson from "../package.json" with { type: "json" };
interface RepositoryEntity {
  type: string;
  url: string;
}

interface PublishConfigEntity {
  access: string;
  registry: string;
}

interface NapiEntity {
  binaryName: string;
  packageName: string;
  constEnum: boolean;
  dtsHeader: string;
  targets: string[];
}

interface EnginesEntity {
  node: string;
}

interface ConfigObjEntity {
  [record: string]: string;
}

export interface PkgJsonEntity {
  name: string;
  version: string;
  description: string;
  packageManager: string;
  keywords: string[];
  license: string;
  repository: RepositoryEntity;
  resolutions: ConfigObjEntity;
  files: string[];
  type: string;
  main: string;
  types: string;
  browser: string;
  publishConfig: PublishConfigEntity;
  scripts: ConfigObjEntity;
  devDependencies: ConfigObjEntity;
  napi: NapiEntity;
  engines: EnginesEntity;
}
/**
 * The CI plan for this package, derived from `napi.targets` in package.json, and for the OCR
 * package built from the same crate, derived from `napi.targets` in ocr/package.json.
 *
 * Adding or removing a target there is the whole change: this script decides
 * where each target is built and, when a runner can execute it, where it is
 * tested. A target without an entry in `plans` fails the run with its name,
 * so a new target is never built or skipped by accident.
 *
 * It prints `name=json` lines for `$GITHUB_OUTPUT`:
 *
 *   node ci/matrix.ts >> "$GITHUB_OUTPUT"
 *
 * Plain `node` runs it: there are no dependencies and nothing here needs more
 * than Node's own type stripping, so it works before `yarn install`.
 */

/** The Node version every target is built and tested with. */
const NODE_CURRENT = "26";
/** The oldest supported Node (`engines.node`), tested on one native target. */
const NODE_FLOOR = "24";
/** The one target that is also tested on `NODE_FLOOR` and typechecked. */
const REFERENCE_TARGET = "x86_64-unknown-linux-gnu";

interface RunnerBuild {
  readonly kind: "runner";
  /** The GitHub-hosted runner that builds the target. */
  readonly host: string;
  /** Extra `napi build` flags: the cross toolchain, or zig for musl. */
  readonly flags: string;
}

/** Built inside a FreeBSD virtual machine, which has its own job. */
interface FreebsdBuild {
  readonly kind: "freebsd";
}

type BuildPlan = RunnerBuild | FreebsdBuild;

/** Tested directly on a runner of the target's own platform. */
interface HostTest {
  readonly kind: "host";
  readonly host: string;
  readonly architecture: "x64" | "arm64";
}

/** Tested in a Node container, which is how musl gets a real musl userland. */
interface DockerTest {
  readonly kind: "docker";
  readonly host: string;
  readonly platform: "linux/amd64" | "linux/arm64";
  readonly libc: "gnu" | "musl";
}

interface WasiTest {
  readonly kind: "wasi";
}

/** Built and published, with no runner here that can execute it. */
interface NoTest {
  readonly kind: "none";
}

type TestPlan = HostTest | DockerTest | WasiTest | NoTest;

interface TargetPlan {
  readonly build: BuildPlan;
  readonly test: TestPlan;
}

const untested = { kind: "none" } as const satisfies NoTest;

const plans = new Map<string, TargetPlan>(
  Object.entries({
    "x86_64-apple-darwin": {
      build: { kind: "runner", host: "macos-latest", flags: "" },
      test: { kind: "host", host: "macos-latest", architecture: "x64" }
    },
    "aarch64-apple-darwin": {
      build: { kind: "runner", host: "macos-latest", flags: "" },
      test: { kind: "host", host: "macos-latest", architecture: "arm64" }
    },
    "x86_64-pc-windows-msvc": {
      build: { kind: "runner", host: "windows-latest", flags: "" },
      test: { kind: "host", host: "windows-latest", architecture: "x64" }
    },
    "i686-pc-windows-msvc": {
      build: { kind: "runner", host: "windows-latest", flags: "" },
      test: untested
    },
    "aarch64-pc-windows-msvc": {
      build: { kind: "runner", host: "windows-latest", flags: "" },
      test: { kind: "host", host: "windows-11-arm", architecture: "arm64" }
    },
    "x86_64-unknown-linux-gnu": {
      build: {
        kind: "runner",
        host: "ubuntu-latest",
        flags: "--use-napi-cross"
      },
      test: {
        kind: "docker",
        host: "ubuntu-latest",
        platform: "linux/amd64",
        libc: "gnu"
      }
    },
    "aarch64-unknown-linux-gnu": {
      build: {
        kind: "runner",
        host: "ubuntu-latest",
        flags: "--use-napi-cross"
      },
      test: {
        kind: "docker",
        host: "ubuntu-24.04-arm",
        platform: "linux/arm64",
        libc: "gnu"
      }
    },
    "armv7-unknown-linux-gnueabihf": {
      build: {
        kind: "runner",
        host: "ubuntu-latest",
        flags: "--use-napi-cross"
      },
      test: untested
    },
    "x86_64-unknown-linux-musl": {
      build: { kind: "runner", host: "ubuntu-latest", flags: "-x" },
      test: {
        kind: "docker",
        host: "ubuntu-latest",
        platform: "linux/amd64",
        libc: "musl"
      }
    },
    "aarch64-unknown-linux-musl": {
      build: { kind: "runner", host: "ubuntu-latest", flags: "-x" },
      test: {
        kind: "docker",
        host: "ubuntu-24.04-arm",
        platform: "linux/arm64",
        libc: "musl"
      }
    },
    "aarch64-linux-android": {
      build: { kind: "runner", host: "ubuntu-latest", flags: "" },
      test: untested
    },
    "armv7-linux-androideabi": {
      build: { kind: "runner", host: "ubuntu-latest", flags: "" },
      test: untested
    },
    "wasm32-wasip1-threads": {
      build: { kind: "runner", host: "ubuntu-latest", flags: "" },
      test: { kind: "wasi" }
    },
    "x86_64-unknown-freebsd": {
      build: { kind: "freebsd" },
      test: untested
    }
  } as const satisfies Record<string, TargetPlan>)
);

function dockerImage(node: string, libc: DockerTest["libc"]) {
  return `node:${node}-${libc === "musl" ? "alpine" : "slim"}`;
}

const build = Array.of<{ target: string; host: string; flags: string }>();
const testHost = Array.of<{
  target: string;
  host: string;
  architecture: string;
  node: string;
}>();
const testDocker = Array.of<{
  target: string;
  host: string;
  platform: string;
  image: string;
  node: string;
  typecheck: boolean;
}>();

export const allPossibleTargets = [
  "aarch64-apple-darwin",
  "aarch64-linux-android",
  "aarch64-unknown-linux-gnu",
  "aarch64-unknown-linux-musl",
  "aarch64-unknown-linux-ohos",
  "aarch64-pc-windows-msvc",
  "x86_64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "x86_64-pc-windows-gnu",
  "x86_64-unknown-linux-gnu",
  "x86_64-unknown-linux-musl",
  "x86_64-unknown-linux-ohos",
  "x86_64-unknown-freebsd",
  "i686-pc-windows-msvc",
  "armv7-unknown-linux-gnueabihf",
  "armv7-unknown-linux-musleabihf",
  "armv7-linux-androideabi",
  "universal-apple-darwin",
  "loongarch64-unknown-linux-gnu",
  "riscv64gc-unknown-linux-gnu",
  "powerpc64le-unknown-linux-gnu",
  "s390x-unknown-linux-gnu",
  "wasm32-wasi-preview1-threads",
  "wasm32-wasip1-threads"
];

const both = [...pkgJson.napi.targets, ...allPossibleTargets];

function determineTargets(both: string[]) {
  const arr = Array.of<string>();
  let record: Record<string, number> = {};
  both.forEach(function (t) {
    record[t] = (record[t] ?? 0) + 1;
  });
  for (const [target, count] of Object.entries(record)) {
    if (count > 1) {
      arr.push(target);
    } else continue;
  }
  return arr;
}

const verifiableTargets = determineTargets(both);

let wasi = false;
let freebsd = false;

for (const target of verifiableTargets) {
  const plan = plans.get(target);
  if (plan === undefined) {
    throw new Error(
      `no CI plan for target ${target}; add it to plans in ci/matrix.ts`
    );
  }
  if (plan.build.kind === "runner") {
    build.push({ target, host: plan.build.host, flags: plan.build.flags });
  } else {
    freebsd = true;
  }
  if (plan.test.kind === "host") {
    testHost.push({
      target,
      host: plan.test.host,
      architecture: plan.test.architecture,
      node: NODE_CURRENT
    });
  } else if (plan.test.kind === "docker") {
    const reference = target === REFERENCE_TARGET;
    testDocker.push({
      target,
      host: plan.test.host,
      platform: plan.test.platform,
      image: dockerImage(NODE_CURRENT, plan.test.libc),
      node: NODE_CURRENT,
      typecheck: reference
    });
    if (reference) {
      testDocker.push({
        target,
        host: plan.test.host,
        platform: plan.test.platform,
        image: dockerImage(NODE_FLOOR, plan.test.libc),
        node: NODE_FLOOR,
        typecheck: false
      });
    }
  } else if (plan.test.kind === "wasi") {
    wasi = true;
  }
}

/**
 * The OCR package's targets, from `napi.targets` in ocr/package.json. Each is built and tested on
 * a runner of its own platform, because the build links that system's Tesseract and Leptonica.
 * The Linux runners are the oldest hosted ones, so the binaries need no newer a glibc than that.
 */
const ocrPlans = new Map<string, { readonly host: string }>(
  Object.entries({
    "aarch64-apple-darwin": { host: "macos-latest" },
    "x86_64-unknown-linux-gnu": { host: "ubuntu-22.04" },
    "aarch64-unknown-linux-gnu": { host: "ubuntu-22.04-arm" }
  } as const satisfies Record<string, { readonly host: string }>)
);

const ocr = Array.of<{ target: string; host: string }>();

for (const target of ocrPkgJson.napi.targets) {
  const plan = ocrPlans.get(target);
  if (plan === undefined) {
    throw new Error(
      `no CI plan for OCR target ${target}; add it to ocrPlans in ci/matrix.ts`
    );
  }
  ocr.push({ target, host: plan.host });
}

/**
 * Whether this run is this package's to do.
 *
 * A commit named `<package>@<version>` releases one package, but it starts the workflow of
 * every package whose files it touches. Only the package it names goes on past this plan; the
 * others stop here, so one release does not queue every pipeline. Any other commit runs
 * whatever its paths started, and a run started by hand always goes on.
 */
const releaseName = pkgJson.name.slice(pkgJson.name.lastIndexOf("/") + 1);
/** One release name for each `crates/<name>-node`: the packages this repository publishes. */
const releaseNames = readdirSync(new URL("../../", import.meta.url))
  .filter((entry) => entry.endsWith("-node"))
  .map((entry) => entry.slice(0, -"-node".length));
const log = spawnSync("git", ["log", "-1", "--pretty=%s"], {
  encoding: "utf8"
});
// Outside a checkout there is no commit to read, and the run goes on.
const subject = log.status === 0 ? log.stdout.trim() : "";
const released = releaseNames.find((entry) =>
  new RegExp(`^${entry}@\\d`).test(subject)
);
const run =
  process.env.GITHUB_EVENT_NAME === "workflow_dispatch" ||
  released === undefined ||
  released === releaseName;

process.stdout.write(
  [
    `run=${run}`,
    `build=${JSON.stringify(build)}`,
    `test_host=${JSON.stringify(testHost)}`,
    `test_docker=${JSON.stringify(testDocker)}`,
    `wasi=${wasi}`,
    `freebsd=${freebsd}`,
    `ocr=${JSON.stringify(ocr)}`
  ]
    .join("\n")
    .concat("\n")
);
