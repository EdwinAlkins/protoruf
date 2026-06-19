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

import time
import json
from pathlib import Path

# --- protoruf -------------------------------------------------
from protoruf import DescriptorCache, compile_proto, load_descriptor

# --- google.protobuf (optional, benchmark only) ---------------
try:
    from google.protobuf import descriptor_pool, message_factory, json_format
    from google.protobuf.descriptor_pb2 import FileDescriptorSet
    from google.protobuf.internal.python_message import GeneratedProtocolMessageType

    HAS_GOOGLE_PROTOBUF = True
except ImportError:
    HAS_GOOGLE_PROTOBUF = False
    print("⚠️  google.protobuf not installed, only protoruf will be measured.")
    print("   Install it with: uv add protobuf")

# ----------------------------------------------------------------------
# Configuration
# ----------------------------------------------------------------------
PROTO_PATH = Path(__file__).parent / "proto" / "message.proto"
DESC_PATH = Path(__file__).parent / "proto" / "message.desc"
MESSAGE_TYPE = "message.Message"

# Number of iterations (adjust to your machine)
ITERATIONS = 100_000  # 100k for a quick bench, 1M for the final table


def get_descriptor():
    """Return the compiled descriptor (via protoruf)."""
    if not DESC_PATH.exists():
        compile_proto(PROTO_PATH, output_path=DESC_PATH)
    return load_descriptor(DESC_PATH)


def create_google_message_factory(descriptor_bytes: bytes):
    """
    Build a factory to instantiate google.protobuf messages dynamically from the
    FileDescriptorSet compiled by protoruf.

    Returns:
        tuple: (message_class, pool) where message_class is the message class
               (or a callable that creates a new instance)
    """
    fds = FileDescriptorSet()
    fds.ParseFromString(descriptor_bytes)

    pool = descriptor_pool.DescriptorPool()
    for file_proto in fds.file:
        pool.Add(file_proto)

    # Look up the message descriptor
    msg_desc = pool.FindMessageTypeByName(MESSAGE_TYPE)

    # API depends on the protobuf version:
    #   - protobuf >= 5.x : module-level function message_factory.GetMessageClass()
    #   - protobuf 4.x    : instance method factory.GetMessageClass()
    #   - protobuf < 4.x  : instance method factory.GetPrototype()
    if hasattr(message_factory, "GetMessageClass"):
        message_class = message_factory.GetMessageClass(msg_desc)
    else:
        factory = message_factory.MessageFactory(pool=pool)
        if hasattr(factory, "GetMessageClass"):
            message_class = factory.GetMessageClass(msg_desc)
        else:
            message_class = factory.GetPrototype(msg_desc)

    return message_class, pool


