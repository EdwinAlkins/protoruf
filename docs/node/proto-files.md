# Proto Files & Compilation (Node.js)

This guide covers compiling `.proto` files: from disk, from memory, with imports and
well-known types, plus loading pre-compiled descriptors.

## `compileProto(protoPath, includePaths?)`

```ts
function compileProto(protoPath: string, includePaths?: string[]): Buffer;
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `protoPath` | `string` | yes | Path to the `.proto` file |
| `includePaths` | `string[]` | no | Include directories for `import`s (default: the parent directory of `protoPath`) |

**Returns** a `Buffer` — the serialized descriptor set. **Throws** on failure.

```ts
const descriptor = compileProto("message.proto");
```

## Working with imports

```
project/
└── protos/
    ├── common/types.proto
    └── api/service.proto
```

```protobuf
// api/service.proto
syntax = "proto3";
package api;
import "common/types.proto";

message Request {
  common.Timestamp timestamp = 1;
  string endpoint = 2;
}
```

```ts
const descriptor = compileProto("protos/api/service.proto", ["protos"]);
```

!!! tip "Include paths"
    `includePaths` tells the compiler where to resolve `import` statements. Imports are
    included in the descriptor automatically — one descriptor is self-contained.

## In-memory compilation

`compileProtoFromSources` compiles `.proto` sources with **no filesystem access** — useful for
schemas received over the network, embedded in your app, or compiled in a worker.

```ts
function compileProtoFromSources(files: Record<string, string>, root: string): Buffer;
```

`files` maps each logical file name to its source; `import`s are resolved **by name** within
that map. `root` is the entry file.

```ts
const descriptor = compileProtoFromSources(
  {
    "common.proto": 'syntax="proto3"; package common; message Id { string value = 1; }',
    "user.proto": 'syntax="proto3"; package user; import "common.proto"; message User { common.Id id = 1; }',
  },
  "user.proto",
);
```

### Google well-known types

`google/protobuf/*.proto` imports (e.g. `timestamp`, `struct`, `wrappers`) are resolved
automatically — you don't provide them:

```ts
const d = compileProtoFromSources(
  {
    "ev.proto":
      'syntax="proto3"; package ev; import "google/protobuf/timestamp.proto"; message Event { google.protobuf.Timestamp at = 1; }',
  },
  "ev.proto",
);
```

## Loading a pre-compiled descriptor

If your schemas are fixed, compile **ahead of time** and ship only the descriptor bytes. There
is no `loadDescriptor` helper — a descriptor is just bytes, so use Node's `fs`:

```ts
import { readFileSync, writeFileSync } from "node:fs";

// build step
writeFileSync("schema.desc", compileProto("schema.proto"));

// runtime (no .proto needed)
const descriptor = readFileSync("schema.desc");
```

This is faster than recompiling and avoids distributing `.proto` files.

## Best Practices

- **Use explicit packages** — `package myapp;` makes the message type `myapp.User`.
- **Compile once, reuse** — compiling is a one-time cost; hold the descriptor (or a
  `DescriptorCache`) for the process lifetime.
- **Ship descriptors in production** — pre-compile and load bytes instead of compiling at
  startup.

## Troubleshooting

| Error message | Fix |
|---|---|
| `Failed to compile proto file: ... not found` | Add the right `includePaths`, or include the imported file in `compileProtoFromSources` |
| `Message type '...' not found in descriptor` | Check the type matches `package` + message name |
| `Failed to load descriptor pool: ...` | Regenerate the descriptor; the bytes are corrupt or truncated |

## Next Steps

- [Advanced Features](advanced.md)
- [API Reference](api.md)
