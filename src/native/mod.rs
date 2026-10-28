//! Native codec: protobuf wire bytes <-> host-language values, with no intermediate
//! format (no JSON, no [`prost_reflect::DynamicMessage`]).
//!
//! This module is pure Rust: [`plan`] describes each message once, [`wire`] holds the
//! encoding primitives and [`wkt`] the calendar rules of the well-known types.
//! Bindings walk a plan to read or build their own values.

pub mod plan;
pub mod wire;
pub mod wkt;
