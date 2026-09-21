//! Build script.
//!
//! napi-rs requires a one-time setup to produce a loadable Node addon (mainly
//! symbol-export linker arguments on macOS/Windows). We run it ONLY when the
//! `node` feature is enabled, detected via the `CARGO_FEATURE_NODE` env var that
//! Cargo sets for active features. The Python, WASM and default builds therefore
//! behave exactly as before (this becomes a no-op).
fn main() {
    if std::env::var_os("CARGO_FEATURE_NODE").is_some() {
        napi_build::setup();
    }
}
