"""
Example 3: IoT Sensor Data

This example demonstrates protoruf with IoT sensor data,
including oneof fields, nested messages, and batch processing.
"""

import json
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

print("Compiling IoT sensors proto file...")
descriptor = compile_proto("examples/iot_sensors.proto")
print(f"Descriptor size: {len(descriptor)} bytes\n")

# Example 1: Temperature Sensor Reading
print("=" * 60)
print("Example 1: Temperature Sensor Reading")
print("=" * 60)

temp_reading_json = json.dumps(
    {
        "device_id": "IoT-DEV-001",
        "sensor_id": "TEMP-001",
        "timestamp": 1709315000,
        "type": "SENSOR_TEMPERATURE",
        "data": {"temperature": {"celsius": 23.5, "fahrenheit": 74.3, "unit": "°C"}},
        "metadata": {
            "location": "Server Room A",
            "rack": "R-12",
            "calibrated": "2024-01-15",
        },
    },
    indent=2,
)

print("\nTemperature Reading JSON:")
print(temp_reading_json)

temp_bytes = json_to_protobuf(
    temp_reading_json, descriptor, message_type="iot.SensorReading"
)
print(f"\nProtobuf binary size: {len(temp_bytes)} bytes")

decoded_temp = protobuf_to_json(
    temp_bytes, descriptor, message_type="iot.SensorReading", pretty=True
)
print("\nDecoded Temperature Reading:")
print(decoded_temp)

# Example 2: Motion Sensor
print("\n" + "=" * 60)
print("Example 2: Motion Sensor Reading")
print("=" * 60)

motion_json = json.dumps(
    {
        "device_id": "IoT-DEV-003",
        "sensor_id": "MOTION-001",
        "timestamp": 1709315200,
        "type": "SENSOR_MOTION",
        "data": {"motion": {"detected": True, "confidence": 0.95, "motion_level": 3}},
        "metadata": {"location": "Entrance Hall"},
    },
    indent=2,
)

print("\nMotion Sensor JSON:")
print(motion_json)

motion_bytes = json_to_protobuf(
    motion_json, descriptor, message_type="iot.SensorReading"
)
print(f"\nProtobuf binary size: {len(motion_bytes)} bytes")

decoded_motion = protobuf_to_json(
    motion_bytes, descriptor, message_type="iot.SensorReading", pretty=True
)
print("\nDecoded Motion Reading:")
print(decoded_motion)

# Example 3: Batch of Sensor Readings
print("\n" + "=" * 60)
print("Example 3: Batch of Sensor Readings")
print("=" * 60)

batch_json = json.dumps(
    {
        "gateway_id": "GATEWAY-001",
        "batch_timestamp": 1709315300,
        "readings": [
            {
                "device_id": "IoT-DEV-001",
                "sensor_id": "TEMP-001",
                "timestamp": 1709315000,
                "type": "SENSOR_TEMPERATURE",
                "data": {
                    "temperature": {"celsius": 23.5, "fahrenheit": 74.3, "unit": "°C"}
                },
            },
            {
                "device_id": "IoT-DEV-002",
                "sensor_id": "HUM-001",
                "timestamp": 1709315050,
                "type": "SENSOR_HUMIDITY",
                "data": {
                    "humidity": {
                        "relative_humidity": 55.2,
                        "absolute_humidity": 12.8,
                        "unit": "%",
                    }
                },
            },
        ],
        "status": "BATCH_COMPLETE",
    },
    indent=2,
)

print("\nBatch Readings JSON:")
print(batch_json)

batch_bytes = json_to_protobuf(batch_json, descriptor, message_type="iot.SensorBatch")
print(f"\nProtobuf binary size: {len(batch_bytes)} bytes")

# Calculate compression
original_size = len(batch_json.encode("utf-8"))
compression_ratio = (1 - len(batch_bytes) / original_size) * 100
print(f"Original JSON size: {original_size} bytes")
print(f"Compression ratio: {compression_ratio:.1f}%")

decoded_batch = protobuf_to_json(
    batch_bytes, descriptor, message_type="iot.SensorBatch", pretty=True
)
print("\nDecoded Batch Readings:")
print(decoded_batch)

# Example 4: Device Alert
print("\n" + "=" * 60)
print("Example 4: Device Alert")
print("=" * 60)

alert_json = json.dumps(
    {
        "device_id": "IoT-DEV-004",
        "level": "ALERT_CRITICAL",
        "alert_type": "HIGH_TEMPERATURE",
        "message": "Temperature exceeded threshold: 85°C",
        "timestamp": 1709315400,
        "context": {
            "threshold": "75",
            "current_value": "85",
            "action_taken": "cooling_system_activated",
            "notified_users": "admin,maintenance",
        },
    },
    indent=2,
)

print("\nDevice Alert JSON:")
print(alert_json)

alert_bytes = json_to_protobuf(alert_json, descriptor, message_type="iot.DeviceAlert")
print(f"\nProtobuf binary size: {len(alert_bytes)} bytes")

decoded_alert = protobuf_to_json(
    alert_bytes, descriptor, message_type="iot.DeviceAlert", pretty=True
)
print("\nDecoded Alert:")
print(decoded_alert)

# Example 5: Device Registration
print("\n" + "=" * 60)
print("Example 5: Device Registration")
print("=" * 60)

registration_json = json.dumps(
    {
        "device_id": "IoT-DEV-100",
        "device_type": "Environmental Sensor Hub",
        "firmware_version": "2.5.1",
        "location": "Building A, Floor 2, Room 201",
        "capabilities": [
            "temperature",
            "humidity",
            "pressure",
            "air_quality",
            "wifi",
            "bluetooth",
        ],
        "registered_at": 1709315500,
    },
    indent=2,
)

print("\nDevice Registration JSON:")
print(registration_json)

reg_bytes = json_to_protobuf(
    registration_json, descriptor, message_type="iot.DeviceRegistration"
)
print(f"\nProtobuf binary size: {len(reg_bytes)} bytes")

decoded_reg = protobuf_to_json(
    reg_bytes, descriptor, message_type="iot.DeviceRegistration", pretty=True
)
print("\nDecoded Registration:")
print(decoded_reg)

print("\n" + "=" * 60)
print("✅ All IoT sensor examples completed successfully!")
print("=" * 60)
