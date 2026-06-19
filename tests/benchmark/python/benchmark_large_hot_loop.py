#!/usr/bin/env python3
"""
Benchmark protoruf vs google.protobuf on LARGE (~1 MB) messages.

⚡ "Hot loop" variant — uses `DescriptorCache` (pool decoded once, outside the
timed loop). For the same ~1 MB message **without** the cache (free functions that
re-decode the descriptor on every call), see `benchmark_large.py`.

Why this benchmark exists
-------------------------
`benchmark.py` and `benchmark_hot_loop.py` use a small message, so the per-call
*fixed* costs (descriptor decode, Python call overhead) dominate. The large
benchmarks do the opposite: they build a **~1 MB** message so the **payload
conversion itself** (serde/prost parsing & encoding of thousands of fields)
dominates — exactly the regime where a Rust core has the most to gain over pure
Python.

It uses a dedicated schema, `proto/large.proto` (`bench.Dataset`): thousands of
nested `Record`s, each with scalars, an enum, a repeated field, a map and a
vector of doubles.

It also doubles as a correctness stress test: the large message is round-tripped
through protoruf and key fields are asserted before any timing is measured.

Like the other benchmarks, it reuses the descriptor compiled by protoruf for both
libraries (no external `protoc`), and on the google side the factory/pool is built
once outside the measured loop (google already caches its pool) — so the cached
comparison is fair.
"""

import json
import time
from pathlib import Path

# --- protoruf -------------------------------------------------
from protoruf import DescriptorCache, compile_proto

# --- google.protobuf (optional, benchmark only) ---------------
try:
    from google.protobuf import descriptor_pool, message_factory, json_format
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    HAS_GOOGLE_PROTOBUF = True
except ImportError:
    HAS_GOOGLE_PROTOBUF = False
    print("⚠️  google.protobuf not installed, only protoruf will be measured.")
    print("   Install it with: uv sync --group benchmark")

# ----------------------------------------------------------------------
# Configuration
# ----------------------------------------------------------------------
# Dedicated schema, next to this benchmark.
PROTO_PATH = Path(__file__).parent / "proto" / "large.proto"
MESSAGE_TYPE = "bench.Dataset"

TARGET_MB = 1.0  # approximate JSON size of the generated message
ITERATIONS = 200  # large messages are heavy; a few hundred iterations is plenty


def build_large_dataset(target_bytes: int) -> dict:
    """
    Build a ~`target_bytes` `bench.Dataset` by appending `Record`s until the
    serialized JSON reaches the target size. Each record exercises scalars, an
    enum, a repeated field, a map and a nested vector of doubles.
    """
    def make_record(index: int) -> dict:
        return {
            "id": f"rec-{index:08d}",
            "name": f"Record number {index}",
            # STATUS_ACTIVE / STATUS_ARCHIVED — non-zero on purpose: proto3
            # omits default (0) values, which would drop the field on output.
            "status": (index % 2) + 1,
            "timestamp": 1_700_000_000 + index,
            "tags": [f"tag-{index % 50}", f"group-{index % 10}"],
            "attributes": {"region": "eu-west", "tier": str(index % 5), "note": "x" * 16},
            "embedding": {
                "values": [round((index % 7) * 0.125 + k, 4) for k in range(8)],
            },
        }

    records: list[dict] = []
    obj = {
        "dataset_id": "ds-benchmark-1",
        "description": "large benchmark dataset",
        "records": records,
        "metadata": {"source": "benchmark", "version": "1.0", "env": "bench"},
    }

    batch_size = 200
    index = 0
    for _ in range(batch_size):
        records.append(make_record(index))
        index += 1

    current_size = len(json.dumps(obj))
    if current_size < target_bytes:
        batch_json = json.dumps(records[-batch_size:])
        bytes_per_record = len(batch_json) / batch_size
        extra_records = int((target_bytes - current_size) / bytes_per_record)
        for _ in range(extra_records):
            records.append(make_record(index))
            index += 1

    while len(json.dumps(obj)) < target_bytes:
        for _ in range(min(batch_size, 50)):
            records.append(make_record(index))
            index += 1

    return obj


def get_descriptor() -> bytes:
    """Compile the dedicated benchmark schema (in memory)."""
    return compile_proto(str(PROTO_PATH))


def create_google_message_factory(descriptor_bytes: bytes):
    """Build a google.protobuf message class from protoruf's FileDescriptorSet."""
    fds = FileDescriptorSet()
    fds.ParseFromString(descriptor_bytes)

    pool = descriptor_pool.DescriptorPool()
    for file_proto in fds.file:
        pool.Add(file_proto)

    msg_desc = pool.FindMessageTypeByName(MESSAGE_TYPE)

    # API differs across protobuf versions.
    if hasattr(message_factory, "GetMessageClass"):
        return message_factory.GetMessageClass(msg_desc)
    factory = message_factory.MessageFactory(pool=pool)
    if hasattr(factory, "GetMessageClass"):
        return factory.GetMessageClass(msg_desc)
    return factory.GetPrototype(msg_desc)