def main():
    print("Preparing the benchmark (hot loop / DescriptorCache)...\n")

    # 1. Load the descriptor (protoruf) and build the cache ONCE, outside the
    #    timed loop (equivalent to google's cached factory/pool).
    descriptor_bytes = get_descriptor()
    cache = DescriptorCache(descriptor_bytes)

    # 2. Test data
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

    # Prepare the protobuf bytes for the read benchmark
    proto_bytes = cache.json_to_protobuf(json_str, MESSAGE_TYPE)

    # 3. google.protobuf setup (if available)
    google_ok = HAS_GOOGLE_PROTOBUF
    if google_ok:
        try:
            MessageFactory, pool = create_google_message_factory(descriptor_bytes)

            # Build a google.protobuf message from the JSON
            if callable(MessageFactory):
                # If it is a function, call it
                google_msg = json_format.Parse(json_str, MessageFactory())
            else:
                # If it is a class, instantiate it
                google_msg = json_format.Parse(json_str, MessageFactory())

            google_proto_bytes = google_msg.SerializeToString()

            print(f"✅ google.protobuf set up successfully")
            print(f"   Message type: {type(google_msg).__name__}")
            print(f"   Binary size: {len(google_proto_bytes)} bytes\n")

        except Exception as e:
            print(f"⚠️  Error while setting up google.protobuf: {e}")
            print("   The benchmark will only cover protoruf.\n")
            google_ok = False

    # 4. Run the benchmarks
    results = {}

    # ----- Write (JSON -> Protobuf) -----
    # protoruf (DescriptorCache)
    print(f"protoruf write benchmark ({ITERATIONS:,} iterations)...")
    start = time.perf_counter()
    for _ in range(ITERATIONS):
        _ = cache.json_to_protobuf(json_str, MESSAGE_TYPE)
    t_protoruf_write = time.perf_counter() - start
    results["protoruf_write"] = t_protoruf_write
    print(f"  → {t_protoruf_write:.4f}s ({ITERATIONS / t_protoruf_write:,.0f} msg/s)\n")

    # google.protobuf
    if google_ok:
        print(f"google.protobuf write benchmark ({ITERATIONS:,} iterations)...")
        start = time.perf_counter()
        for _ in range(ITERATIONS):
            msg = json_format.Parse(json_str, MessageFactory())
            _ = msg.SerializeToString()
        t_google_write = time.perf_counter() - start
        results["google_write"] = t_google_write
        print(f"  → {t_google_write:.4f}s ({ITERATIONS / t_google_write:,.0f} msg/s)\n")

    # ----- Read (Protobuf -> JSON) -----
    # protoruf (DescriptorCache)
    print(f"protoruf read benchmark ({ITERATIONS:,} iterations)...")
    start = time.perf_counter()
    for _ in range(ITERATIONS):
        _ = cache.protobuf_to_json(proto_bytes, MESSAGE_TYPE)
    t_protoruf_read = time.perf_counter() - start
    results["protoruf_read"] = t_protoruf_read
    print(f"  → {t_protoruf_read:.4f}s ({ITERATIONS / t_protoruf_read:,.0f} msg/s)\n")

    # google.protobuf
    if google_ok:
        print(f"google.protobuf read benchmark ({ITERATIONS:,} iterations)...")
        start = time.perf_counter()
        for _ in range(ITERATIONS):
            msg = MessageFactory()
            msg.ParseFromString(google_proto_bytes)
            _ = json_format.MessageToJson(msg)
        t_google_read = time.perf_counter() - start
        results["google_read"] = t_google_read
        print(f"  → {t_google_read:.4f}s ({ITERATIONS / t_google_read:,.0f} msg/s)\n")

    # 5. Print the comparison table
    print("=" * 80)
    print(f"Hot loop results (DescriptorCache) for {ITERATIONS:,} messages")
    print("=" * 80)
    header = f"{'Operation':<30} {'google.protobuf':<20} {'protoruf (cache)':<20} {'Gain':<10}"
    print(header)
    print("-" * len(header))

    if google_ok:
        # Write (serialization)
        g_write = results["google_write"]
        p_write = results["protoruf_write"]
        gain_write = (1 - p_write / g_write) * 100
        print(
            f"{'Serialization (JSON→Proto)':<30} {g_write:>8.4f}s        {p_write:>8.4f}s        {gain_write:>5.1f}%"
        )

        # Read (parsing)
        g_read = results["google_read"]
        p_read = results["protoruf_read"]
        gain_read = (1 - p_read / g_read) * 100
        print(
            f"{'Parsing (Proto→JSON)':<30} {g_read:>8.4f}s        {p_read:>8.4f}s        {gain_read:>5.1f}%"
        )
    else:
        # protoruf-only display
        print(
            f"{'Serialization (JSON→Proto)':<30} {'N/A':<20} {results['protoruf_write']:>8.4f}s        {'':<10}"
        )
        print(
            f"{'Parsing (Proto→JSON)':<30} {'N/A':<20} {results['protoruf_read']:>8.4f}s        {'':<10}"
        )

    # 6. Projected table for 1M messages
    print(f"\n📊 Projection for 1,000,000 messages:\n")
    factor = 1_000_000 / ITERATIONS

    print(f"{'Operation':<30} {'google.protobuf':<15} {'protoruf (cache)':<15}")
    print("-" * 60)

    if google_ok:
        print(
            f"{'Serialization (1M)':<30} {results['google_write'] * factor:>6.2f}s         {results['protoruf_write'] * factor:>6.2f}s"
        )
        print(
            f"{'Parsing (1M)':<30} {results['google_read'] * factor:>6.2f}s         {results['protoruf_read'] * factor:>6.2f}s"
        )

        # Speedup
        speedup_write = results["google_write"] / results["protoruf_write"]
        speedup_read = results["google_read"] / results["protoruf_read"]
        print(f"\n   → protoruf is {speedup_write:.1f}x faster on writes")
        print(f"   → protoruf is {speedup_read:.1f}x faster on reads")
    else:
        print(
            f"{'Serialization (1M)':<30} {'N/A':<15} {results['protoruf_write'] * factor:>6.2f}s"
        )
        print(
            f"{'Parsing (1M)':<30} {'N/A':<15} {results['protoruf_read'] * factor:>6.2f}s"
        )


if __name__ == "__main__":
    main()
