//! Standard `.proto` files embedded in the library, beyond the `google/protobuf/*`
//! well-known types that protox already provides.
//!
//! The sources are the unmodified copies in `third_party/proto/` (Apache-2.0, see the
//! README and LICENSE there), so compiling against them needs no file on disk.

use protox::Error as ProtoxError;
use protox::file::{File, FileResolver};

/// `(import name, source)` of every embedded file.
pub const STANDARD_PROTOS: &[(&str, &str)] = &[
    (
        "google/rpc/code.proto",
        include_str!("../third_party/proto/google/rpc/code.proto"),
    ),
    (
        "google/rpc/error_details.proto",
        include_str!("../third_party/proto/google/rpc/error_details.proto"),
    ),
    (
        "google/rpc/status.proto",
        include_str!("../third_party/proto/google/rpc/status.proto"),
    ),
    (
        "google/type/date.proto",
        include_str!("../third_party/proto/google/type/date.proto"),
    ),
    (
        "google/type/datetime.proto",
        include_str!("../third_party/proto/google/type/datetime.proto"),
    ),
    (
        "google/type/latlng.proto",
        include_str!("../third_party/proto/google/type/latlng.proto"),
    ),
    (
        "google/type/money.proto",
        include_str!("../third_party/proto/google/type/money.proto"),
    ),
    (
        "google/type/timeofday.proto",
        include_str!("../third_party/proto/google/type/timeofday.proto"),
    ),
    (
        "grpc/health/v1/health.proto",
        include_str!("../third_party/proto/grpc/health/v1/health.proto"),
    ),
    (
        "grpc/reflection/v1/reflection.proto",
        include_str!("../third_party/proto/grpc/reflection/v1/reflection.proto"),
    ),
    (
        "grpc/reflection/v1alpha/reflection.proto",
        include_str!("../third_party/proto/grpc/reflection/v1alpha/reflection.proto"),
    ),
];

/// Serves [`STANDARD_PROTOS`] by import name.
pub struct StandardProtoResolver;

impl FileResolver for StandardProtoResolver {
    fn open_file(&self, name: &str) -> Result<File, ProtoxError> {
        match STANDARD_PROTOS.iter().find(|(path, _)| *path == name) {
            Some((path, source)) => File::from_source(path, source),
            None => Err(ProtoxError::file_not_found(name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn embeds_every_vendored_file() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("third_party/proto");
        let mut on_disk = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|e| e == "proto") {
                    let name = path.strip_prefix(&root).unwrap().to_str().unwrap();
                    on_disk.push(name.replace('\\', "/"));
                }
            }
        }
        on_disk.sort();
        let mut embedded: Vec<String> = STANDARD_PROTOS
            .iter()
            .map(|(name, _)| name.to_string())
            .collect();
        embedded.sort();
        assert_eq!(embedded, on_disk);
    }

    #[test]
    fn resolves_by_import_name_only() {
        assert!(
            StandardProtoResolver
                .open_file("google/rpc/status.proto")
                .is_ok()
        );
        let missing = StandardProtoResolver.open_file("google/rpc/nope.proto");
        assert!(missing.is_err_and(|e| e.is_file_not_found()));
    }
}
