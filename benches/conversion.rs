use criterion::{Criterion, criterion_group, criterion_main};
use protoruf::core;
use protoruf::descriptor_resolver::DescriptorResolver;
use std::collections::HashMap;
use std::hint::black_box;
use std::path::PathBuf;

fn test_descriptor() -> Vec<u8> {
    let proto_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/proto/message.proto");
    core::compile_proto(proto_path.to_str().unwrap(), None).unwrap()
}

fn sample_json() -> String {
    r#"{
        "id": "bench-123",
        "content": "Hello World! This is a benchmark.",
        "priority": 5,
        "tags": ["test", "example", "bench"],
        "metadata": {
            "author": "Alice",
            "created_at": 1234567890,
            "attributes": { "env": "prod", "version": "1.0" }
        }
    }"#
    .to_string()
}

fn bench_free_global_lru_json_to_proto(c: &mut Criterion) {
    let descriptor = test_descriptor();
    let json = sample_json();

    c.bench_function("free_global_lru_json_to_proto", |b| {
        b.iter(|| {
            core::json_to_protobuf_bytes(
                black_box(&json),
                black_box(&descriptor),
                "message.Message",
            )
            .unwrap()
        });
    });
}

fn bench_hot_json_to_proto(c: &mut Criterion) {
    let descriptor = test_descriptor();
    let resolver = DescriptorResolver::from_descriptor_bytes(&descriptor).unwrap();
    let json = sample_json();

    c.bench_function("hot_json_to_proto", |b| {
        b.iter(|| {
            let desc = resolver.resolve("message.Message").unwrap();
            core::json_to_protobuf_bytes_with_descriptor_owned(black_box(&json), desc).unwrap()
        });
    });
}

fn bench_free_global_lru_proto_to_json(c: &mut Criterion) {
    let descriptor = test_descriptor();
    let json = sample_json();
    let protobuf = core::json_to_protobuf_bytes(&json, &descriptor, "message.Message").unwrap();

    c.bench_function("free_global_lru_proto_to_json", |b| {
        b.iter(|| {
            core::protobuf_to_json_string(
                black_box(&protobuf),
                black_box(&descriptor),
                "message.Message",
                false,
            )
            .unwrap()
        });
    });
}

fn bench_hot_proto_to_json(c: &mut Criterion) {
    let descriptor = test_descriptor();
    let resolver = DescriptorResolver::from_descriptor_bytes(&descriptor).unwrap();
    let json = sample_json();
    let protobuf = core::json_to_protobuf_bytes(&json, &descriptor, "message.Message").unwrap();

    c.bench_function("hot_proto_to_json", |b| {
        b.iter(|| {
            let desc = resolver.resolve("message.Message").unwrap();
            core::protobuf_to_json_string_with_descriptor_owned(black_box(&protobuf), desc, false)
                .unwrap()
        });
    });
}

fn bench_compile_proto_from_sources(c: &mut Criterion) {
    let proto = r#"syntax = "proto3"; package user; message User { string id = 1; repeated string tags = 2; }"#;
    let files = HashMap::from([("user.proto".to_string(), proto.to_string())]);

    c.bench_function("compile_proto_from_sources", |b| {
        b.iter(|| {
            core::compile_proto_from_sources(black_box(files.clone()), "user.proto", true).unwrap()
        });
    });
}

fn large_descriptor() -> Vec<u8> {
    let mut source =
        String::from(r#"syntax = "proto3"; package bench; message Item { string id = 1; }"#);
    for index in 0..10_000 {
        source.push_str(&format!(" message Padding{index} {{ string value = 1; }}"));
    }
    let files = HashMap::from([("large_descriptor.proto".to_string(), source)]);
    let descriptor =
        core::compile_proto_from_sources(files, "large_descriptor.proto", true).unwrap();
    assert!(descriptor.len() > 100_000);
    descriptor
}

fn bench_large_descriptor_paths(c: &mut Criterion) {
    let descriptor = large_descriptor();
    let json = r#"{"id":"bench"}"#;
    let resolver = DescriptorResolver::from_descriptor_bytes(&descriptor).unwrap();
    let message_descriptor = resolver.resolve("bench.Item").unwrap();

    // Prime the global LRU before measuring hits.
    core::json_to_protobuf_bytes(json, &descriptor, "bench.Item").unwrap();

    let mut group = c.benchmark_group("large_descriptor");
    group.sample_size(20);
    group.bench_function("decode_pool", |b| {
        b.iter(|| core::load_descriptor_pool(black_box(&descriptor)).unwrap());
    });
    group.bench_function("free_global_lru_hit", |b| {
        b.iter(|| {
            core::json_to_protobuf_bytes(black_box(json), black_box(&descriptor), "bench.Item")
                .unwrap()
        });
    });
    group.bench_function("explicit_cache", |b| {
        b.iter(|| {
            core::json_to_protobuf_bytes_with_descriptor_owned(
                black_box(json),
                message_descriptor.clone(),
            )
            .unwrap()
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_free_global_lru_json_to_proto,
    bench_hot_json_to_proto,
    bench_free_global_lru_proto_to_json,
    bench_hot_proto_to_json,
    bench_compile_proto_from_sources,
    bench_large_descriptor_paths,
);
criterion_main!(benches);
