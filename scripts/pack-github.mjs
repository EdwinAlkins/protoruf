#!/usr/bin/env node
// Build self-contained tarballs for distribution via GitHub Releases — install by
// URL, no npm registry, no scope, no token.
//
//   @protoruf/wasm : packed from dist/wasm-web (one portable .wasm)
//   @protoruf/node : a single bundle containing index.js + index.d.ts + EVERY
//                    locally-built *.node. The napi loader resolves the native
//                    binary sitting next to index.js, so there are no per-platform
//                    sub-packages and no optionalDependencies to fetch.
//
// Locally you only have your own platform's .node, so the node bundle supports
// that platform; in CI (all targets built into dist/) the same script bundles them
// all. Output lands in ./release/.

import {
  readFileSync, writeFileSync, mkdirSync, copyFileSync, rmSync, readdirSync, existsSync,
} from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const outDir = join(root, "release");
const stage = join(dist, "github-node");

const rootPkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
mkdirSync(outDir, { recursive: true });

function npmPack(dir) {
  const out = execFileSync("npm", ["pack", dir, "--pack-destination", outDir], {
    cwd: root,
    encoding: "utf8",
  });
  const lines = out.trim().split("\n").map((s) => s.trim()).filter(Boolean);
  return [...lines].reverse().find((l) => l.endsWith(".tgz")) ?? lines.pop();
}

// --- @protoruf/wasm ---------------------------------------------------------
const wasmDir = join(dist, "wasm-web");
if (!existsSync(join(wasmDir, "package.json"))) {
  console.error("dist/wasm-web missing — run `npm run pack:wasm` first.");
  process.exit(1);
}
console.log(`✓ ${npmPack(wasmDir)}`);

// --- @protoruf/node (self-contained bundle) ---------------------------------
const nodeBinaries = readdirSync(dist).filter((f) => f.endsWith(".node"));
if (nodeBinaries.length === 0) {
  console.error("No *.node in dist/ — run `npm run build` first.");
  process.exit(1);
}

rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });
for (const f of ["index.js", "index.d.ts", ...nodeBinaries]) {
  copyFileSync(join(dist, f), join(stage, f));
}
copyFileSync(join(root, "npm", "node", "README.md"), join(stage, "README.md"));

writeFileSync(
  join(stage, "package.json"),
  JSON.stringify(
    {
      name: "@protoruf/node",
      version: rootPkg.version,
      description: rootPkg.description,
      main: "index.js",
      types: "index.d.ts",
      files: ["index.js", "index.d.ts", "README.md", ...nodeBinaries],
      license: rootPkg.license,
      repository: rootPkg.repository,
      homepage: rootPkg.homepage,
      bugs: rootPkg.bugs,
      keywords: rootPkg.keywords,
      engines: rootPkg.engines,
    },
    null,
    2,
  ) + "\n",
);
console.log(`✓ ${npmPack(stage)}`);

console.log(`\nTarballs ready in ./release/ — attach them to a GitHub Release.`);
console.log(`Bundled native targets: ${nodeBinaries.join(", ")}`);
console.log(`Install via URL, e.g.:`);
console.log(`  npm install https://github.com/EdwinAlkins/protoruf/releases/download/v${rootPkg.version}/protoruf-node-${rootPkg.version}.tgz`);
