import { readFileSync } from "node:fs";

/**
 * The CI plan for this package, derived from `napi.targets` in package.json.
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

/** `package.json` as parsed JSON, of no known shape yet. */
function readManifest(): unknown {
  return JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8")
  );
}

/** The target triples in `napi.targets`, or an error saying what is wrong. */
function napiTargets(manifest: unknown) {
  if (
    typeof manifest !== "object" ||
    manifest === null ||
    !("napi" in manifest)
  ) {
    throw new Error("package.json has no napi section");
  }
  const { napi } = manifest;
  if (
    typeof napi !== "object" ||
    napi === null ||
    !("targets" in napi) ||
    !Array.isArray(napi.targets)
  ) {
    throw new Error("package.json napi.targets is not an array");
  }
  const targets = Array.of<string>();
  for (const target of napi.targets) {
    if (typeof target !== "string") {
      throw new Error("package.json napi.targets holds a non-string entry");
    }
    targets.push(target);
  }
  return targets;
}

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
let wasi = false;
let freebsd = false;

for (const target of napiTargets(readManifest())) {
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

process.stdout.write(
  [
    `build=${JSON.stringify(build)}`,
    `test_host=${JSON.stringify(testHost)}`,
    `test_docker=${JSON.stringify(testDocker)}`,
    `wasi=${String(wasi)}`,
    `freebsd=${String(freebsd)}`
  ]
    .join("\n")
    .concat("\n")
);
