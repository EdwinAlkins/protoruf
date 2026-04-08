"""
Example 1: Basic Usage with User Service

This example demonstrates how to use protoruf with a User service schema.
It shows JSON ↔ Protobuf conversion for user management operations.
"""

import json
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# Compile the proto file to get the descriptor
# This uses Rust protox internally - no external dependencies needed!
print("Compiling proto file...")
descriptor = compile_proto("examples/user_service.proto")
print(f"Descriptor size: {len(descriptor)} bytes\n")

# Message type for this proto
MESSAGE_TYPE = "user.User"

# Example 1: Create a User from JSON
print("=" * 60)
print("Example 1: JSON → Protobuf → JSON (User)")
print("=" * 60)

user_json = json.dumps(
    {
        "id": "usr_123456",
        "email": "john.doe@example.com",
        "username": "johndoe",
        "role": "ROLE_ADMIN",
        "profile": {
            "first_name": "John",
            "last_name": "Doe",
            "bio": "Software developer passionate about Rust and Python",
            "avatar_url": "https://example.com/avatars/john.png",
            "social_links": {
                "twitter": "@johndoe",
                "github": "johndoe",
                "linkedin": "john-doe",
            },
        },
        "permissions": ["users:read", "users:write", "admin:access"],
        "active": True,
        "created_at": 1709308800,
    },
    indent=2,
)

print("\nOriginal JSON:")
print(user_json)

# Convert JSON to Protobuf bytes
protobuf_bytes = json_to_protobuf(user_json, descriptor, message_type=MESSAGE_TYPE)
print(f"\nProtobuf binary size: {len(protobuf_bytes)} bytes")

# Convert Protobuf back to JSON
result_json = protobuf_to_json(
    protobuf_bytes, descriptor, message_type=MESSAGE_TYPE, pretty=True
)
print("\nDecoded JSON from Protobuf:")
print(result_json)

# Example 2: CreateUserRequest
print("\n" + "=" * 60)
print("Example 2: CreateUserRequest")
print("=" * 60)

create_request_json = json.dumps(
    {
        "email": "new.user@example.com",
        "username": "newuser",
        "password": "secret123",
        "profile": {
            "first_name": "New",
            "last_name": "User",
            "bio": "Just joined!",
            "social_links": {},
        },
    }
)

print("\nCreateUserRequest JSON:")
print(create_request_json)

request_bytes = json_to_protobuf(
    create_request_json, descriptor, message_type="user.CreateUserRequest"
)
print(f"\nProtobuf binary size: {len(request_bytes)} bytes")

# Example 3: UserList (with repeated fields)
print("\n" + "=" * 60)
print("Example 3: UserList (Repeated Fields)")
print("=" * 60)

user_list_json = json.dumps(
    {
        "users": [
            {
                "id": "usr_001",
                "email": "alice@example.com",
                "username": "alice",
                "role": "ROLE_ADMIN",
                "active": True,
                "created_at": 1709200000,
            },
            {
                "id": "usr_002",
                "email": "bob@example.com",
                "username": "bob",
                "role": "ROLE_USER",
                "active": True,
                "created_at": 1709250000,
            },
            {
                "id": "usr_003",
                "email": "guest@example.com",
                "username": "guest",
                "role": "ROLE_GUEST",
                "active": False,
                "created_at": 1709280000,
            },
        ],
        "total": 3,
        "next_page_token": "page_2_token_xyz",
    }
)

print("\nUserList JSON:")
print(user_list_json)

list_bytes = json_to_protobuf(user_list_json, descriptor, message_type="user.UserList")
print(f"\nProtobuf binary size: {len(list_bytes)} bytes")

# Decode and verify - use correct message type
decoded = protobuf_to_json(
    list_bytes, descriptor, message_type="user.UserList", pretty=True
)
print("\nDecoded UserList:")
print(decoded)

print("\n" + "=" * 60)
print("✅ All examples completed successfully!")
print("=" * 60)
