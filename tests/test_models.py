"""Pydantic models for Protobuf message types (test only)."""

from pydantic import BaseModel, Field


class Metadata(BaseModel):
    """Metadata model matching the Protobuf definition."""

    author: str = Field(default="", description="Author of the message")
    created_at: int = Field(default=0, description="Unix timestamp of creation")
    attributes: dict[str, str] = Field(
        default_factory=dict, description="Custom attributes"
    )


class Message(BaseModel):
    """Message model matching the Protobuf definition."""

    id: str = Field(default="", description="Unique identifier")
    content: str = Field(default="", description="Message content")
    priority: int = Field(default=0, description="Priority level")
    tags: list[str] = Field(default_factory=list, description="List of tags")
    metadata: Metadata = Field(default_factory=Metadata, description="Message metadata")