def main() -> None:
    print("Preparing the large-message benchmark (~1 MB, DescriptorCache)...\n")

    descriptor_bytes = get_descriptor()
    cache = DescriptorCache(descriptor_bytes)

    # 1. Build the large message and report its real size.
    target_bytes = int(TARGET_MB * 1_000_000)
    obj = build_large_dataset(target_bytes)
    json_str = json.dumps(obj)
    json_mb = len(json_str) / 1_000_000

    proto_bytes = cache.json_to_protobuf(json_str, MESSAGE_TYPE)
    proto_mb = len(proto_bytes) / 1_000_000

    print(f"Message size:  JSON = {json_mb:.2f} MB   Protobuf = {proto_mb:.2f} MB")
    print(f"Records:       {len(obj['records'])}\n")

    # 2. Correctness: the large message must round-trip exactly.
    restored = json.loads(cache.protobuf_to_json(proto_bytes, MESSAGE_TYPE))
    assert restored["dataset_id"] == obj["dataset_id"], "dataset_id changed"
    assert restored["description"] == obj["description"], "description changed"
    assert len(restored["records"]) == len(obj["records"]), "record count changed"
    for idx in (0, len(obj["records"]) - 1):
        exp, got = obj["records"][idx], restored["records"][idx]
        assert got["id"] == exp["id"], f"record {idx} id changed"
        assert got["name"] == exp["name"], f"record {idx} name changed"
        assert got["status"] == exp["status"], f"record {idx} status changed"  # enum -> number
        assert got["tags"] == exp["tags"], f"record {idx} tags changed"
        assert got["attributes"] == exp["attributes"], f"record {idx} attributes changed"
        assert len(got["embedding"]["values"]) == 8, f"record {idx} embedding length changed"
    print("✅ round-trip correctness verified on the ~1 MB message\n")

    # 3. google.protobuf setup (optional).
    google_ok = HAS_GOOGLE_PROTOBUF
    if google_ok:
        try:
            MessageFactory = create_google_message_factory(descriptor_bytes)
            google_msg = json_format.Parse(json_str, MessageFactory())
            google_proto_bytes = google_msg.SerializeToString()
            print("✅ google.protobuf set up successfully\n")
        except Exception as e:  # pragma: no cover - environment dependent
            print(f"⚠️  Error while setting up google.protobuf: {e}")
            print("   The benchmark will only cover protoruf.\n")
            google_ok = False

    results: dict[str, float] = {}

    def report(seconds: float) -> None:
        per_msg_ms = seconds / ITERATIONS * 1_000
        throughput = json_mb * ITERATIONS / seconds  # MB/s of JSON processed
        print(f"  → {seconds:.4f}s   {per_msg_ms:.3f} ms/msg   {throughput:,.0f} MB/s\n")

    # ----- Write (JSON -> Protobuf) -----
    print(f"protoruf write ({ITERATIONS:,} iterations)...")
    start = time.perf_counter()
    for _ in range(ITERATIONS):
        _ = cache.json_to_protobuf(json_str, MESSAGE_TYPE)
    results["protoruf_write"] = time.perf_counter() - start
    report(results["protoruf_write"])

    if google_ok:
        print(f"google.protobuf write ({ITERATIONS:,} iterations)...")
        start = time.perf_counter()
        for _ in range(ITERATIONS):
            msg = json_format.Parse(json_str, MessageFactory())
            _ = msg.SerializeToString()
        results["google_write"] = time.perf_counter() - start
        report(results["google_write"])

    # ----- Read (Protobuf -> JSON) -----
    print(f"protoruf read ({ITERATIONS:,} iterations)...")
    start = time.perf_counter()
    for _ in range(ITERATIONS):
        _ = cache.protobuf_to_json(proto_bytes, MESSAGE_TYPE)
    results["protoruf_read"] = time.perf_counter() - start
    report(results["protoruf_read"])

    if google_ok:
        print(f"google.protobuf read ({ITERATIONS:,} iterations)...")
        start = time.perf_counter()
        for _ in range(ITERATIONS):
            msg = MessageFactory()
            msg.ParseFromString(google_proto_bytes)
            _ = json_format.MessageToJson(msg)
        results["google_read"] = time.perf_counter() - start
        report(results["google_read"])

    # 4. Comparison table.
    print("=" * 78)
    print(f"Large-message results (~{json_mb:.2f} MB, {ITERATIONS:,} messages)")
    print("=" * 78)
    header = f"{'Operation':<28} {'google.protobuf':<18} {'protoruf (cache)':<18} {'Speedup':<8}"
    print(header)
    print("-" * len(header))

    for op, label in (("write", "Serialization (JSON→Proto)"), ("read", "Parsing (Proto→JSON)")):
        p = results[f"protoruf_{op}"]
        if google_ok:
            g = results[f"google_{op}"]
            print(f"{label:<28} {g:>8.4f}s         {p:>8.4f}s         {g / p:>5.1f}x")
        else:
            print(f"{label:<28} {'N/A':<18} {p:>8.4f}s")


if __name__ == "__main__":
    main()
