//! protoruf — high-performance JSON ↔ Protobuf conversion.
//!
//! [`core`] is pure Rust with no FFI dependencies. Each language binding lives in
//! its own module, gated behind a feature flag, and is only a thin wrapper that
//! translates types & errors around `core::*`:
//!
//! | feature  | module        | tool        | output            |
//! |----------|---------------|-------------|-------------------|
//! | `python` | [`python`]    | PyO3        | CPython extension |
//! | `node`   | [`node`]      | napi-rs     | Node.js `.node`   |
//! | `wasm`   | [`wasm`]      | wasm-bindgen| `.wasm` + JS glue |
//!
//! Building one target never pulls in the others' dependencies.

// With no binding feature enabled, `core`'s functions have no caller and would
// warn as dead code. A binding-less build isn't a real use case, so silence it
// in that configuration only; any enabled binding restores the warnings.
#![cfg_attr(
    not(any(feature = "python", feature = "node", feature = "wasm")),
    allow(dead_code)
)]

mod pool_cache;

pub mod core;
pub mod descriptor_resolver;

#[cfg(feature = "python")]
mod python;

#[cfg(feature = "node")]
mod node;

#[cfg(feature = "wasm")]
mod wasm;
