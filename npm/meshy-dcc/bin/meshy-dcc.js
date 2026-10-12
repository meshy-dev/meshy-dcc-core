#!/usr/bin/env node
// Runs the meshy-dcc binary from the platform package npm installed beside
// this one. meshy-dcc-server sits next to it, so the CLI finds its gateway.
"use strict";

const { spawnSync } = require("node:child_process");
const path = require("node:path");

const PLATFORM_PACKAGES = {
  "darwin arm64": "@meshy-ai/meshy-dcc-darwin-arm64",
  "win32 x64": "@meshy-ai/meshy-dcc-win32-x64",
};

const key = `${process.platform} ${process.arch}`;
const pkg = PLATFORM_PACKAGES[key];
if (!pkg) {
  console.error(
    `meshy-dcc is published for macOS (Apple Silicon) and Windows x64, not ${process.platform}/${process.arch}.`,
  );
  process.exit(1);
}

let dir;
try {
  dir = path.dirname(require.resolve(`${pkg}/package.json`));
} catch {
  console.error(
    `${pkg} is not installed. Reinstall meshy-dcc without --omit=optional or --no-optional.`,
  );
  process.exit(1);
}

const exe = path.join(dir, "bin", process.platform === "win32" ? "meshy-dcc.exe" : "meshy-dcc");
const result = spawnSync(exe, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`could not run ${exe}: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
