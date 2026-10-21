# Proto Files & Compilation (Browser / WASM)

In the browser there is no filesystem, so schemas are compiled **in memory** — or loaded as
pre-compiled descriptor bytes. Always [`await init()`](installation.md#initialise) first.

## `compileProtoFromSources(files, root)`

```ts
function compileProtoFromSources(files: Record<string, string>, root: string): Uint8Array;
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `files` | `Record<string, string>` | yes | Map of logical file name → source text. `import`s resolve by name within this map. |
| `root` | `string` | yes | Entry file to compile (a key of `files`) |

**Returns** `Uint8Array` — the serialized descriptor set. **Throws** on failure.

```ts
const descriptor = compileProtoFromSources(
  { "user.proto": 'syntax="proto3"; package user; message User { string id = 1; }' },
  "user.proto",
);
```

## Imports across files

`import`s are resolved **by name** within `files` — the whole tree can live in memory:

```ts
const descriptor = compileProtoFromSources(
  {
    "common.proto": 'syntax="proto3"; package common; message Id { string value = 1; }',
    "user.proto": 'syntax="proto3"; package user; import "common.proto"; message User { common.Id id = 1; }',
  },
  "user.proto",
);
```

## Google well-known types

`google/protobuf/*.proto` imports are resolved automatically (embedded in the WASM binary):

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

The **recommended** approach for fixed schemas: compile ahead of time (server/CLI) and ship only
the descriptor bytes. `fetch` them and skip compilation entirely:

```ts
const descriptor = new Uint8Array(await (await fetch("/schema.desc")).arrayBuffer());
const wire = jsonToProtobuf('{"id":"123"}', descriptor, "user.User");
```

This is faster, smaller, and avoids exposing `compileProtoFromSources` to untrusted input
(see [security](advanced.md#security-and-web-workers)).

## Troubleshooting

| Error message | Fix |
|---|---|
| `Failed to compile proto file: ... not found` | Add the imported file to the `files` map |
| `Message type '...' not found in descriptor` | Check the type matches `package` + message name |
| Module not initialised / `init` not called | `await init()` before any call |
| `disallowed MIME type ("text/html")` | Serve over HTTP from the correct root, not `file://` |

## Next Steps

- [Advanced Features](advanced.md)
- [API Reference](api.md)
