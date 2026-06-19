# Installation (Node.js)

This guide covers installing the Node.js target of protoruf — a native addon built with
[napi-rs](https://napi.rs/).

## Prerequisites

- **Node.js 12 or higher** (N-API v8)
- **npm**, **pnpm** or **yarn**
- Rust toolchain is **NOT required** — pre-built binaries are provided

## Install

```bash
npm i @protoruf/node
# or
pnpm add @protoruf/node
# or
yarn add @protoruf/node
```

Pre-built binaries are published per platform as `optionalDependencies`
(`@protoruf/node-linux-x64-gnu`, `-darwin-arm64`, `-win32-x64-msvc`, …) and npm installs only
the one matching the host — no toolchain required to consume the package.

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
