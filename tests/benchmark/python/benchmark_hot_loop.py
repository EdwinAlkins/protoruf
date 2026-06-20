#!/usr/bin/env python3
"""
Benchmark protoruf (hot loop) vs google.protobuf for JSON <-> Protobuf conversion.

⚡ "Hot loop" / high-frequency scenario.

Difference from `benchmark.py`:
  - `benchmark.py` uses the free functions `json_to_protobuf` /
    `protobuf_to_json`, which **re-decode the descriptor on every call**.
  - This file uses `DescriptorCache`, which **decodes the pool only once**
    (outside the timed loop) and then reuses it -- the recommended usage for a
    service processing many messages.

For a fair and transparent comparison, on the google.protobuf side the factory
and pool are also built **once** outside the measured loop (google already caches
its pool). Both files share the same message, descriptor, and iteration count:
only protoruf's descriptor strategy changes.

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

from protoruf import DescriptorCache, compile_proto, load_descriptor

try:
    from google.protobuf import json_format
except ImportError:
    pass

PROTO_PATH = Path(__file__).parents[2] / "proto" / "message.proto"
DESC_PATH = Path(__file__).parents[2] / "proto" / "message.desc"
MESSAGE_TYPE = "message.Message"

ITERATIONS = 1_000
WARMUP_ITERATIONS = 100
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
    print("Preparing the benchmark (hot loop / DescriptorCache, small message)...\n")

    descriptor_bytes = get_descriptor()
    cache = DescriptorCache(descriptor_bytes)

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

    proto_bytes = cache.json_to_protobuf(json_str, MESSAGE_TYPE)
    restored = json.loads(cache.protobuf_to_json(proto_bytes, MESSAGE_TYPE))
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
            _ = cache.json_to_protobuf(json_str, MESSAGE_TYPE)

    def protoruf_read() -> None:
        for _ in range(ITERATIONS):
            _ = cache.protobuf_to_json(proto_bytes, MESSAGE_TYPE)

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
        f"Small-message results — DescriptorCache ({ITERATIONS:,} conversions per run)",
        [
            ("Serialization (JSON→Proto)", google_write_stats, protoruf_write_stats),
            ("Parsing (Proto→JSON)", google_read_stats, protoruf_read_stats),
        ],
        show_mb_s=False,
        protoruf_label="protoruf (cache)",
    )


if __name__ == "__main__":
    main()
