#!/usr/bin/env python3
"""
Benchmark protoruf vs google.protobuf for JSON <-> Protobuf conversion.

Measures timings for:
  - Serialization   : JSON -> Protobuf
  - Parsing/deserialization : Protobuf -> JSON

protoruf uses the free functions `json_to_protobuf` / `protobuf_to_json`, which
re-decode the descriptor on every call. For the cached "hot loop" scenario using
`DescriptorCache`, see `benchmark_hot_loop.py`.

Uses the descriptor compiled by protoruf for both libraries, to avoid any
external compilation with protoc.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from benchmark_utils import (
    BenchmarkStats,
    create_google_message_factory,
    format_stats_line,
    print_comparison_table,
    print_methodology,
    print_system_info,
    run_timed_benchmark,
    warn_missing_google_protobuf,
    HAS_GOOGLE_PROTOBUF,
)

from protoruf import compile_proto, json_to_protobuf, load_descriptor, protobuf_to_json

try:
    from google.protobuf import json_format
except ImportError:
    pass

# Proto lives at tests/proto/ — three levels up from tests/benchmark/python/.
PROTO_PATH = Path(__file__).parents[2] / "proto" / "message.proto"
DESC_PATH = Path(__file__).parents[2] / "proto" / "message.desc"
MESSAGE_TYPE = "message.Message"

ITERATIONS = 10_000
WARMUP_ITERATIONS = 1_000
MEASURED_RUNS = 20


def get_descriptor() -> bytes:
    if not DESC_PATH.exists():
        compile_proto(PROTO_PATH, output_path=DESC_PATH)
    return load_descriptor(DESC_PATH)


def main() -> None:
    print_system_info()
    warn_missing_google_protobuf()
    print_methodology(
        warmup=WARMUP_ITERATIONS,
        runs=MEASURED_RUNS,
        iterations=ITERATIONS,
    )
    print("Preparing the benchmark (free functions, small message)...\n")

    descriptor_bytes = get_descriptor()

    original_obj = {
        "id": "123",
        "content": "Hello World! This is a benchmark.",
        "priority": 5,
        "tags": ["test", "example", "bench"],
        "metadata": {
            "author": "Alice",
            "created_at": 1234567890,
            "attributes": {"env": "prod", "version": "1.0"},
        },
    }
    json_str = json.dumps(original_obj)
    payload_bytes = len(json_str)

    proto_bytes = json_to_protobuf(json_str, descriptor_bytes, MESSAGE_TYPE)
    restored = json.loads(protobuf_to_json(proto_bytes, descriptor_bytes, message_type=MESSAGE_TYPE))
    assert restored == original_obj, "round-trip mismatch"
    print(f"Payload: JSON = {payload_bytes} bytes   Protobuf = {len(proto_bytes)} bytes\n")

    google_ok = HAS_GOOGLE_PROTOBUF
    MessageFactory = None
    google_proto_bytes = b""

    if google_ok:
        try:
            MessageFactory = create_google_message_factory(descriptor_bytes, MESSAGE_TYPE)
            google_msg = json_format.Parse(json_str, MessageFactory())
            google_proto_bytes = google_msg.SerializeToString()
            print("✅ google.protobuf set up successfully\n")
        except Exception as e:
            print(f"⚠️  Error while setting up google.protobuf: {e}")
            print("   The benchmark will only cover protoruf.\n")
            google_ok = False

    def protoruf_write() -> None:
        for _ in range(ITERATIONS):
            _ = json_to_protobuf(json_str, descriptor_bytes, MESSAGE_TYPE)

    def protoruf_read() -> None:
        for _ in range(ITERATIONS):
            _ = protobuf_to_json(proto_bytes, descriptor_bytes, message_type=MESSAGE_TYPE)

    print(f"protoruf write ({MEASURED_RUNS} runs × {ITERATIONS:,} iterations)...")
    protoruf_write_stats = run_timed_benchmark(
        protoruf_write,
        iterations=ITERATIONS,
        payload_bytes=payload_bytes,
        warmup_iterations=WARMUP_ITERATIONS,
        measured_runs=MEASURED_RUNS,
        label="protoruf write",
    )
    print(format_stats_line(protoruf_write_stats, show_mb_s=False) + "\n")

    google_write_stats: BenchmarkStats | None = None
    if google_ok:
        def google_write() -> None:
            for _ in range(ITERATIONS):
                msg = json_format.Parse(json_str, MessageFactory())
                _ = msg.SerializeToString()

        print(f"google.protobuf write ({MEASURED_RUNS} runs × {ITERATIONS:,} iterations)...")
        google_write_stats = run_timed_benchmark(
            google_write,
            iterations=ITERATIONS,
            payload_bytes=payload_bytes,
            warmup_iterations=WARMUP_ITERATIONS,
            measured_runs=MEASURED_RUNS,
            label="google.protobuf write",
        )
        print(format_stats_line(google_write_stats, show_mb_s=False) + "\n")

    print(f"protoruf read ({MEASURED_RUNS} runs × {ITERATIONS:,} iterations)...")
    protoruf_read_stats = run_timed_benchmark(
        protoruf_read,
        iterations=ITERATIONS,
        payload_bytes=payload_bytes,
        warmup_iterations=WARMUP_ITERATIONS,
        measured_runs=MEASURED_RUNS,
        label="protoruf read",
    )
    print(format_stats_line(protoruf_read_stats, show_mb_s=False) + "\n")

    google_read_stats: BenchmarkStats | None = None
    if google_ok:
        def google_read() -> None:
            for _ in range(ITERATIONS):
                msg = MessageFactory()
                msg.ParseFromString(google_proto_bytes)
                _ = json_format.MessageToJson(msg)

        print(f"google.protobuf read ({MEASURED_RUNS} runs × {ITERATIONS:,} iterations)...")
        google_read_stats = run_timed_benchmark(
            google_read,
            iterations=ITERATIONS,
            payload_bytes=payload_bytes,
            warmup_iterations=WARMUP_ITERATIONS,
            measured_runs=MEASURED_RUNS,
            label="google.protobuf read",
        )
        print(format_stats_line(google_read_stats, show_mb_s=False) + "\n")

    print_comparison_table(
        f"Small-message results — free functions ({ITERATIONS:,} conversions per run)",
        [
            ("Serialization (JSON→Proto)", google_write_stats, protoruf_write_stats),
            ("Parsing (Proto→JSON)", google_read_stats, protoruf_read_stats),
        ],
        show_mb_s=False,
        protoruf_label="protoruf (free)",
    )


if __name__ == "__main__":
    main()
