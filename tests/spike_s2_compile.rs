//! Spike S2: does protox give everything fastgrpc3 needs to compile `.proto` files
//! without `protoc`? One test per question; findings in
//! `dev-docs/spike-s2-protox-compile.md`.
//!
//! Run with `cargo test --test spike_s2_compile -- --nocapture` to see the printed
//! measurements and error messages.

use prost::Message;
use prost_reflect::prost_types::{FileDescriptorProto, FileDescriptorSet};
use prost_reflect::{DescriptorPool, Kind};
use protox::Compiler;
use protox::file::{
    ChainFileResolver, File, FileResolver, GoogleFileResolver, IncludeFileResolver,
};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Instant;

fn third_party() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("third_party/proto")
}

/// `.proto` sources held in memory, resolved by name.
struct Memory(HashMap<String, String>);

impl FileResolver for Memory {
    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        match self.0.get(name) {
            Some(source) => File::from_source(name, source),
            None => Err(protox::Error::file_not_found(name)),
        }
    }
}

/// In-memory sources first, then the vendored standard files, then protox's
/// embedded `google/protobuf/*`.
fn compiler(memory: &[(&str, &str)]) -> Compiler {
    let mut chain = ChainFileResolver::new();
    chain.add(Memory(
        memory
            .iter()
            .map(|(name, source)| (name.to_string(), source.to_string()))
            .collect(),
    ));
    chain.add(IncludeFileResolver::new(third_party()));
    chain.add(GoogleFileResolver::new());
    let mut compiler = Compiler::with_file_resolver(chain);
    compiler.include_imports(true).include_source_info(true);
    compiler
}

fn compile(memory: &[(&str, &str)], roots: &[&str]) -> Result<DescriptorPool, protox::Error> {
    let mut compiler = compiler(memory);
    compiler.open_files(roots)?;
    Ok(compiler.descriptor_pool())
}

fn file_names(pool: &DescriptorPool) -> Vec<String> {
    pool.files().map(|f| f.name().to_string()).collect()
}

// ---------------------------------------------------------------- Q1 several roots

#[test]
fn q1_compiles_several_roots_at_once() {
    let common = r#"syntax = "proto3"; package shop.v1; message Money { int64 cents = 1; }"#;
    let orders = r#"syntax = "proto3"; package shop.v1; import "shop/v1/common.proto";
        message Order { Money total = 1; }"#;
    let users = r#"syntax = "proto3"; package shop.v1; message User { string id = 1; }"#;
    let files = [
        ("shop/v1/common.proto", common),
        ("shop/v1/orders.proto", orders),
        ("shop/v1/users.proto", users),
    ];
    let pool = compile(&files, &["shop/v1/users.proto", "shop/v1/orders.proto"]).unwrap();
    let names = file_names(&pool);
    // Every root and the imported file; dependencies come before their importers.
    assert!(names.contains(&"shop/v1/users.proto".to_string()));
    let common_at = names
        .iter()
        .position(|n| n == "shop/v1/common.proto")
        .unwrap();
    let orders_at = names
        .iter()
        .position(|n| n == "shop/v1/orders.proto")
        .unwrap();
    assert!(common_at < orders_at);
    assert!(pool.get_message_by_name("shop.v1.Order").is_some());
}

// ------------------------------------------------- Q2 memory + disk + well-known

