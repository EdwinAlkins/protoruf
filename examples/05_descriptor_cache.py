"""
Example 5: High-Throughput Conversion with DescriptorCache

The free functions (`json_to_protobuf` / `protobuf_to_json`) re-decode the
descriptor set on *every* call, which dominates the cost in hot loops.

`DescriptorCache` decodes the descriptor pool once and reuses it (and the
resolved message descriptors) across every conversion. Build it once, reuse it
everywhere — it is the recommended pattern for services converting many messages.
"""

import json
import time

from protoruf import (
    DescriptorCache,
    compile_proto,
    json_to_protobuf,
    protobuf_to_json,
)

# Compile the proto file to get the descriptor (once)
print("Compiling proto file...")
descriptor = compile_proto("examples/user_service.proto")
print(f"Descriptor size: {len(descriptor)} bytes\n")

MESSAGE_TYPE = "user.User"

user_json = json.dumps(
    {
        "id": "usr_123456",
        "email": "john.doe@example.com",
        "username": "johndoe",
        "role": "ROLE_ADMIN",
        "profile": {
            "first_name": "John",
            "last_name": "Doe",
            "social_links": {"twitter": "@johndoe", "github": "johndoe"},
        },
        "permissions": ["users:read", "users:write"],
        "active": True,
        "created_at": 1709308800,
    }
)

# ---------------------------------------------------------------------------
# Basic usage
# ---------------------------------------------------------------------------
print("=" * 60)
print("Basic usage: build the cache once, reuse it")
print("=" * 60)

# Decode the descriptor pool a single time
cache = DescriptorCache(descriptor)

# JSON -> Protobuf (no descriptor argument needed anymore)
protobuf_bytes = cache.json_to_protobuf(user_json, MESSAGE_TYPE)
print(f"\nProtobuf binary size: {len(protobuf_bytes)} bytes")

# Protobuf -> JSON (pretty is keyword-only after message_type)
result_json = cache.protobuf_to_json(protobuf_bytes, MESSAGE_TYPE, pretty=True)
print("\nDecoded JSON from Protobuf:")
print(result_json)

# A single cache handles every message type in the descriptor
create_request_bytes = cache.json_to_protobuf(
    json.dumps({"email": "new@example.com", "username": "newuser"}),
    "user.CreateUserRequest",
)
print(f"\nCreateUserRequest binary size: {len(create_request_bytes)} bytes")

# ---------------------------------------------------------------------------
# Why it matters: benchmark vs the free functions
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("Benchmark: free functions vs DescriptorCache")
print("=" * 60)

ITERATIONS = 50_000


def bench(fn) -> float:
    start = time.perf_counter()
    for _ in range(ITERATIONS):
        fn()
    return ITERATIONS / (time.perf_counter() - start)


free_rate = bench(lambda: json_to_protobuf(user_json, descriptor, MESSAGE_TYPE))
cache_rate = bench(lambda: cache.json_to_protobuf(user_json, MESSAGE_TYPE))

print(f"\nFree functions : {free_rate:>12,.0f} msg/s")
print(f"DescriptorCache: {cache_rate:>12,.0f} msg/s")
print(f"Speedup        : x{cache_rate / free_rate:.1f}")

print("\n" + "=" * 60)
print("✅ Example completed successfully!")
print("=" * 60)
