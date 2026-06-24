#!/usr/bin/env python3
"""
Benchmark protoruf vs google.protobuf on LARGE messages.

Free-functions ("cold") variant — uses `json_to_protobuf` / `protobuf_to_json`,
which **re-decode the descriptor pool on every call**. For the same large message
**with** `DescriptorCache` (pool decoded once), see `benchmark_large_hot_loop.py`.

It uses a dedicated schema, `proto/large.proto` (`bench.Dataset`): a fixed number
of nested `Record`s, each with scalars, an enum, a repeated field, a map and a
vector of doubles. It also round-trips the message for correctness before timing.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from benchmark_utils import (
    BenchmarkStats,
    build_large_dataset,
    create_google_message_factory,
    format_stats_line,
    print_comparison_table,
    print_methodology,
    print_system_info,
    run_timed_benchmark,
    warn_missing_google_protobuf,
    HAS_GOOGLE_PROTOBUF,
)

from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

try:
    from google.protobuf import json_format
except ImportError:
    pass

PROTO_PATH = Path(__file__).parent / "proto" / "large.proto"
MESSAGE_TYPE = "bench.Dataset"

N_RECORDS = 5_000
ITERATIONS = 200
WARMUP_ITERATIONS = 10
MEASURED_RUNS = 10


def get_descriptor() -> bytes:
    return compile_proto(str(PROTO_PATH))


def verify_round_trip(obj: dict, restored: dict) -> None:
    assert restored["dataset_id"] == obj["dataset_id"], "dataset_id changed"
    assert restored["description"] == obj["description"], "description changed"
    assert len(restored["records"]) == len(obj["records"]), "record count changed"
    for idx in (0, len(obj["records"]) - 1):
        exp, got = obj["records"][idx], restored["records"][idx]
        assert got["id"] == exp["id"], f"record {idx} id changed"
        assert got["name"] == exp["name"], f"record {idx} name changed"
        assert got["status"] == exp["status"], f"record {idx} status changed"
        assert got["tags"] == exp["tags"], f"record {idx} tags changed"
        assert got["attributes"] == exp["attributes"], f"record {idx} attributes changed"
        assert len(got["embedding"]["values"]) == 8, f"record {idx} embedding length changed"


def main() -> None:
    print_system_info()
    warn_missing_google_protobuf()
    print_methodology(
        warmup=WARMUP_ITERATIONS,
        runs=MEASURED_RUNS,
        iterations=ITERATIONS,
    )
    print("Preparing the large-message benchmark (free functions)...\n")

    descriptor_bytes = get_descriptor()

    obj = build_large_dataset(N_RECORDS)
    json_str = json.dumps(obj)
    payload_bytes = len(json_str)
    json_mb = payload_bytes / 1_000_000

    proto_bytes = json_to_protobuf(json_str, descriptor_bytes, MESSAGE_TYPE)
    proto_mb = len(proto_bytes) / 1_000_000

    restored = json.loads(protobuf_to_json(proto_bytes, descriptor_bytes, message_type=MESSAGE_TYPE))
    verify_round_trip(obj, restored)

    print(f"Dataset:       {N_RECORDS:,} records (fixed, reproducible)")
    print(f"Message size:  JSON = {json_mb:.2f} MB   Protobuf = {proto_mb:.2f} MB")
    print("✅ round-trip correctness verified\n")

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
    print(format_stats_line(protoruf_write_stats, show_mb_s=True) + "\n")

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
        print(format_stats_line(google_write_stats, show_mb_s=True) + "\n")

    print(f"protoruf read ({MEASURED_RUNS} runs × {ITERATIONS:,} iterations)...")
    protoruf_read_stats = run_timed_benchmark(
        protoruf_read,
        iterations=ITERATIONS,
        payload_bytes=payload_bytes,
        warmup_iterations=WARMUP_ITERATIONS,
        measured_runs=MEASURED_RUNS,
        label="protoruf read",
    )
    print(format_stats_line(protoruf_read_stats, show_mb_s=True) + "\n")

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
        print(format_stats_line(google_read_stats, show_mb_s=True) + "\n")

    print_comparison_table(
        f"Large-message results — free functions ({N_RECORDS:,} records, "
        f"{json_mb:.2f} MB JSON, {ITERATIONS:,} conversions per run)",
        [
            ("Serialization (JSON→Proto)", google_write_stats, protoruf_write_stats),
            ("Parsing (Proto→JSON)", google_read_stats, protoruf_read_stats),
        ],
        show_mb_s=True,
        protoruf_label="protoruf (free)",
    )


if __name__ == "__main__":
    main()
