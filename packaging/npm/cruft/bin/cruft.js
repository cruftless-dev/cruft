#!/usr/bin/env node

"use strict";

const { spawnSync } = require("node:child_process");

const PLATFORM_PACKAGES = {
  "linux-x64": "@cruftless-dev/cruft-linux-x64",
  "linux-arm64": "@cruftless-dev/cruft-linux-arm64",
  "darwin-x64": "@cruftless-dev/cruft-darwin-x64",
  "darwin-arm64": "@cruftless-dev/cruft-darwin-arm64",
  "win32-x64": "@cruftless-dev/cruft-win32-x64",
};

function invokedAsCpx() {
  if (process.env.CRUFT_NPM_COMMAND === "cpx") {
    return true;
  }
  return /(^|[\\/])cpx(?:\.cmd|\.ps1|\.js)?$/i.test(process.argv[1] || "");
}

function resolveBinary() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = PLATFORM_PACKAGES[key];
  if (!pkg) {
    throw new Error(
      `cruft: no prebuilt binary is available for your platform (${key}).\n` +
        `Supported platforms: ${Object.keys(PLATFORM_PACKAGES).join(", ")}.`
    );
  }
  const binName = process.platform === "win32" ? "cruft.exe" : "cruft";
  try {
    return require.resolve(`${pkg}/${binName}`);
  } catch (_e) {
    throw new Error(
      `cruft: the platform package ${pkg} for ${key} is not installed.\n` +
        `This usually means cruft was installed with optional dependencies disabled ` +
        `(e.g. --no-optional or --omit=optional). Reinstall cruft so npm can add the ` +
        `matching binary package for your platform.`
    );
  }
}

let binary;
const commandName = invokedAsCpx() ? "cpx" : "cruft";
try {
  binary = resolveBinary();
} catch (err) {
  process.stderr.write(`${err && err.message ? err.message : err}\n`);
  process.exit(1);
}

const forwarded = commandName === "cpx"
  ? ["exec", ...process.argv.slice(2)]
  : process.argv.slice(2);
try { require("node:fs").chmodSync(binary, 0o755); } catch (_e) {}
const result = spawnSync(binary, forwarded, { stdio: "inherit" });
if (result.error) {
  process.stderr.write(`cruft: failed to launch ${binary}: ${result.error.message}\n`);
  process.exit(1);
}

if (result.signal) {
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