#[test]
fn q2_mixes_generated_sources_vendored_files_and_well_known_types() {
    // What a code-first app would generate: it imports a contract written by hand
    // (memory here, disk in practice), google/rpc (vendored) and google/protobuf
    // (embedded in protox).
    let contract = r#"syntax = "proto3"; package billing.v1; message Invoice { string id = 1; }"#;
    let generated = r#"syntax = "proto3"; package orders.v1;
        import "billing/v1/invoice.proto";
        import "google/rpc/status.proto";
        import "google/protobuf/timestamp.proto";
        message Order {
          billing.v1.Invoice invoice = 1;
          google.rpc.Status last_error = 2;
          google.protobuf.Timestamp at = 3;
        }"#;
    let pool = compile(
        &[
            ("billing/v1/invoice.proto", contract),
            ("orders/v1/orders.proto", generated),
        ],
        &["orders/v1/orders.proto"],
    )
    .unwrap();
    let order = pool.get_message_by_name("orders.v1.Order").unwrap();
    let types: Vec<String> = order
        .fields()
        .map(|f| match f.kind() {
            Kind::Message(m) => m.full_name().to_string(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        types,
        [
            "billing.v1.Invoice",
            "google.rpc.Status",
            "google.protobuf.Timestamp"
        ]
    );
    // google.rpc.Status pulls google/protobuf/any.proto in with include_imports.
    assert!(file_names(&pool).contains(&"google/protobuf/any.proto".to_string()));
}

#[test]
fn q2_memory_shadows_disk_with_the_same_name() {
    // The first resolver wins: a project can override a vendored file.
    let custom = r#"syntax = "proto3"; package google.rpc; message Status { int32 code = 1; }"#;
    let pool = compile(
        &[("google/rpc/status.proto", custom)],
        &["google/rpc/status.proto"],
    )
    .unwrap();
    assert_eq!(
        pool.get_message_by_name("google.rpc.Status")
            .unwrap()
            .fields()
            .len(),
        1
    );
}

// ------------------------------------------------------------- Q3 comments

/// Leading and trailing comments of the element at `path` in `file`.
fn comments(file: &FileDescriptorProto, path: &[i32]) -> (Option<String>, Option<String>) {
    let info = file.source_code_info.as_ref().expect("source info kept");
    let location = info
        .location
        .iter()
        .find(|l| l.path == path)
        .expect("location");
    (
        location.leading_comments.clone(),
        location.trailing_comments.clone(),
    )
}

#[test]
fn q3_keeps_comments_for_every_element() {
    let source = r#"syntax = "proto3";
package doc.v1;

// An order placed by a customer.
message Order {
  // Public identifier.
  string id = 1;
  Status status = 2; // Current state.
}

// Lifecycle of an order.
enum Status {
  // Not set.
  STATUS_UNSPECIFIED = 0;
}

// Order management.
service Orders {
  // Fetch one order.
  rpc GetOrder(Order) returns (Order);
}
"#;
    let pool = compile(&[("doc.proto", source)], &["doc.proto"]).unwrap();
    let file = pool.get_file_by_name("doc.proto").unwrap();
    let proto = file.file_descriptor_proto();
    let order = pool.get_message_by_name("doc.v1.Order").unwrap();
    let status = pool.get_enum_by_name("doc.v1.Status").unwrap();
    let service = pool.get_service_by_name("doc.v1.Orders").unwrap();
    let method = service.methods().next().unwrap();

    let leading = |path: &[i32]| comments(proto, path).0.map(|c| c.trim().to_string());
    assert_eq!(
        leading(order.path()).as_deref(),
        Some("An order placed by a customer.")
    );
    let id = order.get_field_by_name("id").unwrap();
    assert_eq!(leading(id.path()).as_deref(), Some("Public identifier."));
    let status_field = order.get_field_by_name("status").unwrap();
    assert_eq!(
        comments(proto, status_field.path())
            .1
            .as_deref()
            .map(str::trim),
        Some("Current state.")
    );
    assert_eq!(
        leading(status.path()).as_deref(),
        Some("Lifecycle of an order.")
    );
    let value = status.values().next().unwrap();
    assert_eq!(leading(value.path()).as_deref(), Some("Not set."));
    assert_eq!(
        leading(service.path()).as_deref(),
        Some("Order management.")
    );
    assert_eq!(leading(method.path()).as_deref(), Some("Fetch one order."));
}

#[test]
fn q3_source_info_is_optional_and_costs_size() {
    let source = std::fs::read_to_string(third_party().join("google/rpc/error_details.proto"))
        .expect("vendored file");
    let encode = |with_info: bool| {
        let mut compiler = compiler(&[("e.proto", &source)]);
        compiler
            .include_source_info(with_info)
            .include_imports(false);
        compiler.open_file("e.proto").unwrap();
        compiler.encode_file_descriptor_set()
    };
    let (with, without) = (encode(true), encode(false));
    println!(
        "error_details.proto descriptor: {} bytes without source info, {} with",
        without.len(),
        with.len()
    );
    assert!(with.len() > without.len());
    let set = FileDescriptorSet::decode(without.as_slice()).unwrap();
    assert!(set.file[0].source_code_info.is_none());
}

// ---------------------------------------------------------- Q4 determinism

#[test]
fn q4_output_is_deterministic() {
    let a = r#"syntax = "proto3"; package d; message A { string x = 1; }"#;
    let b = r#"syntax = "proto3"; package d; import "a.proto"; message B { A a = 1; }"#;
    let files = [("a.proto", a), ("b.proto", b)];
    let encode = |roots: &[&str]| {
        let mut compiler = compiler(&files);
        compiler.open_files(roots).unwrap();
        compiler.encode_file_descriptor_set()
    };
    // Same input, same bytes: embedded descriptors and `generate --check` can rely on it.
    assert_eq!(encode(&["b.proto"]), encode(&["b.proto"]));
    // Root order only changes the order of independent files, not their content.
    let forward = FileDescriptorSet::decode(encode(&["a.proto", "b.proto"]).as_slice()).unwrap();
    let backward = FileDescriptorSet::decode(encode(&["b.proto", "a.proto"]).as_slice()).unwrap();
    let sorted = |set: FileDescriptorSet| {
        let mut files = set.file;
        files.sort_by(|x, y| x.name.cmp(&y.name));
        files
    };
    assert_eq!(sorted(forward), sorted(backward));
}

// ------------------------------------------------- Q5 per-file split (reflection)

#[test]
fn q5_splits_the_set_per_file_and_finds_symbols() {
    let source = r#"syntax = "proto3"; package r.v1;
        import "google/protobuf/empty.proto";
        message Ping { string id = 1; message Inner {} }
        enum Mode { MODE_UNSPECIFIED = 0; }
        service Pinger { rpc Ping(r.v1.Ping) returns (google.protobuf.Empty); }"#;
    let mut compiler = compiler(&[("r/v1/ping.proto", source)]);
    compiler.open_file("r/v1/ping.proto").unwrap();
    let set = compiler.file_descriptor_set();
    let pool = compiler.descriptor_pool();

    // Each file re-encodes to exactly its entry in the set: reflection can serve the
    // per-file bytes as they are.
    for file in pool.files() {
        let bytes = file.file_descriptor_proto().encode_to_vec();
        let expected = set.file.iter().find(|f| f.name() == file.name()).unwrap();
        assert_eq!(
            FileDescriptorProto::decode(bytes.as_slice()).unwrap(),
            *expected
        );
    }

    // "File containing symbol" for the kinds of names a reflection client sends.
    let file_of = |symbol: &str| -> Option<String> {
        if let Some(m) = pool.get_message_by_name(symbol) {
            return Some(m.parent_file().name().to_string());
        }
        if let Some(e) = pool.get_enum_by_name(symbol) {
            return Some(e.parent_file().name().to_string());
        }
        if let Some(s) = pool.get_service_by_name(symbol) {
            return Some(s.parent_file().name().to_string());
        }
        let (service, method) = symbol.rsplit_once('.')?;
        let service = pool.get_service_by_name(service)?;
        service
            .methods()
            .any(|m| m.name() == method)
            .then(|| service.parent_file().name().to_string())
    };
    for symbol in [
        "r.v1.Ping",
        "r.v1.Ping.Inner",
        "r.v1.Mode",
        "r.v1.Pinger",
        "r.v1.Pinger.Ping",
    ] {
        assert_eq!(
            file_of(symbol).as_deref(),
            Some("r/v1/ping.proto"),
            "{symbol}"
        );
    }
    assert_eq!(file_of("r.v1.Missing"), None);

    // Transitive dependencies, as reflection returns them.
    let file = pool.get_file_by_name("r/v1/ping.proto").unwrap();
    let deps: Vec<String> = file.dependencies().map(|d| d.name().to_string()).collect();
    assert_eq!(deps, ["google/protobuf/empty.proto"]);
}

// ------------------------------------------------------------------ Q6 errors

#[test]
fn q6_errors_point_to_file_line_and_column() {
    let cases = [
        (
            "syntax",
            r#"syntax = "proto3"; package e; message A { string x = 1 }"#,
        ),
        (
            "unknown type",
            "syntax = \"proto3\";\npackage e;\nmessage A {\n  Missing x = 1;\n}",
        ),
        (
            "missing import",
            r#"syntax = "proto3"; package e; import "nowhere.proto";"#,
        ),
        (
            "duplicate number",
            "syntax = \"proto3\";\npackage e;\nmessage A {\n  string x = 1;\n  string y = 1;\n}",
        ),
        (
            "reserved number",
            "syntax = \"proto3\";\npackage e;\nmessage A {\n  reserved 2;\n  string x = 2;\n}",
        ),
    ];
    for (label, source) in cases {
        let err = compile(&[("e.proto", source)], &["e.proto"]).unwrap_err();
        let (display, debug) = (err.to_string(), format!("{err:?}"));
        println!("[{label}]\n  display: {display}\n  debug:   {debug}");
        assert!(
            debug.contains("e.proto"),
            "{label}: no file name in {debug:?}"
        );
    }
    // Unknown types and duplicates carry a line:column (line 4 / 5 above).
    let err = compile(&[("e.proto", cases[1].1)], &["e.proto"]).unwrap_err();
    assert!(format!("{err:?}").contains("e.proto:4:"), "{err:?}");
}

// ------------------------------------------------------------ Q7 standard files

#[test]
fn q7_compiles_every_vendored_standard_file() {
    let roots = [
        "google/rpc/status.proto",
        "google/rpc/code.proto",
        "google/rpc/error_details.proto",
        "google/type/date.proto",
        "google/type/datetime.proto",
        "google/type/timeofday.proto",
        "google/type/latlng.proto",
        "google/type/money.proto",
        "grpc/health/v1/health.proto",
        "grpc/reflection/v1/reflection.proto",
        "grpc/reflection/v1alpha/reflection.proto",
    ];
    let pool = compile(&[], &roots).unwrap();
    for name in [
        "google.rpc.Status",
        "google.rpc.ErrorInfo",
        "google.rpc.BadRequest",
        "google.type.Date",
        "grpc.health.v1.HealthCheckResponse",
        "grpc.reflection.v1.ServerReflectionRequest",
    ] {
        assert!(pool.get_message_by_name(name).is_some(), "{name}");
    }
    let health = pool.get_service_by_name("grpc.health.v1.Health").unwrap();
    let watch = health.methods().find(|m| m.name() == "Watch").unwrap();
    assert!(!watch.is_client_streaming() && watch.is_server_streaming());
    let reflection = pool
        .get_service_by_name("grpc.reflection.v1.ServerReflection")
        .unwrap();
    let info = reflection.methods().next().unwrap();
    assert!(info.is_client_streaming() && info.is_server_streaming());
}

// ------------------------------------------------------------- Q8 compile time

#[test]
fn q8_compile_time_for_a_large_schema() {
    // 200 files x 25 messages x 20 fields, each file importing the previous one.
    let mut sources = Vec::new();
    for f in 0..200 {
        let mut text = String::from("syntax = \"proto3\";\npackage big.v1;\n");
        if f > 0 {
            text.push_str(&format!("import \"big/f{}.proto\";\n", f - 1));
        }
        for m in 0..25 {
            text.push_str(&format!("message M{f}_{m} {{\n"));
            for n in 1..=20 {
                text.push_str(&format!("  string field_{n} = {n};\n"));
            }
            text.push_str("}\n");
        }
        sources.push((format!("big/f{f}.proto"), text));
    }
    let files: Vec<(&str, &str)> = sources
        .iter()
        .map(|(n, s)| (n.as_str(), s.as_str()))
        .collect();
    for with_info in [true, false] {
        let started = Instant::now();
        let mut compiler = compiler(&files);
        compiler.include_source_info(with_info);
        compiler.open_file("big/f199.proto").unwrap();
        let bytes = compiler.encode_file_descriptor_set();
        let compile_time = started.elapsed();
        let started = Instant::now();
        let decoded = DescriptorPool::decode(bytes.as_slice()).unwrap();
        let decode_time = started.elapsed();
        println!(
            "5000 messages / 100000 fields, source info {with_info}: compile {compile_time:?}, \
             descriptor {} KiB, decode {decode_time:?}",
            bytes.len() / 1024
        );
        assert_eq!(decoded.all_messages().len(), 5000);
    }
}

// --------------------------------------------------------- Q9 proto2, editions

#[test]
fn q9_proto2_is_read_and_editions_are_not() {
    let proto2 = r#"syntax = "proto2"; package p2;
        message Legacy { required string id = 1; optional int32 retries = 2 [default = 3]; }"#;
    let pool = compile(&[("p2.proto", proto2)], &["p2.proto"]).unwrap();
    let retries = pool
        .get_message_by_name("p2.Legacy")
        .unwrap()
        .get_field_by_name("retries")
        .unwrap();
    assert_eq!(retries.default_value(), prost_reflect::Value::I32(3));

    let editions = r#"edition = "2023"; package ed; message M { string x = 1; }"#;
    let err = compile(&[("ed.proto", editions)], &["ed.proto"]).unwrap_err();
    println!("editions: {err:?}");
}

// ------------------------------------------- Q10 what code generation needs

#[test]
fn q10_exposes_what_model_generation_needs() {
    let source = r#"syntax = "proto3"; package g.v1;
        message Order {
          message Line { string sku = 1; }
          enum Kind { KIND_UNSPECIFIED = 0; KIND_GIFT = 1; }
          reserved 9, 15 to 20;
          reserved "legacy";
          string id = 1 [json_name = "orderId", deprecated = true];
          optional string note = 2;
          oneof payment { string card = 3; string iban = 4; }
          map<string, Line> lines = 5;
          repeated Kind kinds = 6;
          Order parent = 7;
        }
        service Orders { rpc Watch(stream Order) returns (stream Order); }"#;
    let pool = compile(&[("g.proto", source)], &["g.proto"]).unwrap();
    let order = pool.get_message_by_name("g.v1.Order").unwrap();

    // Nested types and their parent.
    let line = pool.get_message_by_name("g.v1.Order.Line").unwrap();
    assert_eq!(line.parent_message().unwrap().full_name(), "g.v1.Order");
    assert!(pool.get_enum_by_name("g.v1.Order.Kind").is_some());

    // Reserved numbers and names: input for proto.lock and `breaking` checks.
    let ranges: Vec<_> = order.reserved_ranges().collect();
    assert_eq!(ranges, [9..10, 15..21]);
    assert_eq!(order.reserved_names().collect::<Vec<_>>(), ["legacy"]);

    // Field metadata.
    let id = order.get_field_by_name("id").unwrap();
    assert_eq!(id.json_name(), "orderId");
    assert!(
        id.field_descriptor_proto()
            .options
            .as_ref()
            .unwrap()
            .deprecated()
    );
    let note = order.get_field_by_name("note").unwrap();
    assert!(note.supports_presence());
    assert!(note.containing_oneof().unwrap().is_synthetic());

    // Real oneofs vs the synthetic one of `optional`.
    let real: Vec<_> = order
        .oneofs()
        .filter(|o| !o.is_synthetic())
        .map(|o| {
            (
                o.name().to_string(),
                o.fields().map(|f| f.name().to_string()).collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        real,
        [(
            "payment".to_string(),
            vec!["card".to_string(), "iban".to_string()]
        )]
    );

    // Maps, repeated enums, recursion.
    let lines = order.get_field_by_name("lines").unwrap();
    assert!(lines.is_map());
    let Kind::Message(entry) = lines.kind() else {
        panic!()
    };
    assert_eq!(
        entry
            .map_entry_value_field()
            .kind()
            .as_message()
            .unwrap()
            .full_name(),
        "g.v1.Order.Line"
    );
    let kinds = order.get_field_by_name("kinds").unwrap();
    assert!(kinds.is_list() && kinds.is_packed());
    let parent = order.get_field_by_name("parent").unwrap();
    assert_eq!(
        parent.kind().as_message().unwrap().full_name(),
        "g.v1.Order"
    );

    // Streaming shape of methods.
    let watch = pool
        .get_service_by_name("g.v1.Orders")
        .unwrap()
        .methods()
        .next()
        .unwrap();
    assert!(watch.is_client_streaming() && watch.is_server_streaming());

    // Field declaration order is kept (codegen keeps the .proto order).
    let names: Vec<_> = order.fields().map(|f| f.name().to_string()).collect();
    assert_eq!(
        names,
        ["id", "note", "card", "iban", "lines", "kinds", "parent"]
    );
    let _: BTreeSet<u32> = order.fields().map(|f| f.number()).collect();
}
