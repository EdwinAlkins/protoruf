# Fuzzing

Coverage-guided fuzzing with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
(libFuzzer). This crate is separate from the library: nothing here is built into a wheel
or a package.

```bash
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked

cd fuzz
cargo +nightly fuzz run wire -- -max_total_time=60 -timeout=5
cargo +nightly fuzz run core_decode -- -max_total_time=300 -timeout=5
cargo +nightly fuzz run core_json corpus/core_json seeds/core_json -- \
    -dict=json.dict -max_total_time=300 -timeout=5
```

Give the working corpus (`corpus/<target>`, not versioned) first: libFuzzer writes new
inputs into the first directory, and `seeds/` must stay small. `-timeout=5` reports any
input that takes more than 5 s, which is how a quadratic path shows up.

| Target | What | Checks |
|--------|------|--------|
| `wire` | `native::wire` on arbitrary bytes, read as a message of unknown fields | no panic, positions move forward and stay in bounds, varints and zigzag round-trip |
| `core_decode` | arbitrary bytes → JSON (`core`, prost-reflect) for a message using every field kind | no panic; JSON that decodes encodes back to the same JSON |
| `core_json` | arbitrary text → protobuf for the same message (`json.dict` and `seeds/` help it reach valid JSON) | no panic; what encodes decodes back |

A failing input is saved under `artifacts/<target>/`; replay it with
`cargo +nightly fuzz run <target> artifacts/<target>/<file>`, then turn it into a test.

Not covered yet: the native Python codec (`python_native.rs`), built with PyO3's
`extension-module`, which a fuzzing binary cannot link; it would need a feature that
embeds Python.

Found so far: a double that did not survive `protobuf -> JSON -> protobuf` (one bit
off), fixed by serde_json's `float_roundtrip` (test `test_doubles_survive_json_exactly`).
