# Installation (Node.js)

This guide covers installing the Node.js target of protoruf — a native addon built with
[napi-rs](https://napi.rs/).

## Prerequisites

- **Node.js 12 or higher** (N-API v8)
- **npm**, **pnpm** or **yarn**
- Rust toolchain is **NOT required** — pre-built binaries are provided

## Install

!!! note "v0.2.0 distribution"
    The npm registry release is coming soon. For now, install the prebuilt package from the
    [GitHub Releases page](https://github.com/EdwinAlkins/protoruf/releases) by URL:

```bash
npm install https://github.com/EdwinAlkins/protoruf/releases/download/v0.2.0/protoruf-node-0.2.0.tgz
```

The tarball is **self-contained**: it bundles the native binary for every supported platform
(Linux x64/arm64, macOS x64/arm64, Windows x64), and the loader picks the one matching the host
at runtime — no Rust toolchain, nothing else to resolve.

!!! tip "Once on npm"
    The registry release will install with the usual `npm i @protoruf/node` (per-platform
    binaries shipped as `optionalDependencies`, only the matching one downloaded). The `import`
    statements below are identical either way.

## Verify Installation

```ts
import { compileProtoFromSources, jsonToProtobuf } from "@protoruf/node";

const descriptor = compileProtoFromSources(
  { "m.proto": 'syntax="proto3"; package m; message M { string id = 1; }' },
  "m.proto",
);
console.log(jsonToProtobuf('{"id":"123"}', descriptor, "m.M").length, "bytes");
```

If this prints a byte count without errors, you're ready.

!!! note "TypeScript"
    Type definitions ship with the package (`index.d.ts`), so editors get autocomplete and
    type-checking out of the box — no `@types/...` needed.

## Platform Support

Pre-built binaries are available for:

| Platform | Architecture |
|----------|--------------|
| Linux | x86_64, aarch64 |
| macOS | x86_64, arm64 |
| Windows | x86_64 |

!!! tip "Unsupported platform?"
    If no pre-built binary matches your platform, build from source (below).

## Development / Build from Source

Requires the [Rust toolchain](https://rustup.rs/) and `@napi-rs/cli`. From the repository root:

```bash
git clone https://github.com/EdwinAlkins/protoruf.git
cd protoruf
npm install
npm run build            # napi build --release --features node -> dist/
```

This produces `dist/index.js`, `dist/index.d.ts` and the platform `.node` addon. See the build
guide for the full matrix: <https://github.com/EdwinAlkins/protoruf/blob/main/dev-docs/npm-build.md>.

## Next Steps

- Follow the [Quick Start](quick-start.md) to convert your first message
- Read [Basic Usage](basic-usage.md) for all supported Protobuf features
