"""
Example 4: Pydantic Integration

This example shows how to integrate protoruf with Pydantic
for type-safe JSON validation before Protobuf conversion.
"""

import json
from datetime import datetime
from pydantic import BaseModel, Field, field_validator
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# Compile proto file
descriptor = compile_proto("examples/user_service.proto")

# ============================================
# Pydantic Models matching the Protobuf schema
# ============================================


class Profile(BaseModel):
    """User profile model."""

    first_name: str = Field(..., min_length=1, max_length=100)
    last_name: str = Field(..., min_length=1, max_length=100)
    bio: str = Field(default="", max_length=500)
    avatar_url: str | None = None
    social_links: dict[str, str] = Field(default_factory=dict)

    @field_validator("avatar_url")
    @classmethod
    def validate_url(cls, v: str | None) -> str | None:
        if v and not v.startswith("http"):
            raise ValueError("avatar_url must be a valid URL")
        return v


class User(BaseModel):
    """User model with validation."""

    id: str = Field(..., pattern=r"^usr_[a-zA-Z0-9]+$")
    email: str = Field(..., pattern=r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
    username: str = Field(..., min_length=3, max_length=50)
    role: str = Field(default="ROLE_USER")
    profile: Profile | None = None
    permissions: list[str] = Field(default_factory=list)
    active: bool = True
    created_at: int

    @field_validator("role")
    @classmethod
    def validate_role(cls, v: str) -> str:
        valid_roles = [
            "ROLE_UNSPECIFIED",
            "ROLE_ADMIN",
            "ROLE_MODERATOR",
            "ROLE_USER",
            "ROLE_GUEST",
        ]
        if v not in valid_roles:
            raise ValueError(f"Invalid role: {v}. Must be one of {valid_roles}")
        return v

    @field_validator("created_at")
    @classmethod
    def validate_timestamp(cls, v: int) -> int:
        if v < 0:
            raise ValueError("created_at must be a positive timestamp")
        return v


# ============================================
# Example Usage
# ============================================

print("=" * 60)
print("Example 4: Pydantic Integration")
print("=" * 60)

# Create a validated user using Pydantic
print("\n1. Creating validated User with Pydantic...")

user = User(
    id="usr_abc123",
    email="alice@example.com",
    username="alice_dev",
    role="ROLE_ADMIN",
    profile=Profile(
        first_name="Alice",
        last_name="Smith",
        bio="Senior Software Engineer",
        avatar_url="https://example.com/alice.png",
        social_links={"github": "alice-dev", "linkedin": "alice-smith"},
    ),
    permissions=["users:read", "users:write", "system:admin"],
    active=True,
    created_at=int(datetime.now().timestamp()),
)

print(f"✅ User validated: {user.username} ({user.email})")

# Convert Pydantic model to JSON
user_json = user.model_dump_json(indent=2)
print("\n2. Pydantic → JSON:")
print(user_json)

# Convert JSON to Protobuf
print("\n3. JSON → Protobuf...")
protobuf_bytes = json_to_protobuf(user_json, descriptor, message_type="user.User")
print(f"✅ Protobuf size: {len(protobuf_bytes)} bytes")

# Convert Protobuf back to JSON
print("\n4. Protobuf → JSON...")
result_json = protobuf_to_json(
    protobuf_bytes, descriptor, message_type="user.User", pretty=True
)
print(result_json)

# Parse back to Pydantic to verify round-trip
print("\n5. Verifying round-trip with Pydantic...")
result_data = json.loads(result_json)

# Convert enum number back to string for Pydantic validation
enum_mapping = {
    0: "ROLE_UNSPECIFIED",
    1: "ROLE_ADMIN",
    2: "ROLE_MODERATOR",
    3: "ROLE_USER",
    4: "ROLE_GUEST",
}
if isinstance(result_data.get("role"), int):
    result_data["role"] = enum_mapping.get(result_data["role"], "ROLE_USER")

result_user = User(**result_data)
print(f"✅ Round-trip successful: {result_user.username}")

# ============================================
# Validation Error Example
# ============================================

print("\n" + "=" * 60)
print("Example: Pydantic Validation Error")
print("=" * 60)

print("\nTrying to create user with invalid data...")

try:
    invalid_user = User(
        id="invalid-id",  # Doesn't match pattern usr_*
        email="not-an-email",  # Invalid email
        username="ab",  # Too short (min 3)
        role="INVALID_ROLE",  # Invalid role
        created_at=-1,  # Negative timestamp
    )
except Exception as e:
    print(f"✅ Caught validation error (as expected):")
    print(f"   {type(e).__name__}: {str(e)[:100]}...")

# ============================================
# Batch Processing with Pydantic
# ============================================

print("\n" + "=" * 60)
print("Example: Batch Processing with Pydantic")
print("=" * 60)

users_data = [
    {
        "id": "usr_001",
        "email": "user1@example.com",
        "username": "user_one",
        "role": "ROLE_USER",
        "active": True,
        "created_at": 1709300000,
    },
    {
        "id": "usr_002",
        "email": "user2@example.com",
        "username": "user_two",
        "role": "ROLE_MODERATOR",
        "permissions": ["users:read"],
        "active": True,
        "created_at": 1709301000,
    },
    {
        "id": "usr_003",
        "email": "admin@example.com",
        "username": "admin_user",
        "role": "ROLE_ADMIN",
        "permissions": ["users:read", "users:write", "system:admin"],
        "active": True,
        "created_at": 1709302000,
    },
]

print(f"\nProcessing {len(users_data)} users...")

for i, user_data in enumerate(users_data, 1):
    # Validate with Pydantic
    user = User(**user_data)

    # Convert to JSON
    user_json = user.model_dump_json()

    # Convert to Protobuf
    protobuf_bytes = json_to_protobuf(user_json, descriptor, message_type="user.User")

    # Convert back to JSON
    result_json = protobuf_to_json(protobuf_bytes, descriptor, message_type="user.User")

    print(f"  {i}. {user.username} → {len(protobuf_bytes)} bytes ✓")

print("\n✅ All users processed successfully!")

print("\n" + "=" * 60)
print("✅ Pydantic integration example completed!")
print("=" * 60)
