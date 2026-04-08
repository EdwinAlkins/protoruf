"""
Example 2: E-commerce Order System

This example demonstrates protoruf with a more complex e-commerce schema
including nested messages, enums, and repeated fields.
"""

import json
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

print("Compiling e-commerce proto file...")
descriptor = compile_proto("examples/ecommerce.proto")
print(f"Descriptor size: {len(descriptor)} bytes\n")

# Message types
ORDER_TYPE = "ecommerce.Order"
INVENTORY_UPDATE_TYPE = "ecommerce.UpdateInventoryRequest"
INVENTORY_RESPONSE_TYPE = "ecommerce.UpdateInventoryResponse"

# Example 1: Complete Order with all fields
print("=" * 60)
print("Example 1: Complete Order")
print("=" * 60)

order_json = json.dumps(
    {
        "order_id": "ORD-2024-001234",
        "customer_id": "CUST-789456",
        "status": "STATUS_PROCESSING",
        "items": [
            {
                "product_id": "PROD-001",
                "product_name": "Wireless Headphones",
                "quantity": 2,
                "unit_price": 7999,  # $79.99 in cents
                "attributes": {"color": "black", "warranty": "2 years"},
            },
            {
                "product_id": "PROD-002",
                "product_name": "USB-C Cable",
                "quantity": 3,
                "unit_price": 1299,  # $12.99 in cents
                "attributes": {"length": "2m", "color": "white"},
            },
        ],
        "shipping_address": {
            "street": "123 Main Street, Apt 4B",
            "city": "New York",
            "state": "NY",
            "postal_code": "10001",
            "country": "USA",
            "phone": "+1-555-123-4567",
        },
        "payment": {
            "method": "PAYMENT_CREDIT_CARD",
            "transaction_id": "TXN-ABC123XYZ",
            "amount": 19895,  # $198.95 in cents
            "status": "PAYMENT_STATUS_COMPLETED",
        },
        "created_at": 1709308800,
        "updated_at": 1709312400,
    },
    indent=2,
)

print("\nOriginal Order JSON:")
print(order_json)

# Convert to Protobuf
order_bytes = json_to_protobuf(order_json, descriptor, message_type=ORDER_TYPE)
print(f"\nProtobuf binary size: {len(order_bytes)} bytes")

# Calculate compression ratio
original_size = len(order_json.encode("utf-8"))
compression_ratio = (1 - len(order_bytes) / original_size) * 100
print(f"Original JSON size: {original_size} bytes")
print(f"Compression ratio: {compression_ratio:.1f}%")

# Convert back to JSON
decoded_json = protobuf_to_json(
    order_bytes, descriptor, message_type=ORDER_TYPE, pretty=True
)
print("\nDecoded Order from Protobuf:")
print(decoded_json)

# Example 2: Order with minimal fields (testing defaults)
print("\n" + "=" * 60)
print("Example 2: Minimal Order (Testing Defaults)")
print("=" * 60)

minimal_order_json = json.dumps(
    {
        "order_id": "ORD-2024-001235",
        "customer_id": "CUST-789457",
        "status": "STATUS_PENDING",
    }
)

print("\nMinimal Order JSON:")
print(minimal_order_json)

minimal_bytes = json_to_protobuf(
    minimal_order_json, descriptor, message_type=ORDER_TYPE
)
print(f"\nProtobuf binary size: {len(minimal_bytes)} bytes")

decoded_minimal = protobuf_to_json(
    minimal_bytes, descriptor, message_type=ORDER_TYPE, pretty=True
)
print("\nDecoded Minimal Order:")
print(decoded_minimal)

# Example 3: Inventory Update
print("\n" + "=" * 60)
print("Example 3: Inventory Update")
print("=" * 60)

inventory_update_json = json.dumps(
    {
        "sku": "WH-001-PROD-001",
        "quantity_change": -5,
        "reason": "Order fulfillment - ORD-2024-001234",
    }
)

print("\nInventory Update JSON:")
print(inventory_update_json)

update_bytes = json_to_protobuf(
    inventory_update_json, descriptor, message_type=INVENTORY_UPDATE_TYPE
)
print(f"\nProtobuf binary size: {len(update_bytes)} bytes")

# Example 4: Inventory Response
print("\n" + "=" * 60)
print("Example 4: Inventory Update Response")
print("=" * 60)

inventory_response_json = json.dumps(
    {
        "success": True,
        "item": {
            "sku": "WH-001-PROD-001",
            "product_id": "PROD-001",
            "quantity_available": 150,
            "quantity_reserved": 25,
            "warehouse_id": "WH-001",
            "last_updated": 1709315000,
        },
        "message": "Inventory updated successfully",
    }
)

print("\nInventory Response JSON:")
print(inventory_response_json)

response_bytes = json_to_protobuf(
    inventory_response_json, descriptor, message_type=INVENTORY_RESPONSE_TYPE
)
print(f"\nProtobuf binary size: {len(response_bytes)} bytes")

decoded_response = protobuf_to_json(
    response_bytes, descriptor, message_type=INVENTORY_RESPONSE_TYPE, pretty=True
)
print("\nDecoded Inventory Response:")
print(decoded_response)

# Example 5: Enum handling
print("\n" + "=" * 60)
print("Example 5: Enum Values")
print("=" * 60)

print("""
Available OrderStatus values:
- STATUS_UNSPECIFIED (0)
- STATUS_PENDING (1)
- STATUS_CONFIRMED (2)
- STATUS_PROCESSING (3)
- STATUS_SHIPPED (4)
- STATUS_DELIVERED (5)
- STATUS_CANCELLED (6)
- STATUS_REFUNDED (7)

Available PaymentMethod values:
- PAYMENT_UNSPECIFIED (0)
- PAYMENT_CREDIT_CARD (1)
- PAYMENT_DEBIT_CARD (2)
- PAYMENT_PAYPAL (3)
- PAYMENT_BANK_TRANSFER (4)
- PAYMENT_CRYPTO (5)
""")

print("\n" + "=" * 60)
print("✅ All e-commerce examples completed successfully!")
print("=" * 60)
