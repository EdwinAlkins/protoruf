#!/usr/bin/env node
// Post-build step: turn a `wasm-pack` output directory into a publishable
// `@protoruf/wasm` npm package.
//
// `wasm-pack` derives the package name and version from Cargo.toml, so the raw
// output is named "protoruf". This script renames it to the scoped package,
// keeps its version in lockstep with `@protoruf/node` (the root package.json),
// fills in the publish metadata, and drops in the browser README.
//
// The published @protoruf/wasm targets the browser, so it is built with
// `wasm-pack --target web` into dist/wasm-web (the dist/wasm nodejs build stays
// for the test/benchmark harness).
//
// Usage:  node scripts/prepare-npm-wasm.mjs [outDir]   (default: dist/wasm-web)

import { readFileSync, writeFileSync, copyFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = process.argv[2] ?? join(root, "dist", "wasm-web");

const pkgPath = join(outDir, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const rootPkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));

pkg.name = "@protoruf/wasm";
pkg.version = rootPkg.version; // keep @protoruf/node and @protoruf/wasm in lockstep
pkg.description =
  "High-performance JSON <-> Protobuf conversion, powered by Rust (WebAssembly / browser).";
pkg.license = rootPkg.license;
pkg.repository = rootPkg.repository;
pkg.homepage = rootPkg.homepage;
pkg.bugs = rootPkg.bugs;
pkg.keywords = ["protobuf", "json", "rust", "wasm", "webassembly", "serialization"];

if (!Array.isArray(pkg.files)) pkg.files = [];
if (!pkg.files.includes("README.md")) pkg.files.push("README.md");

writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
copyFileSync(join(root, "npm", "wasm", "README.md"), join(outDir, "README.md"));

console.log(`Prepared ${pkg.name}@${pkg.version} in ${outDir}`);
